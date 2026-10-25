#!/bin/sh
# Check for scripts/release-notes.sh. Run: sh scripts/test-release-notes.sh
set -eu
cd "$(dirname "$0")"
actual=$(printf '%s\n' \
    'feat(onboarding): add offline sample pull requests' \
    'fix: keep the review position' \
    'perf(diff): color large diffs off the UI thread' \
    'feat(api)!: drop the old token format' \
    'docs: explain first start' \
    'chore: release v1.0.0' \
    'ci: cache cargo' | sh release-notes.sh)
expected='## Features

- Add offline sample pull requests (onboarding)

## Fixes

- Keep the review position
- Color large diffs off the UI thread (diff)

## Breaking Changes

- Drop the old token format (api)

## Other changes

- Explain first start'
[ "$actual" = "$expected" ] || { printf 'Unexpected notes:\n%s\n' "$actual" >&2; exit 1; }
[ "$(printf 'chore: bump\n' | sh release-notes.sh)" = "Maintenance release." ] || { echo "Missing maintenance line" >&2; exit 1; }
echo "release-notes: ok"
