#!/bin/bash
# Update the Homebrew tap (zyvorai/homebrew-tap) for a published release: sets version and sha256 from the
# release's .sha256 files, then copies Formula/macora.rb and Casks/macora.rb into the tap checkout.
# Usage: scripts/update-homebrew.sh VERSION TAP_DIR      e.g. scripts/update-homebrew.sh 0.3.1 ~/src/homebrew-tap
set -euo pipefail
version="${1:?usage: update-homebrew.sh VERSION TAP_DIR}"; tap="${2:?usage: update-homebrew.sh VERSION TAP_DIR}"
root="$(cd "$(dirname "$0")/.." && pwd)"
base="https://github.com/zyvorai/zyvor-macora/releases/download/v$version"
sha() { curl -fsSL "$base/$1.sha256" | awk '{print $1}'; }
tar_sha="$(sha "macora-$version-arm64-apple-darwin.tar.gz")"; dmg_sha="$(sha "Macora-$version.dmg")"
[[ ${#tar_sha} -eq 64 && ${#dmg_sha} -eq 64 ]] || { echo "could not read checksums for v$version" >&2; exit 1; }
mkdir -p "$tap/Formula" "$tap/Casks"
# Formula: the version lives only in the url. Cask: in the version stanza.
sed -E "/^  url /s/[0-9]+\.[0-9]+\.[0-9]+/$version/g; s/^(  sha256 )\".*\"/\1\"$tar_sha\"/" "$root/packaging/homebrew/Formula-macora.rb" > "$tap/Formula/macora.rb"
sed -E "s/^(  version )\".*\"/\1\"$version\"/; s/^(  sha256 )\".*\"/\1\"$dmg_sha\"/" "$root/packaging/homebrew/Cask-macora.rb" > "$tap/Casks/macora.rb"
echo "Updated $tap to MACORA $version (formula $tar_sha, cask $dmg_sha)"
