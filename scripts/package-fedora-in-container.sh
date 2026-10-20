#!/bin/sh
set -eu

version=$(sed -n 's/^version = "\(.*\)"/\1/p' crates/revisor-app/Cargo.toml)
[ -n "$version" ] || { echo "Cannot read Revisor version." >&2; exit 1; }

desktop-file-validate scripts/revisor.desktop
cargo build --locked --release -p revisor-app

topdir="$PWD/target/fedora-rpm"
mkdir -p "$topdir/RPMS" "$topdir/BUILD" "$topdir/BUILDROOT"
rpmbuild -bb scripts/revisor-fedora.spec \
    --define "_topdir $topdir" \
    --define "revisor_version $version" \
    --define "revisor_root $PWD"

rpm_file=$(find "$topdir/RPMS/x86_64" -maxdepth 1 -name "revisor-$version-*.rpm" -print -quit)
[ -n "$rpm_file" ] || { echo "RPM build did not produce an x86_64 package." >&2; exit 1; }
cp "$rpm_file" target/dist/
echo "Built target/dist/$(basename "$rpm_file")"
