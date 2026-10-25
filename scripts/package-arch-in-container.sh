#!/bin/sh
# Build and package from an Arch container as an unprivileged user.
set -eu

version=$(sed -n 's/^version = "\(.*\)"/\1/p' crates/revisor-app/Cargo.toml)
[ -n "$version" ] || { echo "Cannot read Revisor version." >&2; exit 1; }

desktop-file-validate scripts/revisor.desktop
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/target/arch-x86_64}"
cargo build --locked --release -p revisor-app

export REVISOR_ROOT="$PWD" PKGDEST="$PWD/target/dist"
mkdir -p "$PKGDEST" target/arch-package
cp scripts/PKGBUILD.arch target/arch-package/PKGBUILD
cd target/arch-package
makepkg --force --nodeps --noconfirm

package="$PKGDEST/revisor-$version-1-x86_64.pkg.tar.zst"
[ -f "$package" ] || { echo "Arch package was not created." >&2; exit 1; }
echo "Built $package"
