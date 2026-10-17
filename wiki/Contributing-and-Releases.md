# Contributing and Releases

## Principles

Read-only by default; zero external dependencies; honest output (never invent data); preserve the JSON schema; Apache 2.0.

## Before a pull request

```sh
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
python3 scripts/check-contract.py && python3 scripts/test-tui-pty.py
cargo build --release --locked && scripts/smoke-cli.sh
swift test --package-path macos
python3 scripts/verify-cli.py target/release/macora && python3 scripts/verify-diagnose.py target/release/macora   # live network
```

Sanitize any fixture you add: replace real IPs, MACs, SSIDs and hostnames.

## How it is tested

- Pure parts (parsers, the path-MTU search, the analyser, the renderers) are unit tested against recorded, sanitized output; the collector runs against a fake runner.
- The path-MTU search is exact for every MTU from 576 to 1500.
- About 70 scenarios break a healthy baseline one way at a time (no link, router ignores ping, dead DNS, captive portal, PMTU 1492/1480/1400, weak Wi-Fi, and more).
- Mutation checks: deliberately breaking 16 behaviours must fail the suite.
- Live, read-only checks compare the tool with `ifconfig`, `netstat`, `ping`, `dig`, `curl` and `system_profiler`.
- Released files are downloaded back and checked: checksums, notarization, signature, version, and a fresh Homebrew install of both the formula and the cask.

Details and results: [docs/TESTING.md](https://github.com/zyvorai/zyvor-macora/blob/main/docs/TESTING.md).

## Releasing

Package, sign and notarize on a Mac with the Developer ID keys, verify the downloaded files, publish the GitHub release, then update the Homebrew tap and the site. The full checklist, including the pitfalls, is in [docs/RELEASE.md](https://github.com/zyvorai/zyvor-macora/blob/main/docs/RELEASE.md).
