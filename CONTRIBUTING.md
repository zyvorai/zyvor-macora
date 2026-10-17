# Contributing

Thanks for helping make MACORA better. Bug reports, fixtures from other macOS versions, and focused pull requests are all welcome.

## Principles

- **Read-only by default.** Monitoring and diagnostics never change settings. Anything that mutates needs typed validation, a preview, explicit authorization and plan tests that do not touch the developer's machine.
- **Zero dependencies.** No external Rust crates or Swift packages. Apple system frameworks are fine.
- **Honest output.** Never invent data (missing counters stay "unavailable"), never claim VPN ownership or internet health from interface state alone.
- Preserve the JSON schema or add an explicit version migration. Use public macOS APIs and fixed executable paths with argument arrays, never a shell string.
- Code is contributed under Apache 2.0. Do not copy code from BUSL or other incompatible sources.

## Set up

```sh
git clone https://github.com/zyvorai/zyvor-macora.git && cd zyvor-macora
cargo test --locked
swift test --package-path macos
./scripts/build-macos.sh        # builds dist/Macora.app
open dist/Macora.app --args --demo
```

## Before you open a PR

```sh
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
python3 scripts/check-contract.py && python3 scripts/test-tui-pty.py
cargo build --release --locked && scripts/smoke-cli.sh
swift test --package-path macos
```

Describe which Mac/OS workflows you actually exercised (see [docs/TESTING.md](docs/TESTING.md)). Add a test for new behavior; parsers are tested against sanitized fixtures in `tests/fixtures/`. **Sanitize fixtures**: replace real IPs, MACs, SSIDs and hostnames before committing.

UI changes: check light and dark, and attach a screenshot made with `scripts/shots.sh` (simulated data only).
