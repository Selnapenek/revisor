//! App updates from the Revisor releases on Gitea. CI publishes them; see docs/releasing.md.

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

const LATEST_RELEASE_URL: &str = "https://gitea.slel.pro/api/v1/repos/slel/revisor/releases/latest";

/// End of the release asset name that updates this build.
fn asset_suffix() -> Option<&'static str> {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("-macos-arm64.tar.gz")
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        match linux_package()? {
            LinuxPackage::Fedora => Some(".x86_64.rpm"),
            LinuxPackage::Arch => Some("-x86_64.pkg.tar.zst"),
        }
    } else {
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LinuxPackage {
    Fedora,
    Arch,
}

fn linux_package() -> Option<LinuxPackage> {
    linux_package_from_os_release(&std::fs::read_to_string("/etc/os-release").ok()?)
}

fn linux_package_from_os_release(contents: &str) -> Option<LinuxPackage> {
    let mut id = "";
    let mut id_like = "";
    for line in contents.lines() {
        if let Some(value) = line.strip_prefix("ID=") {
            id = value.trim_matches('"');
        } else if let Some(value) = line.strip_prefix("ID_LIKE=") {
            id_like = value.trim_matches('"');
        }
    }
    if id == "arch" || id == "cachyos" || id_like.split_whitespace().any(|part| part == "arch") {
        Some(LinuxPackage::Arch)
    } else if id == "fedora" || id_like.split_whitespace().any(|part| part == "fedora") {
        Some(LinuxPackage::Fedora)
    } else {
        None
    }
}

#[derive(Clone, Debug)]
pub struct Release {
    pub version: String,
    pub notes: String,
    url: String,
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    #[serde(default)]
    body: String,
    assets: Vec<ApiAsset>,
}

#[derive(Deserialize)]
struct ApiAsset {
    name: String,
    browser_download_url: String,
}

/// The latest release, if it is newer than `current` and has a package for this platform.
pub async fn check(current: &str) -> Result<Option<Release>> {
    let Some(suffix) = asset_suffix() else {
        return Ok(None);
    };
    let release: ApiRelease = reqwest::get(LATEST_RELEASE_URL)
        .await?
        .error_for_status()?
        .json()
        .await?;
    let version = release.tag_name.trim_start_matches('v').to_string();
    if !is_newer(&version, current) {
        return Ok(None);
    }
    Ok(release
        .assets
        .into_iter()
        .find(|a| a.name.ends_with(suffix))
        .map(|asset| Release {
            version,
            notes: release.body,
            url: asset.browser_download_url,
        }))
}

/// Download and install `release`. Returns the app to start in place of this one.
// ponytail: trusts HTTPS from our Gitea; add package signatures (minisign) if the server is ever shared.
pub async fn install(release: &Release) -> Result<PathBuf> {
    let app = installed_app()?;
    let package = reqwest::get(&release.url)
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    tokio::task::spawn_blocking(move || replace(&app, &package).map(|()| app)).await?
}

/// Start the app that `install` returned. The caller quits right after.
pub fn launch(app: &Path) -> Result<()> {
    // `-n`: this instance is still running.
    #[cfg(target_os = "macos")]
    std::process::Command::new("open")
        .arg("-n")
        .arg(app)
        .spawn()?;
    #[cfg(not(target_os = "macos"))]
    std::process::Command::new(app).spawn()?;
    Ok(())
}

fn is_newer(candidate: &str, current: &str) -> bool {
    version_parts(candidate) > version_parts(current)
}

/// `1.2.3` → `[1, 2, 3]`; a pre-release or build suffix is ignored.
fn version_parts(version: &str) -> [u64; 3] {
    let mut parts = [0; 3];
    let core = version.split(['-', '+']).next().unwrap_or_default();
    for (part, text) in parts.iter_mut().zip(core.split('.')) {
        *part = text.parse().unwrap_or(0);
    }
    parts
}

#[cfg(target_os = "macos")]
fn installed_app() -> Result<PathBuf> {
    // Revisor.app/Contents/MacOS/Revisor
    let exe = std::env::current_exe()?;
    exe.ancestors()
        .nth(3)
        .filter(|app| app.extension().is_some_and(|ext| ext == "app"))
        .map(Path::to_path_buf)
        .context("Updates work only in the packaged app (Revisor.app).")
}

/// Swap the `.app` bundle for the one in the `.tar.gz` package.
#[cfg(target_os = "macos")]
fn replace(app: &Path, package: &[u8]) -> Result<()> {
    let folder = app.parent().context("Revisor.app has no parent folder")?;
    // Unpack next to the app so the swap is two renames on one volume.
    let staging = tempfile::Builder::new()
        .prefix(".revisor-update")
        .tempdir_in(folder)
        .with_context(|| format!("Cannot write to {}", folder.display()))?;
    tar::Archive::new(flate2::read::GzDecoder::new(package)).unpack(staging.path())?;
    let new = staging.path().join("Revisor.app");
    if !new.join("Contents/MacOS/Revisor").is_file() {
        bail!("The update package has no Revisor.app");
    }
    let verified = std::process::Command::new("codesign")
        .args(["--verify", "--strict"])
        .arg(&new)
        .status()?;
    if !verified.success() {
        bail!("The update package has an invalid code signature");
    }
    let old = staging.path().join("old.app");
    std::fs::rename(app, &old)?;
    if let Err(e) = std::fs::rename(&new, app) {
        std::fs::rename(&old, app).ok();
        return Err(e.into());
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn installed_app() -> Result<PathBuf> {
    // After the upgrade, /proc/self/exe ends in " (deleted)"; read it now.
    let exe = std::env::current_exe()?;
    if exe != Path::new("/usr/bin/revisor") {
        bail!("Updates work only in the installed Linux package.");
    }
    Ok(exe)
}

#[cfg(target_os = "linux")]
fn replace(_: &Path, package: &[u8]) -> Result<()> {
    let (suffix, command, args): (_, _, &[&str]) = match linux_package() {
        Some(LinuxPackage::Fedora) => (".rpm", "/usr/bin/dnf", &["install", "-y"]),
        Some(LinuxPackage::Arch) => (".pkg.tar.zst", "/usr/bin/pacman", &["-U", "--noconfirm"]),
        None => bail!("Updates are not available on this Linux distribution."),
    };
    let file = tempfile::Builder::new().suffix(suffix).tempfile()?;
    std::fs::write(file.path(), package)?;
    // pkexec shows the system password prompt; the native package manager installs the update.
    let status = std::process::Command::new("pkexec")
        .arg(command)
        .args(args)
        .arg(file.path())
        .status()
        .context("Cannot start pkexec")?;
    if !status.success() {
        bail!("The update was cancelled or package installation failed ({status})");
    }
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn installed_app() -> Result<PathBuf> {
    bail!("Updates are not available on this platform.")
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn replace(_: &Path, _: &[u8]) -> Result<()> {
    bail!("Updates are not available on this platform.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions() {
        assert!(is_newer("1.0.1", "1.0.0"));
        assert!(is_newer("1.10.0", "1.9.9"));
        assert!(is_newer("2.0.0", "1.99.99"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("0.9.0", "1.0.0"));
        assert!(!is_newer("1.2.0-beta.1", "1.2.0"));
        assert!(is_newer("1.2.0-beta.1", "1.1.0"));
        assert!(!is_newer("not-a-version", "1.0.0"));
    }

    #[test]
    fn selects_linux_package_from_os_release() {
        assert_eq!(
            linux_package_from_os_release("ID=fedora\n"),
            Some(LinuxPackage::Fedora)
        );
        assert_eq!(
            linux_package_from_os_release("ID=arch\n"),
            Some(LinuxPackage::Arch)
        );
        assert_eq!(
            linux_package_from_os_release("ID=cachyos\nID_LIKE=arch\n"),
            Some(LinuxPackage::Arch)
        );
        assert_eq!(
            linux_package_from_os_release("ID=other\nID_LIKE=\"arch linux\"\n"),
            Some(LinuxPackage::Arch)
        );
        assert_eq!(
            linux_package_from_os_release("ID=ubuntu\nID_LIKE=debian\n"),
            None
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn replaces_app_bundle() {
        let dir = tempfile::tempdir().unwrap();
        let write_app = |root: &Path, text: &str| {
            let exe = root.join("Revisor.app/Contents/MacOS/Revisor");
            std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
            std::fs::write(exe, text).unwrap();
        };
        let installed = dir.path().join("Applications");
        write_app(&installed, "old");
        let build = dir.path().join("build");
        write_app(&build, "new");
        let signed = std::process::Command::new("codesign")
            .args(["--force", "--sign", "-"])
            .arg(build.join("Revisor.app"))
            .status()
            .unwrap();
        assert!(signed.success());
        let mut package = tar::Builder::new(flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::fast(),
        ));
        package
            .append_dir_all("Revisor.app", build.join("Revisor.app"))
            .unwrap();
        let package = package.into_inner().unwrap().finish().unwrap();

        let app = installed.join("Revisor.app");
        replace(&app, &package).unwrap();
        assert_eq!(
            std::fs::read_to_string(app.join("Contents/MacOS/Revisor")).unwrap(),
            "new"
        );
        // Only the app is left; the staging folder with the old bundle is gone.
        assert_eq!(std::fs::read_dir(&installed).unwrap().count(), 1);

        assert!(replace(&app, b"not a package").is_err());
        assert_eq!(
            std::fs::read_to_string(app.join("Contents/MacOS/Revisor")).unwrap(),
            "new"
        );
    }
}
