#!/bin/sh
# Install tools for the x86_64 Arch package build. Run as root in an Arch container.
set -eu

pacman -Syu --noconfirm --needed \
    base-devel clang cmake curl desktop-file-utils fontconfig git \
    libx11 libxkbcommon libxkbcommon-x11 openssl pkgconf \
    rust vulkan-headers vulkan-icd-loader wayland webkit2gtk-4.1 zstd
