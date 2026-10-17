#!/bin/bash
# Install the macora CLI (and man page + completions) from a source checkout. Does not need sudo with PREFIX set.
# Usage: scripts/install-macos.sh [PREFIX]   (default /usr/local)
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
prefix="${1:-/usr/local}"
cd "$root"
cargo build --release --locked
install -d "$prefix/bin" "$prefix/share/man/man1" "$prefix/share/zsh/site-functions" "$prefix/share/bash-completion/completions" "$prefix/share/fish/vendor_completions.d"
install -m 755 target/release/macora "$prefix/bin/macora"
install -m 644 man/macora.1 "$prefix/share/man/man1/macora.1"
install -m 644 completions/_macora "$prefix/share/zsh/site-functions/_macora"
install -m 644 completions/macora.bash "$prefix/share/bash-completion/completions/macora"
install -m 644 completions/macora.fish "$prefix/share/fish/vendor_completions.d/macora.fish"
echo "Installed macora to $prefix/bin. Try: macora diagnose"
