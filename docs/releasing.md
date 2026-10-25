# Releasing

A tag like `v1.2.3` starts [the release workflow](../.github/workflows/release.yml) on GitHub Actions. It builds the macOS, Fedora, and Arch packages. When all builds pass, it publishes a [GitHub release](https://github.com/Selnapenek/revisor/releases) with all packages.

## Make a release

```sh
./scripts/release.sh 1.2.3
git push origin HEAD v1.2.3
```

`release.sh` sets the version in both crates, updates `Cargo.lock`, commits, and tags. The workflow stops if the tag and the `revisor-app` version are different. The release notes are the commit subjects since the previous tag.

If a run fails, delete the tag (`git push origin :v1.2.3` and `git tag -d v1.2.3`), fix the cause, then tag again. The workflow makes the release only at the end, so a failed run leaves no release.

## One-time setup

1. In **Settings > Secrets and variables > Actions > Secrets**, add:
   - `REVISOR_GITHUB_CLIENT_ID`. It is baked into the packages.
   - `MINISIGN_SECRET_KEY`: the release signing key. Its public half is `RELEASE_PUBLIC_KEY` in `crates/revisor-core/src/update.rs`. The owner keeps a backup in `~/.config/revisor-release/`. If the key is lost, installed apps cannot verify new updates; users must then install the next version by hand.
2. Optional: in **Variables**, set `REVISOR_SIGNING_IDENTITY` to sign the macOS app with a Developer ID certificate. GitHub-hosted runners do not have your certificate, so this needs a step that imports it. Without it, the app has an ad hoc signature.

Runners: macOS on `macos-15` (one universal binary for Apple Silicon and Intel), Windows on `windows-2025` with the preinstalled Inno Setup, and Linux in `fedora:43`, `ubuntu:22.04`, and `archlinux:latest` containers on `ubuntu-latest` and `ubuntu-24.04-arm`. Public repositories do not pay for these runners.

To test the workflow without a release, open **Actions > Release > Run workflow**. It builds and signs every package and keeps them as the `release-dry-run` artifact.

## Release notes

`scripts/release-notes.sh` groups the commit subjects since the previous tag: `feat` under Features, `fix` and `perf` under Fixes, `type!:` under Breaking Changes, and the rest under Other changes. `chore`, `ci`, `test`, and `style` commits are left out. Write commit subjects for users, because they become the notes. Check the script with `sh scripts/test-release-notes.sh`.

## Release assets

| File | Use |
| --- | --- |
| `Revisor-<version>-macos-universal.dmg` | First install on macOS |
| `Revisor-<version>-macos-universal.tar.gz` | In-app update on macOS |
| `Revisor-<version>-x64-setup.exe` | First install and in-app update on Windows |
| `revisor_<version>_amd64.deb`, `revisor_<version>_arm64.deb` | First install and in-app update on Ubuntu and Debian |
| `revisor-<version>-1.fc43.x86_64.rpm`, `revisor-<version>-1.fc43.aarch64.rpm` | First install and in-app update on Fedora |
| `revisor-<version>-1-x86_64.pkg.tar.zst` | First install and in-app update on Arch and CachyOS |
| `<file>.sig` | minisign signature of each file. The updater installs only packages with a valid signature |
| `SHA256SUMS` | SHA-256 checksums of all packages |

The in-app updater finds its package by the end of the file name and its signature by the `.sig` ending. Do not rename these files.

## In-app updates

Revisor reads `https://api.github.com/repos/Selnapenek/revisor/releases/latest` when it starts and every 6 hours. **Revisor > Check for Updates…** checks now. A newer version opens a prompt one time and shows **Update to x.y.z** in the title bar. **Update and restart** downloads the package, installs it, and starts the new version:

- macOS: the updater replaces `Revisor.app` where it is installed. The folder must be writable, which is usual for `/Applications`.
- Fedora: `pkexec dnf install` asks for the system password, then upgrades the RPM.
- Arch and CachyOS: `pkexec pacman -U` asks for the system password, then upgrades the package.
- Ubuntu and Debian: `pkexec apt-get install` asks for the system password, then upgrades the package.
- Windows: the updater renames the running `Revisor.exe`, runs the new installer silently, and starts the new version. It needs no administrator.

Updates work only in the packaged app, not in `cargo run`. Drafts, pre-releases, and releases without signatures are not offered.
