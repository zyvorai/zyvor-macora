<div align="center">

# MACORA
### Your Mac’s network, made clear.
**Native SwiftUI workspace + Rust CLI + interactive terminal. Built by Zyvor AI Labs.**

macOS 14+ · Apple Silicon & Intel source builds · Apache 2.0 · Local-first

![Design preview — simulated values, not a native app screenshot](docs/design-preview.png)

</div>

MACORA replaces a wall of `ifconfig` output with interface groups, explanations and a native network management workspace. Its Rust collector and terminal workspace preserve scoped IPv6, deprecated/temporary addresses, bridge members, flags and offload options. Its SwiftUI app presents adapters, live traffic, scoped DNS, routes and connection diagnostics alongside deliberate network configuration controls.

The illustration above is a **design preview with simulated values**. The app itself starts with live collection; `--demo` is a separate, clearly labeled mode. This project is new source code, not a fork of Vytrix. The design takes broad inspiration from Vytrix’s sidebar/card layout without copying its code or assets.

## New in 0.2

**A Nuvora-inspired native workspace and GuestKit-style interactive exploration, implemented for Mac networking.**

- Spacious light/dark overview, connection stage and metric band; Cmd-K fuzzy command palette.
- Menu bar state/rates, pinned adapters, saved appearance/density/polling preferences and workspace IP/MAC masking.
- Observed bridge topology, JSON baseline import, field-by-field comparison and an in-memory change timeline.
- Explicit TCP/subnet/DNS/report tools and a practical investigation guide.
- `macora interactive` / `repl` with cached capture, selection, search, history, plans and private exports.
- `macora tui` with six pages, keyboard navigation, filtering, favorites, sorting, pause, manual refresh, baseline drift and safe terminal restoration.
- New `topology`, `report` and `resolve` commands. No new runtime package dependencies.

The source ZIP also includes `UPGRADE-v0.2.0.patch`, checked against the 0.1.0 source. The patch excludes generated archive checksum metadata.

See [CHANGELOG](CHANGELOG.md), [interactive guide](docs/INTERACTIVE.md) and [UX/reference notes](docs/UX.md).

## Build on your Mac

Install Rust and Xcode Command Line Tools, then:

```sh
# From the unzipped project directory:
./scripts/build-macos.sh
open dist/Macora.app
```

The script runs Rust and Swift tests, builds the CLI and native SwiftUI executable, bundles the collector, and ad-hoc signs both. Its output is a **local development build**; it is not notarized. Set `MACORA_SIGN_IDENTITY` to your Developer ID identity to sign a distribution build; notarization is a separate release step. No Mac App Store compatibility is claimed: configuration currently uses administrator-authorized AppleScript rather than a sandboxed helper.

CLI only:

```sh
cargo install --path . --locked --offline
macora
macora --all
macora explain en0
macora watch --interval 1
macora doctor
macora interactive
macora tui --interval 1
```

There are **zero external Rust crates and zero external Swift packages**. SwiftUI, Charts, CoreWLAN, CoreLocation and AppKit are Apple system frameworks. No node server, webview or daemon is required. The Rust parser/tests can run on Linux for development; the product and live networking features target **macOS only**.

## Native application

- **Overview:** active adapter count, live RX/TX, a 90-sample in-memory traffic chart, clear interface cards, pause/resume and refresh timestamps. Missing counters remain unavailable rather than being invented.
- **Interface explorer:** group and search adapters, tunnels, bridges, loopback and Apple peer interfaces; inspect addresses, scopes, temporary/deprecated IPv6 attributes, MTU, MAC, media, flags, offloads and bridge members.
- **Wi-Fi:** user-triggered CoreWLAN discovery, signal strength, channel, security class, join open/personal networks, and confirmed disconnect. Network names may require Location permission. Enterprise 802.1X and saved-credential management open System Settings.
- **Services:** DHCP/static IPv4, automatic/link-local/static IPv6, custom DNS and reset to automatic, search domains, HTTP/HTTPS/SOCKS/PAC proxies, proxy bypass entries, per-proxy disable, Wi-Fi power, forgetting a preferred network, MTU, service enable/disable and exact service priority ordering.
- **Locations:** create populated network locations, switch locations, and delete an existing location with the same preview/authorization flow.
- **Explore:** full IPv4/IPv6 route tables, every scoped DNS resolver, ARP/IPv6 neighbors and TCP/UDP sockets visible to the current user. These pages retain native diagnostic output in a selectable inspector.
- **Diagnostics:** self-assigned IPv4, missing address assignment, duplicate/tentative IPv6, normal privacy rotation, low tunnel MTU, inactive bridges, and default route/reachability inspection.
- **Workspace:** Cmd-K action search; observed topology; imported/captured JSON baselines; counter-independent change comparison; bounded in-memory change timeline; pinned adapters; menu bar monitoring; persistent appearance/density/refresh preferences.
- **Network tools:** user-triggered numeric TCP probes, local IPv4 subnet calculations, system DNS lookups and redacted interface evidence reports.
- **Change preview:** a validated command and consequence before any settings mutation; the macOS authorization dialog handles the administrator password. Wi-Fi association uses a separate explicit join sheet and CoreWLAN.
- **Sharing:** full or IP/MAC-redacted JSON export with private file permissions; successful configuration history stays in memory for the current app session.
- **Light and dark:** system appearance, semantic native colors, blue accents, rounded cards, sidebar navigation and keyboard shortcuts.

Open the review sheet with **Configure → Action → target → Preview Change → Authorize & Apply**. No mutation runs merely by selecting a menu item or typing in a field. There is no automatic rollback. CoreWLAN joins and disconnects can also interrupt a connection, so both have explicit user actions.

## Interactive terminal

```sh
macora interactive              # Persistent command session
macora repl --from tests/fixtures/macos.ifconfig
macora tui --all --interval 1    # Live full-screen workspace on a Mac
macora tui --from tests/fixtures/macos.ifconfig
```

In the REPL, use `select en0`, `detail`, `baseline`, `refresh`, `changes`, `history`, `redact on`, `export FILE` and `help`. A settings plan is never executed without explicit apply and exact terminal confirmation. The TUI uses `/` to filter, `:` for its palette, 1–6 for pages, arrows/j/k for navigation, `b` for baseline, `p` for pause and `q` to exit. [Full guide](docs/INTERACTIVE.md).

![Actual Rust terminal rendering against the sanitized fixture](docs/tui-preview.png)

## CLI cookbook

```sh
macora                              # Connected adapters first; system groups collapsed
macora --all --compact              # Expand everything with less detail
macora --group Tunnels
macora --interface en0 --ipv6
macora --json                       # All interface records, schema v1
macora --all --redact                # IP/MAC masking for a terminal capture
macora services
macora wifi
macora routes
macora topology
macora report --redact
macora resolve example.com           # Explicit DNS lookup
macora path                          # Default IPv4/IPv6 route + scutil reachability
macora dns                           # Keep resolver scopes; do not flatten VPN DNS
macora neighbors
macora sockets
macora watch --count 10 --interval 1 --json  # NDJSON frames, counters and per-interface rates
macora snapshot before.macora        # Exact ifconfig capture, mode 0600; no overwrites
macora --from before.macora --all     # Inspect a capture offline
macora snapshot after.macora
macora diff before.macora after.macora --redact
macora subnet 192.168.1.8/24
macora probe 1.1.1.1:443 --timeout-ms 1500  # Explicit outbound TCP connection
macora set en0 mtu 1400               # Preview only
sudo macora set en0 mtu 1400 --apply   # Explicit change; device may reject MTU
```

`--up` means administratively enabled; link-active is a different property. `utun` presence does **not** establish VPN ownership or connectivity. `en0` is not assumed to be Wi-Fi: hardware port names come from `networksetup`. Global-range address labels do not verify internet reachability. UI traffic sums the Network adapter group only, reducing tunnel/bridge double counting; it is not a per-process bandwidth accounting engine.

Snapshots contain private network data. Redaction masks IP and MAC values present in the interface model; it does not anonymize interface labels, service names, SSIDs, proxy settings, route/DNS inspectors or arbitrary copied text. Raw captures and full exports remain intentionally unredacted. Redacted JSON is a sharing format; do not use it as a configuration source.

Colors automatically turn off in pipes and with `NO_COLOR`. `--color always|never` overrides automatic detection. Built-in help lists every command. Optional zsh completion is in `completions/_macora`.

## NetworkManager-style coverage

MACORA uses macOS network services and locations rather than Linux NetworkManager profiles, `nmcli`, netlink or systemd. See [the precise feature mapping](docs/FEATURES.md). Custom VPN providers, enterprise certificate workflows, firewall engines, Internet Sharing and virtual bridge/VLAN creation remain in System Settings or require future privileged/network-extension implementations. It does not promise complete Linux NetworkManager parity.

## Project layout

```text
src/                  Dependency-free Rust CLI, parser, collector and renderer
tests/                Scenario tests and sanitized macOS ifconfig fixture
macos/Sources/        Native SwiftUI app and reusable Swift core
macos/Tests/          Command validation, JSON and traffic calculation tests
scripts/              Mac .app build/sign script and fixture schema validation
docs/                 Architecture, feature coverage, testing and design preview
.github/workflows/    Rust lint/tests, macOS app build/tests and read-only smoke checks
```

## Validation status

40 Rust tests, four pseudo-terminal tests, formatting and Clippy were executed in a Linux development container against sanitized macOS captures and a local TCP listener. Native Swift source syntax was checked separately across 16 files; 19 native unit-test methods are supplied but were not run here. **SwiftUI compilation, Swift tests, macOS live collection, authorization, CoreWLAN and real settings changes were not executed in this environment.** macOS CI and a manual hardware test plan are included; their presence is not a claim they have already passed.

See [TESTING.md](docs/TESTING.md) for exact checks and the hardware acceptance checklist. The ZIP is a complete source project, not a prebuilt `.app` or DMG.

## License

Copyright 2026 Zyvor AI Labs. Licensed under **Apache License 2.0**, including the Rust CLI and Swift application. See [LICENSE](LICENSE) and [NOTICE](NOTICE). The versioned source package is 0.2.0. The proposed repository slug is `zyvorai/zyvor-macora`; no repository or release has been published by this package.
