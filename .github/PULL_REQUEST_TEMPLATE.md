## What and why

## Checklist

- [ ] `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` pass
- [ ] `swift test --package-path macos` passes; UI changes checked in light and dark
- [ ] `scripts/smoke-cli.sh` passes; no command mutates settings without `--apply` / a confirmed preview
- [ ] Still zero external Rust crates and Swift packages
- [ ] No real IPs, MACs, SSIDs or passwords in code, tests, fixtures or images
- [ ] `CHANGELOG.md` updated
