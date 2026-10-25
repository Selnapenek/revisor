#!/bin/sh
# Build an x86_64 Arch package on a Docker host.
set -eu
cd "$(dirname "$0")/.."

command -v docker >/dev/null || { echo "Docker is required to build the Arch package." >&2; exit 1; }
mkdir -p target/dist target/arch-home

docker build --platform linux/amd64 -f scripts/Dockerfile.arch -t revisor-arch-builder:latest .
docker run --rm --platform linux/amd64 \
    --user "$(id -u):$(id -g)" \
    -e HOME=/work/target/arch-home \
    -e CARGO_HOME=/work/target/arch-home/.cargo \
    -e CARGO_BUILD_JOBS=2 \
    -e CARGO_TARGET_DIR=/work/target/arch-x86_64 \
    -e REVISOR_GITHUB_CLIENT_ID \
    -v "$PWD:/work" -w /work revisor-arch-builder:latest \
    sh scripts/package-arch-in-container.sh
