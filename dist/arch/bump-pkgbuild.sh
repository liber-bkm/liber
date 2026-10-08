#!/usr/bin/env bash
# Refresh pkgver plus sha256sums in both Arch packages for a release.
# Usage: ./bump-pkgbuild.sh 0.2.0
# Needs: curl, sha256sum. Run from anywhere; edits dist/arch in place.
set -euo pipefail

ver="${1:?usage: bump-pkgbuild.sh <version, no leading v>}"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
base="https://github.com/liber-bkm/liber/releases/download/v$ver"
raw="https://raw.githubusercontent.com/liber-bkm/liber/v$ver"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

sha_of_url() {
  curl -sSL --fail "$1" -o "$tmp/f" && sha256sum "$tmp/f" | cut -d' ' -f1
}

echo "fetching release assets for v$ver..."
tarball_sum="$(sha_of_url "$base/liber-rs-$ver-linux-x86_64.tar.gz")"
desktop_sum="$(sha_of_url "$raw/dist/arch/liber.desktop")"
icon32_sum="$(sha_of_url "$raw/crates/liber-tauri/icons/32x32.png")"
icon128_sum="$(sha_of_url "$raw/crates/liber-tauri/icons/128x128.png")"
srctar_sum="$(sha_of_url "https://github.com/liber-bkm/liber/archive/refs/tags/v$ver.tar.gz")"

set_version_and_sums() {
  python3 - "$1" "$2" "$3" "$4" "$5" <<'EOF'
import re, sys
path, ver, sums = sys.argv[1], sys.argv[2], sys.argv[3:]
text = open(path).read()
text = re.sub(r"^pkgver=.*$", f"pkgver={ver}", text, count=1, flags=re.M)
lines = text.split("\n")
out, i, n = [], 0, len(sums)
for line in lines:
    if i < n and "'SKIP'" in line:
        line = line.replace("'SKIP'", f"'{sums[i]}'", 1)
        i += 1
    elif i < n and re.search(r"'[0-9a-f]{64}'", line):
        line = re.sub(r"'[0-9a-f]{64}'", f"'{sums[i]}'", line, count=1)
        i += 1
    out.append(line)
assert i == n, f"expected {n} checksum slots, filled {i}"
open(path, "w").write("\n".join(out))
EOF
}

set_version_and_sums "$here/liber-bin/PKGBUILD" "$ver" \
  "$tarball_sum" "$desktop_sum" "$icon32_sum" "$icon128_sum"
set_version_and_sums "$here/liber/PKGBUILD" "$ver" "$srctar_sum"
echo "dist/arch PKGBUILDs now at $ver"
