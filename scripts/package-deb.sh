#!/bin/sh
# Builds target/dist/revisor_<version>_<arch>.deb for this machine's architecture.
# The release workflow runs it in ubuntu:22.04, so the package works on Ubuntu 22.04+ and Debian 12+.
set -eu
cd "$(dirname "$0")/.."

version=$(sed -n 's/^version = "\(.*\)"/\1/p' crates/revisor-app/Cargo.toml)
[ -n "$version" ] || { echo "Cannot read Revisor version." >&2; exit 1; }
arch=$(dpkg --print-architecture)

desktop-file-validate scripts/revisor.desktop
cargo build --locked --release -p revisor-app
bin="${CARGO_TARGET_DIR:-$PWD/target}/release/revisor"

root=target/deb/revisor_${version}_${arch}
rm -rf "$root"
install -Dm755 "$bin" "$root/usr/bin/revisor"
install -Dm644 scripts/revisor.desktop "$root/usr/share/applications/revisor.desktop"
install -Dm644 brand/icon.svg "$root/usr/share/icons/hicolor/scalable/apps/revisor.svg"
install -Dm644 LICENSE.md "$root/usr/share/doc/revisor/copyright"

# Libraries the binary links to, plus the ones GPUI opens at run time.
shlibs_dir=$(mktemp -d)
mkdir "$shlibs_dir/debian"
printf 'Source: revisor\n\nPackage: revisor\nArchitecture: any\n' > "$shlibs_dir/debian/control"
installed_bin="$PWD/$root/usr/bin/revisor"
linked=$(cd "$shlibs_dir" && dpkg-shlibdeps --ignore-missing-info -O "$installed_bin" | sed -n 's/^shlibs:Depends=//p')
rm -rf "$shlibs_dir"
[ -n "$linked" ] || { echo "dpkg-shlibdeps found no libraries." >&2; exit 1; }

mkdir -p "$root/DEBIAN"
cat > "$root/DEBIAN/control" <<CONTROL
Package: revisor
Version: $version
Architecture: $arch
Maintainer: Viacheslav Solomin <Selnapenek@users.noreply.github.com>
Homepage: https://github.com/Selnapenek/revisor
Section: devel
Priority: optional
Depends: $linked, libvulkan1, libwayland-client0, libxkbcommon-x11-0
Recommends: pkexec | policykit-1
Description: AI code review that lands on the diff
 Native GitHub pull request review. Runs your Claude Code or Codex CLI and
 shows each finding on its line.
CONTROL

mkdir -p target/dist
dpkg-deb --build -Zxz --root-owner-group "$root" "target/dist/revisor_${version}_${arch}.deb"
