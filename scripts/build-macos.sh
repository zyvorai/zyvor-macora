#!/bin/bash
set -euo pipefail
if [[ "$(uname -s)" != "Darwin" ]]; then
  echo 'Macora.app must be built on macOS with Xcode command line tools.' >&2
  exit 1
fi
project_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$project_root"
command -v cargo >/dev/null || { echo 'Install the Rust toolchain first.' >&2; exit 1; }
cargo test --locked --offline
cargo build --release --locked --offline
swift test --package-path macos
swift build --package-path macos -c release
swift_bin="$(swift build --package-path macos -c release --show-bin-path)"
app="$project_root/dist/Macora.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$swift_bin/Macora" "$app/Contents/MacOS/Macora"
cp target/release/macora "$app/Contents/Resources/macora"
cp macos/Info.plist "$app/Contents/Info.plist"
cp macos/demo.json "$app/Contents/Resources/demo.json"
iconset="$project_root/dist/Macora.iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" macos/Assets/AppIcon.png --out "$iconset/icon_${size}x${size}.png" >/dev/null
  retina=$((size * 2))
  sips -z "$retina" "$retina" macos/Assets/AppIcon.png --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/Macora.icns"
if [[ -n "${MACORA_SIGN_IDENTITY:-}" ]]; then
  codesign --force --options runtime --timestamp --sign "$MACORA_SIGN_IDENTITY" "$app/Contents/Resources/macora"
  codesign --force --options runtime --timestamp --entitlements macos/Macora.entitlements --sign "$MACORA_SIGN_IDENTITY" "$app"
else
  codesign --force --sign - "$app/Contents/Resources/macora"
  codesign --force --sign - "$app"
fi
codesign --verify --strict "$app"
echo "Built $app"
echo 'Open with: open dist/Macora.app'
echo 'Demo mode: open dist/Macora.app --args --demo'
