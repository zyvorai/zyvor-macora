#!/bin/bash
# Mirror the wiki/ folder into the GitHub wiki (zyvorai/zyvor-macora.wiki). The repo is the source of truth.
# Usage: scripts/sync-wiki.sh   (needs push access; the wiki must have at least one page created in the browser)
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
git clone -q git@github.com:zyvorai/zyvor-macora.wiki.git "$work"
find "$work" -maxdepth 1 -name '*.md' -delete
cp "$root"/wiki/*.md "$work"/
cd "$work"
git add -A
if git diff --cached --quiet; then echo "wiki already up to date"; exit 0; fi
git commit -q -m "Sync wiki from the repository"
git push -q origin HEAD
echo "wiki updated"
