# AI docs (zyvor-macora)

_Generated 2026-10-20 by claude-opus-5-5 from a snapshot of tracked sources; verify before acting._

# Part 1: Improved README.md

````markdown
<div align="center">

# MACORA

[![verify](https://github.com/zyvorai/zyvor-macora/actions/workflows/ci.yml/badge.svg)](https://github.com/zyvorai/zyvor-macora/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache--2.0-0071e3)](LICENSE)
[![macOS](https://img.shields.io/badge/macOS-14%2B-2997ff)](docs/TESTING.md)
[![Rust](https://img.shields.io/badge/rust-1.74%2B-orange)](Cargo.toml)

**MACORA explains your Mac's network and tells you why it feels slow. It ships as a SwiftUI app, an `ifconfig`-style Rust CLI, a REPL and a full-screen terminal UI.**

Read-only by default · No external Rust crates or Swift packages · No telemetry, server or daemon · macOS 14+

![MACORA diagnosis wizard on simulated demo data](docs/ux/native-dark-diagnostics.png)

*Screenshot of the app running on its built-in simulated demo data (`--demo`).*

</div>

---

## Contents

- [What it does](#what-it-does)
- [Install](#install)
- [Quick start](#quick-start)
- [Usage](#usage)
- [Configuration](#configuration)
- [Privacy and safety](#privacy-and-safety)
- [Limitations](#limitations)
- [Contributing](#contributing)
- [Documentation](#documentation)
- [License](#license)

---

## What it does

- **Interfaces, explained.** `macora` reads `ifconfig -a`, `networksetup -listallhardwareports` and `netstat -ibn`. It groups interfaces as Network, Bridges, Virtual, Tunnels, System and Loopback. It labels each one with its hardware name and explains flags, IPv6 scopes and attributes, MTU, MAC, media, bridge members and byte counters.
- **"Why is my network slow?"** `macora diagnose` checks the network from the bottom up and names the lowest layer that is actually wrong. It covers link, route, Wi-Fi quality, router, internet, captive portal, each DNS server, HTTPS timing, path MTU, IPv6 and proxies. It then suggests what to try.
- **Snapshots and diffs.** It can save a private `ifconfig` capture, compare two captures field by field, or inspect a saved capture offline.
- **Machine-readable output.** Every command accepts `json`. `watch` also emits NDJSON frames or Prometheus text. Reports can be written as Markdown or HTML, and `redact` masks addresses.
- **Native app.** The app includes:
  - an overview with live rates
  - an interface explorer
  - Wi-Fi scan and join through CoreWLAN
  - route, DNS, neighbor and socket inspectors
  - the diagnosis wizard
  - baseline comparison
  - a menu bar panel
  - a ⌘K command palette
  - two Shortcuts actions
  - previewed, administrator-authorized configuration changes
- **Changes always need two steps.** Nothing changes a setting until you preview it and then confirm explicitly. In the CLI you add `apply`. In the app you press **Authorize & Apply** in the macOS authorization dialog.

## Install

**Requirements:** macOS 14 (Sonoma) or newer. Prebuilt releases are for Apple silicon.

### Download (app and CLI)

Get these files from the [Releases page](https://github.com/zyvorai/zyvor-macora/releases):

| File | Contents | Notarized |
| --- | --- | --- |
| `Macora-X.Y.Z.dmg` | Drag `Macora.app` to Applications. | Yes |
| `Macora-X.Y.Z.pkg` | Installs the app and links `/usr/local/bin/macora` to the CLI inside the app. | Yes |
| `macora-X.Y.Z-<arch>-apple-darwin.tar.gz` | CLI, man page and completions. | No (signed only) |

Each file has a `.sha256` file:

```sh
shasum -a 256 -c Macora-X.Y.Z.dmg.sha256
```

If macOS blocks the tarball binary, remove the quarantine flag once:

```sh
xattr -d com.apple.quarantine macora
```

### Homebrew

```sh
brew install --cask zyvorai/tap/macora   # the app, plus `macora` on PATH and shell completions
brew install zyvorai/tap/macora          # the CLI only (with man page and completions)
```

Install one or the other. Both provide the same `macora` command and are declared as conflicting.

Recent Homebrew versions may report that `zyvorai/tap` is not trusted. If so, run `brew trust --cask zyvorai/tap/macora` (or `brew trust --formula zyvorai/tap/macora`) and install again.

### From source

You need the Xcode command line tools and a Rust toolchain (1.74 or newer).

```sh
git clone https://github.com/zyvorai/zyvor-macora.git && cd zyvor-macora

./scripts/install-macos.sh ~/.local   # CLI + man page + completions; PREFIX defaults to /usr/local
./scripts/build-macos.sh              # runs Rust and Swift tests, then builds dist/Macora.app (ad-hoc signed)
open dist/Macora.app
```

Source builds target the host Mac's architecture.

### Uninstall

- **App:** drag `Macora.app` to the Trash, or run `brew uninstall` on the cask or formula.
- **Background services:** none.
- **Preferences:** stored in `~/Library/Preferences/dev.zyvor.macora.plist`. The cask's `zap` also removes saved application state.

## Quick start

```sh
macora                  # connected adapters first, system groups collapsed
macora en0              # one interface, explained
macora diagnose         # why is my network slow? (about 8 s)
macora tui              # full-screen terminal workspace
open -a MACORA --args --demo   # explore the app on simulated data, without touching your network
```

Turn on shell completion by adding one line to your shell startup file:

```sh
eval "$(macora init zsh)"     # ~/.zshrc
eval "$(macora init bash)"    # ~/.bashrc
macora init fish | source     # fish config
```

## Usage

### Options are plain words

Options are words rather than dashes. For example, `macora report html redact`, not `--html --redact`. You can combine words: `macora ipv6 group Tunnels`.

The older `--flag` spellings still work so existing scripts keep running.

| Word | Meaning |
| --- | --- |
| `all` | Expand every group. |
| `up` / `down` | Filter by state. |
| `compact` | Shorter output. |
| `ipv4` / `ipv6` | Show one address family. |
| `raw` | Print `ifconfig`'s own text. |
| `json` | Machine-readable output. |
| `redact` | Mask addresses. |
| `html` / `md` / `prometheus` | Output format. |
| `from FILE` | Read a saved capture instead of the live system. |
| `interface NAME` | One interface. |
| `group NAME` | One group. |
| `count N` | Number of repeats. |
| `interval SECONDS` | Time between repeats. |
| `timeout MS` | Probe timeout. |
| `theme NAME` | TUI theme. |
| `color always\|never` | Force colors on or off. |
| `apply` | Run a previewed change. |
| `type RECORD` | Record type for `lookup`. |

Run `macora help` for the full list, or `man macora` after installing.

### Inspect interfaces

```sh
macora all                    # everything, including tunnels, system links and loopback
macora en0 raw                # ifconfig's own text for en0
macora list                   # names in system order, identical to `ifconfig -l`
macora list up                # only interfaces that are up (also: down)
macora explain en0            # flags, IPv6 and role in plain words
macora doctor                 # local interface findings; sends no network traffic
macora routes | dns | wifi | services | neighbors | sockets | path | topology
```

### Diagnose

```sh
macora diagnose                        # full check, about 8 s
macora diagnose quick                  # about 2 s; skips Wi-Fi details and the path-MTU search
macora diagnose to github.com          # also check one site: DNS per server, HTTPS stages, path
macora diagnose speed                  # also run Apple's networkQuality (about 15 s, uses bandwidth)
macora diagnose redact md              # shareable report with your addresses masked (also: html, json)
macora diagnose count 10 interval 30   # repeat and summarize what flapped
macora diagnose strict                 # exit 1 on warnings as well as failures
```

**Reading the output:**

- Each line has a status: `OK`, `WARN`, `FAIL`, `SKIP` or `INFO`.
- **VERDICT** names the most important problem. Failures outrank warnings, and the lower layer wins between findings of the same status.
- **WHAT TO TRY** gives up to three concrete steps.
- **ALSO NOTICED** lists the remaining findings.

The JSON output contains `steps[]` (each with `step`, `status`, `detail` and `ms`), plus `verdict`, `advice[]`, `also[]` and `target`.

See [docs/DIAGNOSE.md](docs/DIAGNOSE.md) for every check, its thresholds and what it sends.

### Network tools

All of these run only when you invoke them:

```sh
macora ping 1.1.1.1 count 5
macora trace example.com
macora resolve example.com
macora probe 1.1.1.1:443 timeout 1500   # one explicit outbound TCP connection
macora subnet 192.168.1.8/24            # local IPv4 calculation
```

Version 0.5.0 adds these commands (macOS only, read-only):

```sh
macora scan                 # nearby Wi-Fi, channel crowding, quieter-channel hint
macora apps interval 3      # traffic by process (via nettop)
macora lan                  # devices on your subnet (ping sweep up to a /22, plus the ARP table)
macora mtr 1.1.1.1 count 20 # per-hop loss, latency and jitter
macora lookup github.com type MX   # your resolver vs 1.1.1.1, 8.8.8.8 and 9.9.9.9
macora firewall             # application firewall and pf state
macora throughput HOST      # iperf3 wrapper; needs `iperf3 -s` on the other machine
```

Notes on these commands:

- `scan` needs Location Services to allow your terminal before macOS reveals other networks.
- Run `lan` only on networks you manage.
- `apps` shows per-process rates, not per-connection accounting.

### Reports, snapshots and monitoring

```sh
macora report html redact > report.html   # standalone, script-free HTML
macora report md
macora snapshot before.macora             # exact ifconfig capture, mode 0600, refuses to overwrite
macora diff before.macora after.macora redact
macora all from before.macora             # inspect a capture offline
macora watch count 10 interval 1 json     # NDJSON frames with counters and rates
macora watch prometheus                   # Prometheus text exposition
macora completions zsh > ~/.zsh/completions/_macora   # also bash, fish
```

### Changing settings (CLI)

```sh
macora en0 mtu 1400               # preview only
sudo macora en0 mtu 1400 apply    # runs /sbin/ifconfig; the device may still refuse
```

The CLI changes only interface MTU and up/down state. It does not request administrator rights itself, so an unprivileged `apply` fails with the normal system error. Offline captures (`from FILE`) can never be applied.

### Interactive terminal

```sh
macora interactive                                # or: macora repl
macora repl from tests/fixtures/macos.ifconfig    # offline; no network operations or apply
macora tui all interval 1
macora tui theme high-contrast                    # default | mono | high-contrast
```

**REPL commands:** `select en0`, `show`, `detail`, `family 4|6|all`, `filter TEXT`, `baseline`, `refresh`, `changes`, `redact on`, `report`, `export FILE`, `history`, `!N`, `plan mtu 1400`, `apply`, `cancel`, `help`, `quit`.

- `apply` requires a live session, a terminal, and the exact `APPLY …` line typed back.
- Recalling `apply` from history is refused.
- Pipes, redirection, backticks and command chains are rejected.

**TUI keys:**

| Key | Action |
| --- | --- |
| `1`–`6` | Overview, Interfaces, Detail, Topology, Diagnostics, Changes |
| `/` | Filter |
| `:` | Palette |
| `j`/`k` or arrows | Navigate |
| `f` | Favorite |
| `s` | Sort |
| `a` | Show all |
| `p` | Pause |
| `r` | Refresh |
| `b` | Baseline |
| `d` | Changes |
| `?` | Help |
| `q` / Ctrl-C / Ctrl-D | Quit |

The TUI never changes settings and restores the terminal when it exits. See [docs/INTERACTIVE.md](docs/INTERACTIVE.md) for the full guide.

### Native app

The sidebar pages are:

- Overview
- Interfaces
- Wi-Fi
- Services
- Topology
- Routes
- DNS
- Neighbors
- Connections
- Network Tools
- Diagnostics
- Compare
- Change History

**Configure** walks you through **Action → target → Preview Change → Authorize & Apply**. Supported actions:

- DHCP, static IPv4
- IPv6 automatic, link-local only, or static
- DNS servers (blank restores automatic), search domains
- HTTP, HTTPS, SOCKS and PAC proxies, disabling a proxy, bypass domains
- Wi-Fi power on/off, forgetting a saved Wi-Fi network
- MTU (576–9000)
- Enabling or disabling a service, service order
- Creating, switching and deleting locations
- Flushing the directory DNS cache

Each change shows the exact `networksetup`/`dscacheutil` command first. macOS owns the password dialog. Changes are **not** rolled back automatically, and they are disabled in demo mode.

**Keyboard shortcuts:**

| Shortcut | Action |
| --- | --- |
| ⌘K | Command palette |
| ⌘R | Refresh |
| ⇧⌘P | Pause or resume |
| ⇧⌘D | Run the diagnosis |
| ⇧⌘C | Copy redacted summary |
| ⇧⌘E | Export redacted snapshot |
| ⌘/ | Shortcut sheet |

**Shortcuts actions:**

- **Network status:** read-only summary with addresses masked.
- **Why is my network slow?:** runs `diagnose` and returns the verdict.

The app runs the bundled CLI from `Macora.app/Contents/Resources/macora`. If that file is missing, it falls back to `/usr/local/bin/macora`.

## Configuration

MACORA has no configuration file.

### CLI

- **Words:** behavior is controlled by the words described in [Usage](#usage).
- **Color:** turns off in pipes and when `NO_COLOR` is set. Use `color always|never` to override.
- **Exit status:**
  - `0` on success.
  - `1` on error.
  - For `diagnose`, `1` when any step fails, or when any step warns if you add `strict`.

### App preferences

Open **MACORA → Settings**. Preferences are stored locally in UserDefaults under `dev.zyvor.macora`:

| Preference | Values |
| --- | --- |
| Appearance | System, Light, Dark |
| Accent color | blue, teal, purple, orange, pink |
| Density | Comfortable, Compact |
| Refresh interval | 1, 2, 5 or 10 seconds |
| Mask addresses | Masks modeled IP and MAC addresses in workspace views |

Preferences never change network settings. Traffic history keeps 90 samples and the change timeline keeps 200 events. Both are held in memory only.

### App launch arguments

- `--demo`: use the bundled simulated data. Live tools and changes are disabled.

### Build and release environment variables

These are used by `scripts/build-macos.sh` and `scripts/package-mac.sh`:

| Variable | Purpose |
| --- | --- |
| `MACORA_SIGN_IDENTITY` | Developer ID Application identity. Without it, builds are ad-hoc signed. |
| `DEVELOPER_ID_INSTALLER` | Signs the pkg. |
| `NOTARY_PROFILE` | `notarytool` keychain profile. Notarization runs only with this variable and a signing identity. |

## Privacy and safety

- **No shell.** Inspection calls fixed macOS executable paths with argument arrays and never goes through a shell. Host names are validated before any probe.
- **Active checks run only on request.** `diagnose`, `ping`, `trace`, `probe` and `resolve` run only when you invoke them. Nothing is sent to Zyvor, and nothing runs in the background except the app's local polling.
- **`redact` coverage is narrow.**
  - It masks IP and MAC values from the interface model.
  - For `diagnose`, it also masks the router, local addresses and configured DNS servers. The public probe targets (1.1.1.1, 8.8.8.8, 9.9.9.9) stay readable.
  - It does **not** anonymize interface labels, service names, proxy settings, route/DNS inspector output or text you add yourself.
  - Read a report before you share it.
- **Snapshots and exports.** Snapshots are mode 0600 and are never overwritten. Raw captures and full exports are intentionally unredacted. A snapshot is not a rollback mechanism.
- **Wi-Fi passwords** are passed to CoreWLAN in memory. They are not written to commands, exports or history.

Report vulnerabilities privately as described in [SECURITY.md](SECURITY.md).

## Limitations

- **What output does not prove:**
  - `up` means administratively enabled, which is not the same as link-active.
  - A `utun` interface does not prove a VPN is connected or identify its owner.
  - A global-range address label does not prove internet reachability.
- **Platform:** live collection is macOS-only. On Linux, the parsers, tests and offline `from FILE` inspection work.
- **Not yet verified:** Intel Macs, administrator-authorized configuration changes, Wi-Fi join/disconnect and Location-permission flows. See [docs/TESTING.md](docs/TESTING.md).
- **Left to System Settings:** VPN providers, enterprise 802.1X, firewall engines, Internet Sharing and creating bridges or VLANs. See [docs/FEATURES.md](docs/FEATURES.md).

## Contributing

Bug reports, fixtures from other macOS versions and focused pull requests are welcome. Please read [CONTRIBUTING.md](CONTRIBUTING.md) and the [Code of Conduct](CODE_OF_CONDUCT.md).

The project follows these principles:

- Read-only by default.
- No external Rust crates or Swift packages.
- Never invent data.
- Preserve JSON schema version 1, or add an explicit migration.

Run these checks before opening a PR:

```sh
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
python3 scripts/check-contract.py && python3 scripts/test-tui-pty.py
cargo build --release --locked && scripts/smoke-cli.sh
swift test --package-path macos
# live network, read-only:
python3 scripts/verify-cli.py target/release/macora && python3 scripts/verify-diagnose.py target/release/macora
```

- Sanitize fixtures by replacing real IPs, MACs, SSIDs and hostnames.
- For UI changes, check both light and dark mode and attach screenshots from `scripts/shots.sh` (demo data only).
- `scripts/render-terminal.py` regenerates the terminal images. It needs Pillow, which is a documentation-only dependency.

**Project layout**

```text
src/            Rust CLI: parser, collector, diagnose, probes, exporters, REPL, TUI
tests/          Rust integration tests and sanitized fixtures
macos/          SwiftUI app (MacoraApp), testable core (MacoraCore), Swift tests
scripts/        build, package, install, smoke/verify, screenshots, Homebrew, wiki sync
completions/    bash, zsh and fish completions
man/            macora(1)
packaging/      Homebrew formula/cask templates and pkg scripts
site/           GitHub Pages site
wiki/           Wiki sources (synced by scripts/sync-wiki.sh)
```

## Documentation

| Doc | Contents |
| --- | --- |
| [docs/DIAGNOSE.md](docs/DIAGNOSE.md) | Every `diagnose` check, thresholds, what is sent, caveats |
| [docs/INTERACTIVE.md](docs/INTERACTIVE.md) | REPL and TUI guide |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Collector, JSON schema, app structure, authorization model |
| [docs/FEATURES.md](docs/FEATURES.md) | macOS ↔ NetworkManager-style task mapping |
| [docs/TESTING.md](docs/TESTING.md) | What was verified, and the manual hardware checklist |
| [docs/RELEASE.md](docs/RELEASE.md) | Release checklist |
| [docs/UX.md](docs/UX.md) | Design notes |
| [Wiki](https://github.com/zyvorai/zyvor-macora/wiki) | Install, CLI guide, troubleshooting, FAQ |
| [CHANGELOG.md](CHANGELOG.md) | Release history |

## License

Copyright 2026 Zyvor AI Labs. Licensed under the [Apache License 2.0](LICENSE), covering both the Rust CLI and the Swift app. See also [NOTICE](NOTICE).

The visual design takes inspiration from other Zyvor projects, but no code or assets are shared with them.
````

---

# Part 2: Documentation gaps I could not fill

**Source I could not see**
- `src/cli.rs`, `src/netcmds.rs`, `src/session.rs`, `src/tui.rs` and `src/export.rs` were not provided.
- As a result, I could not confirm these from code: `init`, `scan`, `apps`, `lan`, `mtr`, `lookup`, `firewall`, `throughput`, the `version` command, the "did you mean" hints, and the full word list. These are documented only in CHANGELOG, the wiki and the old README.
- I could not see the man page or the completion files.

**Unverified app launch arguments**
- `--page NAME` is used by `scripts/shots.sh`, and `--autorun` is mentioned in TESTING.md. I could not find either in the shown Swift code, so they are left out of the README.

**pkg postinstall**
- `packaging/pkg/scripts` was not shown. The `/usr/local/bin/macora` link behavior comes from the wiki and RELEASE.md only.

**Prometheus metrics**
- Only `macora_interface_up` is confirmed. The full set of metric names and labels is undocumented.

**JSON schema**
- There is no formal schema document. The interface schema (v1) can only be inferred from `model.rs` and `Models.swift`.
- The NDJSON `watch` frame format is only partially visible (`frame` and `rates` keys).

**Contradictions between existing docs**
- **What `diagnose` sends.** SECURITY.md says "a few ICMP packets, one TCP connection to 1.1.1.1:443 and one DNS lookup". DIAGNOSE.md and the FAQ say about 30 ICMP probes, three TCP targets, multiple DNS lookups and HTTPS/HTTP requests. SECURITY.md looks stale.
- **SSID redaction.** CHANGELOG 0.5.0 says "`redact` masks SSIDs and MACs where they appear". The README, FAQ and CLI Guide say SSIDs are *not* anonymized. The scope needs to be stated precisely.
- **Test counts.** The README says 178 Rust and 34 Swift tests. TESTING.md says "166+" Rust tests in one place and "19 native unit-test methods" in another.
- **Palette size.** UX.md says the palette has 17 actions. `Workspace.swift` defines 19.
- **Stale ZIP note.** TESTING.md ends with "No notarized binary is included in this ZIP", but notarized releases exist.
- **macOS version.** DIAGNOSE.md and TESTING.md refer to "macOS 27". I could not confirm the actual tested OS version.

**Architecture and naming of artifacts**
- `package-mac.sh` builds a *universal* CLI tarball when both Rust targets are installed, and `release.yml` installs both.
- However, the Homebrew formula and `update-homebrew.sh` expect an `arm64` tarball name.
- So the artifact naming for CI builds versus hand-made builds is unclear.

**Intel support**
- The universal CLI suggests the binary may run on Intel, but releases are documented as Apple silicon only and Intel is unverified.

**Version flag**
- Docs use both `macora --version` and `macora version`. I could not confirm whether both exist.

**Missing files**
- `CODEOWNERS` is referenced in the Code of Conduct but was not provided. The maintainer contact for conduct reports is unknown.
- `NOTICE` and `LICENSE` were not shown, so I did not reproduce their contents.

**Swift toolchain**
- `Package.swift` requires swift-tools 5.9, but there is no documented minimum Xcode version.

**App data and permissions**
- I could not confirm which data lives in UserDefaults beyond the keys shown. For example, the storage key for pinned interfaces and the `macora.onboarded` key are not documented.
- The entitlements (`macos/Macora.entitlements`) and the `Info.plist` permission strings were not shown.

**Troubleshooting environment differences**
- TESTING.md notes that `arp -an` returns nothing when launched from another program. The root cause is unidentified, and there is no user-facing workaround beyond the `lan` behavior.
