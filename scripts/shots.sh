#!/bin/bash
# Capture documentation screenshots of the app running on its built-in simulated data.
# Usage: scripts/shots.sh   (macOS, needs Screen Recording permission for the terminal)
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
app="$root/dist/Macora.app"
out="$root/docs/ux"
mkdir -p "$out"
[[ -d "$app" ]] || { echo "Build first: ./scripts/build-macos.sh" >&2; exit 1; }
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"; pkill -x Macora 2>/dev/null || true' EXIT
cat > "$tmp/winid.swift" <<'S'
import CoreGraphics
import Foundation
let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
for w in list where (w[kCGWindowOwnerName as String] as? String) == "MACORA" && (w[kCGWindowLayer as String] as? Int) == 0 {
    let b = w[kCGWindowBounds as String] as? [String: Any] ?? [:]
    if ((b["Width"] as? NSNumber)?.doubleValue ?? 0) > 400, ((b["Height"] as? NSNumber)?.doubleValue ?? 0) > 400 { print(w[kCGWindowNumber as String] as? Int ?? 0); break }
}
S
shot() { # shot <page> <appearance> <file>
  pkill -x Macora 2>/dev/null || true; sleep 1
  defaults write dev.zyvor.macora macora.appearance "$2"
  defaults write dev.zyvor.macora macora.onboarded -bool "$4"
  open -n "$app" --args --demo --page "$1"
  sleep 6
  caffeinate -u -t 2 || true
  osascript -e 'tell application id "dev.zyvor.macora" to activate' >/dev/null 2>&1 || true
  sleep 2
  id="$(swiftc -O -o "$tmp/winid" "$tmp/winid.swift" 2>/dev/null; "$tmp/winid")"
  [[ -n "$id" ]] || { echo "no window for $1" >&2; return 1; }
  screencapture -x -o -l "$id" "$out/$3"; echo "wrote docs/ux/$3"
}
shot Overview dark   native-dark-overview.png false
shot Overview light  native-light-overview.png true
shot Diagnostics light native-light-diagnostics.png true
shot Diagnostics dark  native-dark-diagnostics.png true
shot Interfaces light  native-light-interfaces.png true
shot Interfaces dark   native-dark-interfaces.png true
defaults write dev.zyvor.macora macora.appearance system
