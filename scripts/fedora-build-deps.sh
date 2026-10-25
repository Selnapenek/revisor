#!/bin/sh
# Installs the Fedora packages and Rust toolchain that the RPM build needs. Run as root.
set -eu

dnf -q install -y --setopt=install_weak_deps=False \
    clang cmake curl desktop-file-utils fontconfig-devel gcc gcc-c++ git \
    libX11-devel libxkbcommon-x11-devel make openssl-devel \
    pkgconf-pkg-config rpm-build tar vulkan-loader-devel \
    wayland-devel webkit2gtk4.1-devel libzstd-devel
dnf clean all

curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal
chmod -R a+rX "$CARGO_HOME" "$RUSTUP_HOME"
