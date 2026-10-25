#!/bin/sh
# Groups conventional commit subjects (one per line on stdin) into Markdown release notes.
# Usage: git log --no-merges --format=%s v1.0.0..v1.1.0 | scripts/release-notes.sh
# Sections: Features (feat), Fixes (fix, perf), Breaking Changes (any `type!:`), Other changes.
# chore, ci, test, and style commits are left out.
set -eu
awk '
function item(subject,   text, scope) {
    text = subject
    sub(/^[a-z]+(\([^)]*\))?!?: */, "", text)
    scope = ""
    if (match(subject, /^[a-z]+\([^)]*\)/)) {
        scope = substr(subject, 1, RLENGTH)
        sub(/^[a-z]+\(/, "", scope)
        sub(/\)$/, "", scope)
    }
    text = toupper(substr(text, 1, 1)) substr(text, 2)
    return "- " text (scope != "" ? " (" scope ")" : "") "\n"
}
/^[a-z]+(\([^)]*\))?!:/ { breaking = breaking item($0); next }
/^feat(\([^)]*\))?:/ { features = features item($0); next }
/^(fix|perf)(\([^)]*\))?:/ { fixes = fixes item($0); next }
/^(chore|ci|test|style)(\([^)]*\))?:/ { next }
NF { other = other item($0) }
END {
    if (features != "") printf "## Features\n\n%s\n", features
    if (fixes != "") printf "## Fixes\n\n%s\n", fixes
    if (breaking != "") printf "## Breaking Changes\n\n%s\n", breaking
    if (other != "") printf "## Other changes\n\n%s\n", other
    if (features fixes breaking other == "") printf "Maintenance release.\n\n"
}'
