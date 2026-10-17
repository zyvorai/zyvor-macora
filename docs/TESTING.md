# Validation and hardware acceptance

## Executed for 0.3.0

Run on an Apple silicon Mac (macOS 27.x, Rust 1.98, Swift 6):

- 51 Rust tests (parser, scenarios, interactive/TUI, and 11 new feature tests: ping parsing, verdict logic, host validation, HTML/Prometheus output, completions, `--format` validation).
- 4 pseudo-terminal tests (terminal restoration, palette, pause/refresh).
- rustfmt and Clippy with warnings denied.
- 27 Swift tests, including diagnosis decoding, sparkline history and redacted summaries.
- `scripts/smoke-cli.sh`: 44 read-only checks of the release binary against the live Mac (valid JSON/NDJSON, snapshot mode 0600 and no-overwrite, error exits).
- `scripts/build-macos.sh` and `scripts/package-mac.sh`: app build, DMG, pkg, tarball and verified checksums.
- Screenshots in `docs/ux/` captured from the running app in `--demo` mode.

## Not executed

Intel Macs, notarization, administrator-authorized configuration changes, Wi-Fi join/disconnect and Location-permission flows. Use the manual checklist below for those.

## Repeat locally

```sh
cargo fmt --check
cargo clippy --all-targets --offline --locked -- -D warnings
cargo test --offline --locked
python3 scripts/check-contract.py
python3 scripts/test-tui-pty.py
scripts/smoke-cli.sh
swift test --package-path macos
./scripts/build-macos.sh
open dist/Macora.app
```

19 native unit-test methods cover palette search, schema uniqueness, snapshot comparison, redaction, service parsing, typed IPv4/IPv6/DNS input, proxy limits, device/service validation, shell quoting, exact service ordering, cache/IPv6 plans, schema versioning and rate resets. They build plans only; they do not mutate the test machine.

GitHub Actions builds the Rust project on macOS 14/15 and Linux, checks the declared Rust 1.74 minimum, builds/tests the native package on macOS, and performs read-only interface/doctor smoke checks. CI does not exercise administrator prompts or edit its runner's networking.

## Manual Mac acceptance

1. Build/open on an Apple Silicon Mac and, before claiming Intel support is verified, an Intel Mac. Confirm light/dark mode, keyboard shortcuts, selection, window resize and readable inspector text.
2. Confirm collector output matches `ifconfig -a`, hardware ports match `networksetup -listallhardwareports`, and byte totals match `netstat -ibn`. Check an attached Ethernet/Thunderbolt device, a bridge, several utun interfaces, link-down adapters and scoped/deprecated IPv6.
3. Disable Location permission, scan, and verify the app explains redacted network names. Enable permission and rescan. Join a personal network using correct and incorrect passwords; verify a failed join stays visible, and no password enters process arguments or event history. Try an enterprise SSID and confirm settings handoff.
4. Open Configure without applying anything; verify settings are untouched. Preview DNS, cancel authorization, and confirm cancellation is reported. Apply a known test DNS server, inspect the current service configuration, then reset to automatic. Use an expendable local connection when testing changes.
5. Test DHCP, static IPv4, static IPv6, IPv6 automatic, search domains, each proxy type, bypass list, power on/off, preferred-network removal, MTU, service enable/disable, service order and locations. Inspect effective values with the corresponding `networksetup -get…` calls. Restore your original settings manually.
6. Trigger a refused device MTU and verify the system error appears. Confirm no event claims success after an error. Verify the Configure action stays disabled in demo mode.
7. Confirm snapshots are private and cannot overwrite an existing capture. Compare before/after captures. Check redacted exports for all modeled addresses/MACs; verify full DNS/route inspectors remain unredacted.
8. Test Cmd-K fuzzy search and arrow/Return routing. Pin an interface, restart and verify ordering. Change system/light/dark appearance, compact density and 1/2/5/10-second refresh. Open the menu bar, close/reopen the workspace and verify one polling task, updated timestamps and no fabricated rates.
9. Capture a baseline, change a test MTU and inspect the typed diff and observed timeline. Import a versioned JSON snapshot and reject invalid/duplicate inventories. Check IP/MAC masking across current and removed addresses, timeline and full/redacted demo exports.
10. Run native Network Tools: local subnet calculations, a known numeric TCP listener and a refused endpoint, DNS with a configured resolver, and the local-only evidence report. Confirm demo disables network operations.
11. Disconnect a cable or pause polling: verify no fabricated counters, stale state gets a warning after a failed refresh, and rates recover after the next successful samples.
12. Test system-settings links on each supported OS; Apple may change settings URL anchors. If a specific anchor no longer works, open the Network pane directly.

Release requires actual hardware validation and, for public downloads, Developer ID signing and notarization. No notarized binary is included in this ZIP.
