# Validation and hardware acceptance

## Executed for the `diagnose` rework

Run on an Apple silicon Mac (macOS 27.x, Rust 1.98, Swift 6):

- **Rust:** `cargo test --locked`: parser/JSON/path-MTU tests, ~70 diagnose scenarios, the collector replayed against a fake runner, CLI validation and real-socket tests, plus the earlier suites. `cargo clippy -D warnings` and `cargo fmt --check` are clean.
- **Path-MTU search:** exact for every MTU from 576 to 1500, never probes a size twice, at most 4 rounds.
- **Mutation checks:** 16 deliberate breakages (for example removing the "router ignores ping" downgrade, dropping jitter from the ping parser, counting a 169.254 address as usable, redacting public probe targets) must make the suite fail. The first pass left two survivors and a later one a third; each became a test and all are now caught.
- **Swift:** `swift test` (decoding old and new JSON, redaction, Markdown, site-name validation, an explanation for every step) and `scripts/check-contract.py`, which fails if the Rust step names or statuses and the app's list drift apart (also verified to fail when either side is changed).
- **Live, read-only:** `scripts/smoke-cli.sh` against the real network (all modes and formats, validation errors, redaction removes the router address in text/JSON/Markdown/HTML), and the app built and run live with `--autorun` to confirm it decodes the real CLI output.
- **Measured, not assumed:** the path MTU the tool reports was compared with a manual `ping -D` bisection (1480 passes, 1492 fails); the Wi-Fi scan's effect on latency was measured (4 ms became 40-60 ms) and is why it runs last.

## Release verification (v0.4.0 and 0.4.1)

An independent pass, run against the **released** artifacts rather than the working tree. Everything is read-only; nothing changes a network setting.

| Area | What was checked | Result |
| --- | --- | --- |
| Fresh clone of the tag | `cargo fmt --check`, `clippy -D warnings`, 166+ Rust tests, `check-contract.py`, 4 PTY tests, 34 Swift tests | all pass |
| Released CLI | `scripts/smoke-cli.sh` (71 checks) on the downloaded binary | 71/71 |
| CLI vs ground truth | `scripts/verify-cli.py` (52 checks): `list`, `raw` and `en0 raw` equal `ifconfig` (MAC lines excluded); JSON counters equal `netstat -ibn`; `subnet` equals Python `ipaddress` for 7 CIDRs including /0, /31 and /32; `routes`, `dns`, `wifi`, `sockets`, `doctor` run; `probe`, `ping`, `trace` against open, closed and local targets; snapshots are mode 0600 and never overwrite; `diff` sees a changed MTU; a previewed or unprivileged MTU change leaves the interface untouched; NDJSON `watch` and Prometheus text are well formed; `redact` removes this Mac's IPv4, IPv6 and MAC addresses from 8 output kinds; colour rules; bash/zsh/fish completions parse and complete | 52/52 |
| `diagnose` vs independent tools | `scripts/verify-diagnose.py` (28 checks): gateway and interface equal `route get default`; router latency vs `ping`; DNS vs `dig`; HTTPS total vs `curl -w`; path MTU vs a manual `ping -D` bisection; Wi-Fi signal vs `system_profiler`; portal vs a direct fetch; `to` a real, a missing and a local host; hostile host names rejected; text/JSON/Markdown/HTML valid; `redact` leaks none of the local addresses in any format; `count`, `strict`, `speed` (vs a second `networkQuality` run); time budget (full under 15 s, quick under 5 s) | 28/28 |
| Released DMG and pkg | notarization stapled and accepted by `spctl`; app signed by team NX667LTDQ2 with the hardened runtime; `codesign --verify --deep --strict`; bundled CLI runs; the app launches from the DMG | pass |
| pkg `postinstall` | run against a scratch root: links `/usr/local/bin/macora`; an unwritable target prints a message and still exits 0 | pass |
| Homebrew (fresh install, both) | Formula: installs, signed by team NX667LTDQ2 with the hardened runtime, passes `scripts/verify-cli.py` (52/52), man page and bash/zsh/fish completions linked, `brew test` passes, uninstalls cleanly. Cask: installs to `/Applications`, notarized and accepted by Gatekeeper, opens a window, links `macora` into `/opt/homebrew/bin`, uninstalls cleanly | pass |
| Website and README | zyvor-web build succeeds with `/macora` and `/docs/macora`, no macora broken links; every image the README references exists | pass |
| Mini | the released CLI runs on a second Mac and returns a clean verdict | pass |

**0.4.1 artifacts.** Built, signed and notarized on a separate Mac; downloaded back and checked: SHA-256 files match, DMG and pkg stapled and accepted by `spctl`, app passes `codesign --verify --deep --strict`, reports 0.4.1 with the completion fix, and the tarball binary passes `verify-cli.py` 52/52.

**Test-environment pitfalls.** (1) A `macora` earlier on `PATH` than Homebrew's (here, an old copy in `~/.local/bin`) is what `macora` runs, so test the installed path (`$(brew --prefix macora)/bin/macora`). (2) Running the app's `Contents/MacOS/Macora` with `--help` starts the GUI and does not return; use the bundled CLI in `Contents/Resources/macora` instead. (3) A Mac that sleeps drops off Wi-Fi and may return with a new address; confirm its SSH host key against the saved fingerprint before trusting the new address.

**Findings.** One bug: tab completion after `diagnose to` offered option words although the next word is a host. Fixed in 0.4.1 for bash, zsh and fish, with a test.

**One measurement worth knowing.** On the same router, the author's laptop measures a path MTU of 1480 and a second Mac measures 1500 (a 1500-byte do-not-fragment ping to 1.1.1.1 gets no reply on the laptop, three times of three, while the same size to the router works). The tool reports each correctly. The cause on the laptop was not identified.

Run the two scripts yourself: `python3 scripts/verify-cli.py target/release/macora` and `python3 scripts/verify-diagnose.py target/release/macora`. They need a live network, and the Wi-Fi check needs Wi-Fi.

## Not executed

Intel Macs, administrator-authorized configuration changes, Wi-Fi join/disconnect and Location-permission flows. Use the manual checklist below for those.

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
