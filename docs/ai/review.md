# AI review (zyvor-macora)

_Generated 2026-10-20 by claude-opus-5-5 from a snapshot of tracked sources; verify before acting._

# Code review: zyvor-macora

## Summary

MACORA is a macOS network-inspection tool with three front ends. The core is a zero-dependency Rust CLI (`macora`) that parses `ifconfig`/`netstat`/`networksetup` output, runs a layered `diagnose` walk (link → router → internet → DNS → HTTPS → path MTU → Wi-Fi/IPv6), and includes a REPL and a TUI. On top of it sits a SwiftUI app that shells out to the bundled CLI. Settings changes go through validated `CommandPlan`s that are executed via `osascript … with administrator privileges`. Packaging covers a DMG, a pkg, a tarball and a Homebrew formula and cask.

## Bugs and correctness risks

**Release workflow can overwrite notarized assets.** In `.github/workflows/release.yml`, the Publish step protects existing installers only when `HAVE_CERTS != 'true'`. If certificates are configured but notary secrets are not (`HAVE_NOTARY` false), the job runs `gh release upload --clobber`. That replaces previously notarized DMG/pkg files with signed but un-notarized ones. The guard should require both `HAVE_CERTS` and `HAVE_NOTARY`.

**CI-built tarball name does not match what Homebrew and the site expect.**
- `release.yml` installs both `aarch64-apple-darwin` and `x86_64-apple-darwin` targets.
- With both installed, `scripts/package-mac.sh` sets `arch=universal`, producing `macora-X-universal-apple-darwin.tar.gz`.
- `scripts/update-homebrew.sh`, `packaging/homebrew/Formula-macora.rb` and `site/index.html` all hard-code `…-arm64-apple-darwin.tar.gz`.
- Result: a CI-built release breaks the formula update and the download links.
- The formula also declares `depends_on arch: :arm64`, which is inconsistent with a universal binary.

**Substring-based redaction can corrupt output or leak digits.** `src/diagnose/render.rs::redact` and `src/view.rs::redact` use plain `str::replace` with no address boundaries.
- A secret `10.0.0.1` also matches inside a non-secret `10.0.0.13`, yielding `<IPv4>3`.
- `fe80::1` (kept because `form.len() > 2` in `facts.rs::secrets`) matches inside longer link-local addresses.
- Sorting longest-first only helps when *both* strings are secrets.

**Swift redaction regex over-matches.** In `macos/Sources/MacoraCore/Diagnosis.swift`, the `DiagnosisPrivacy.ipv6` regex `(?:[0-9A-Fa-f]{0,4}:){2,7}…` also matches clock times (`12:30:45`) and MAC addresses. Those get labelled `<IPv6>`.

**IPv6 path-MTU probing may be inaccurate (needs confirmation).** `src/probe.rs::ping_command` relies on
