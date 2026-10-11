#!/bin/sh
# bump-version.sh - bump every liber version string in one command.
#
#   bash dist/bump-version.sh X.Y.Z [--code N]
#
# Edits Cargo.toml, tauri.conf.json, frontend/package.json, and
# android-version.properties (auto-incremented unless --code is given),
# then runs local/check-release.sh to verify. Edits files only: it never
# commits or tags. Requires GNU sed.
set -eu

fail() {
    echo "bump-version: FAIL: $1" >&2
    exit 1
}

[ $# -ge 1 ] || fail "usage: bash dist/bump-version.sh X.Y.Z [--code N]"
VERSION=$1
CODE=""
shift
while [ $# -gt 0 ]; do
    case "$1" in
        --code)
            [ $# -ge 2 ] || fail "--code needs a value"
            CODE=$2
            shift 2
            ;;
        *) fail "unknown argument: $1" ;;
    esac
done
case "$VERSION" in
    v* | *[!0-9.]* | "" | .* | *.) fail "version must be plain X.Y.Z, got '$VERSION'" ;;
esac

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"

if [ -z "$CODE" ]; then
    CUR=$(grep -E '^tauri\.android\.versionCode=' crates/liber-tauri/android-version.properties | cut -d= -f2 | tr -d ' ' || true)
    case "$CUR" in '' | *[!0-9]*) fail "cannot auto-increment: no numeric versionCode line" ;; esac
    CODE=$((CUR + 1))
fi
case "$CODE" in '' | *[!0-9]*) fail "--code must be an integer, got '$CODE'" ;; esac

sed -i "s/^version = \".*\"$/version = \"$VERSION\"/" Cargo.toml
sed -i "s/^\(\s*\"version\": \"\).*\(\".*\)$/\1$VERSION\2/" crates/liber-tauri/tauri.conf.json
sed -i "s/^\(\s*\"version\": \"\).*\(\".*\)$/\1$VERSION\2/" frontend/package.json
sed -i "s/^tauri\.android\.versionCode=.*/tauri.android.versionCode=$CODE/" crates/liber-tauri/android-version.properties

python3 -c "import json; json.load(open('crates/liber-tauri/tauri.conf.json')); json.load(open('frontend/package.json')); print('bump-version: JSON valid')"
grep -n "^version = \"$VERSION\"$" Cargo.toml | head -n 1
grep -n "\"version\": \"$VERSION\"" crates/liber-tauri/tauri.conf.json frontend/package.json
grep -n "^tauri.android.versionCode=$CODE\$" crates/liber-tauri/android-version.properties

bash local/check-release.sh "$VERSION"
echo "bump-version: done. Review with: git diff --stat"
echo "bump-version: next: git commit, git tag v$VERSION, git push origin main v$VERSION"
