#!/bin/sh
# Usage: ./scripts/release.sh 1.2.3
# Sets the app version, commits, and tags v1.2.3. Push the tag to start the release build.
set -eu
cd "$(dirname "$0")/.."

version=${1:?Usage: ./scripts/release.sh <version>, for example 1.2.3}
echo "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' || { echo "Version must look like 1.2.3." >&2; exit 1; }
[ -z "$(git status --porcelain)" ] || { echo "Commit or stash your changes first." >&2; exit 1; }

for manifest in crates/revisor-app/Cargo.toml crates/revisor-core/Cargo.toml; do
    sed -i.bak "s/^version = \".*\"/version = \"$version\"/" "$manifest" && rm "$manifest.bak"
done
cargo update --workspace --quiet

git commit --quiet -am "chore: release v$version"
git tag "v$version"
echo "Tagged v$version. Start the release with: git push origin HEAD v$version"
