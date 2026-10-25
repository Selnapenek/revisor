#!/bin/sh
# Installs the Ubuntu packages and Rust toolchain that the .deb build needs. Run as root in ubuntu:22.04.
set -eu

export DEBIAN_FRONTEND=noninteractive
apt-get -qq update
apt-get -qq install -y --no-install-recommends \
    build-essential ca-certificates clang cmake curl desktop-file-utils dpkg-dev git \
    libfontconfig-dev libssl-dev libvulkan-dev libwayland-dev libx11-dev libx11-xcb-dev \
    libxkbcommon-x11-dev libzstd-dev pkg-config
rm -rf /var/lib/apt/lists/*

curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal
