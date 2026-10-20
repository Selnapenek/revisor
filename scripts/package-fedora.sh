#!/bin/sh
# Build a Fedora x86_64 RPM from macOS or another Docker host.
set -eu
cd "$(dirname "$0")/.."

command -v docker >/dev/null || { echo "Docker is required to build the Fedora RPM." >&2; exit 1; }
mkdir -p target/dist target/fedora-home target/fedora-cargo

docker build --platform linux/amd64 -f scripts/Dockerfile.fedora -t revisor-fedora-builder:43 .
docker run --rm --platform linux/amd64 \
    --user "$(id -u):$(id -g)" \
    -e HOME=/work/target/fedora-home \
    -e CARGO_HOME=/work/target/fedora-cargo \
    -e RUSTUP_HOME=/opt/rustup \
    -e CARGO_BUILD_JOBS=2 \
    -e CARGO_TARGET_DIR=/work/target/fedora-x86_64 \
    -e REVISOR_GITHUB_CLIENT_ID \
    -v "$PWD:/work" -w /work revisor-fedora-builder:43 \
    sh scripts/package-fedora-in-container.sh
