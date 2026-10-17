#!/bin/bash
# Build release artifacts into release/: CLI tarball, Macora.app DMG and pkg, with .sha256 files.
# Usage: scripts/package-mac.sh VERSION
# Env: MACORA_SIGN_IDENTITY (Developer ID Application), DEVELOPER_ID_INSTALLER, NOTARY_PROFILE.
# Without signing identities the artifacts are ad-hoc signed development builds and are NOT notarized.
set -euo pipefail
version="${1:?usage: scripts/package-mac.sh VERSION}"
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
[[ "$(uname -s)" == Darwin ]] || { echo 'Packaging requires macOS.' >&2; exit 1; }
grep -q "^version = \"$version\"" Cargo.toml || { echo "Cargo.toml version is not $version" >&2; exit 1; }
out="$root/release"; rm -rf "$out"; mkdir -p "$out"

# CLI: universal when both Apple targets are installed, otherwise the host architecture.
installed="$(rustup target list --installed 2>/dev/null || true)"
if grep -q aarch64-apple-darwin <<<"$installed" && grep -q x86_64-apple-darwin <<<"$installed"; then
  cargo build --release --locked --target aarch64-apple-darwin
  cargo build --release --locked --target x86_64-apple-darwin
  mkdir -p target/universal
  lipo -create -output target/universal/macora target/aarch64-apple-darwin/release/macora target/x86_64-apple-darwin/release/macora
  cli=target/universal/macora; arch=universal
else
  cargo build --release --locked; cli=target/release/macora; arch="$(uname -m)"
fi
if [[ -n "${MACORA_SIGN_IDENTITY:-}" ]]; then
  codesign --force --options runtime --timestamp --sign "$MACORA_SIGN_IDENTITY" "$cli"
else codesign --force --sign - "$cli"; fi
stage="$(mktemp -d)"; trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/macora-$version/man" "$stage/macora-$version/completions"
cp "$cli" "$stage/macora-$version/macora"
cp README.md LICENSE NOTICE "$stage/macora-$version/"
cp man/macora.1 "$stage/macora-$version/man/"
cp completions/* "$stage/macora-$version/completions/"
tar -C "$stage" -czf "$out/macora-$version-$arch-apple-darwin.tar.gz" "macora-$version"

# Native app (also runs Rust and Swift tests).
./scripts/build-macos.sh
app="$root/dist/Macora.app"
mkdir -p "$stage/dmg"; ditto "$app" "$stage/dmg/Macora.app"; ln -s /Applications "$stage/dmg/Applications"
hdiutil create -volname "Macora $version" -srcfolder "$stage/dmg" -ov -format UDZO "$out/Macora-$version.dmg" >/dev/null
# Sign the disk image itself so Gatekeeper accepts it (the app inside is already signed).
if [[ -n "${MACORA_SIGN_IDENTITY:-}" ]]; then codesign --force --timestamp --sign "$MACORA_SIGN_IDENTITY" "$out/Macora-$version.dmg"; fi
# The postinstall script links /usr/local/bin/macora to the CLI bundled inside the app.
pkg_args=(--component "$app" --install-location /Applications --version "$version" --scripts packaging/pkg/scripts)
if [[ -n "${DEVELOPER_ID_INSTALLER:-}" ]]; then pkg_args+=(--sign "$DEVELOPER_ID_INSTALLER" --timestamp); fi
pkgbuild "${pkg_args[@]}" "$out/Macora-$version.pkg" >/dev/null
if [[ -n "${NOTARY_PROFILE:-}" && -n "${MACORA_SIGN_IDENTITY:-}" ]]; then
  to_notarize=("$out/Macora-$version.dmg")
  # An unsigned pkg cannot be notarized.
  if [[ -n "${DEVELOPER_ID_INSTALLER:-}" ]]; then to_notarize+=("$out/Macora-$version.pkg"); else echo "pkg is unsigned (no DEVELOPER_ID_INSTALLER): skipping its notarization."; fi
  for f in "${to_notarize[@]}"; do
    xcrun notarytool submit "$f" --keychain-profile "$NOTARY_PROFILE" --wait
    xcrun stapler staple "$f"
  done
  echo "Notarized and stapled."
else
  echo "NOT notarized: users must approve the first launch in System Settings > Privacy & Security."
fi
( cd "$out" && for f in *; do shasum -a 256 "$f" > "$f.sha256"; done )
ls -la "$out"
