# Changelog

## 0.3.1

### CLI: words, not dashes, and it works like ifconfig

- Options are plain words: `macora en0`, `macora en0 raw`, `macora list up`, `macora report html redact`, `macora watch count 5 json`. `macora en0` shows one interface, `raw` prints ifconfig's own text for it, and `list` prints names in system order, identical to `ifconfig -l`.
- `macora en0 mtu 1400` and `macora en0 up|down` preview the change; add `apply` to run it (ifconfig applies immediately).
- The older `--flag` spellings keep working as undocumented aliases, so scripts and CI do not break. Help, man page, completions and README show only words. Completions now complete interface names.
- The native app calls the CLI with words as well.
- Fix: `raw` on an offline capture no longer prefixes the OFFLINE banner.
- A command wins over leading option words (`macora color always diagnose`), and a name after options is the interface (`macora all en0`).

### Output

- Much more colorful: orange title and idle state, per-group headers, scope and attribute colors, green RX and orange TX totals. Address columns line up, including with `redact` (placeholders are padded). Colors still turn off in pipes and with `NO_COLOR`.
- The collapsed-group hint says `run macora all` instead of a dash flag.

### Packaging

- The pkg installs a `/usr/local/bin/macora` link to the CLI bundled in Macora.app, so `macora` works after installing the app.
- Add `scripts/render-terminal.py`, which renders real `macora` output (with `redact`) into README screenshots and a typing GIF.


## 0.3.0

### Diagnose

- Add `macora diagnose` and a native "Why is my network slow?" wizard: link, gateway, internet, DNS and MTU checks that stop at the lowest failing layer and explain the result in plain language. `--json` output is structured. Active checks run only when invoked.
- Add `macora ping HOST` and `macora trace HOST` (fixed-path `/sbin/ping` and `/usr/sbin/traceroute`, strict hostname validation).

### CLI and TUI

- Add `--format table|json|ndjson|md|html|prometheus`; `report --format html` writes a standalone, script-free report with light/dark styling; `watch/show --format prometheus` emits scrape-ready text.
- Add `macora completions bash|zsh|fish`, generated from one command table, and a `macora(1)` man page.
- Add `tui --theme default|mono|high-contrast`.

### Native app

- Add a first-run welcome card, per-adapter sparklines, a richer menu-bar panel (rates, diagnosis, copy redacted summary), a keyboard cheat sheet (Cmd-/), an accent color preference and a persistent DEMO/LIVE badge.
- Add Shortcuts actions "Network status" and "Why is my network slow?" (App Intents).
- Fix the export button stretching across the toolbar.

### Project

- Add CI smoke test (`scripts/smoke-cli.sh`), release workflow (CLI tarball, DMG, pkg and checksums; signs and notarizes only when Apple secrets exist), Pages workflow, issue/PR templates, Dependabot, Code of Conduct and a Homebrew formula template.
- Fix a Clippy `collapsible_match` lint and a macOS-only PTY test flake (`PENDIN` terminal flag).

### Validation

Run on macOS (Apple silicon): 51 Rust tests, 4 pseudo-terminal tests, rustfmt, Clippy, 27 Swift tests, an app build, packaging (DMG, pkg, tarball, checksums) and a 44-check read-only CLI smoke script. Not run: Intel Macs, notarization, authorized configuration changes, Wi-Fi join/disconnect.

## 0.2.0

### Terminal workspace

- Add `interactive` / `repl`: cached capture, interface focus, quoted arguments, name/address/group search, address-family selection, memory-only history/recall, baseline comparison, local evidence reports and private no-overwrite JSON export.
- Add explicit interface plan/cancel/apply workflow. Offline sessions cannot apply changes; history recall cannot authorize apply; live apply requires exact command confirmation on a terminal and existing privileges.
- Add full-screen `tui`: six pages, arrow/j/k navigation, detail, topology, diagnostics, drift, palette, filtering, favorites, sorting, pause/resume, manual refresh, masking and traffic sparklines when counters exist.
- Add terminal state restoration on normal exit, handled Ctrl-C/Ctrl-D and Rust unwinding, plus four pseudo-terminal acceptance tests.
- Add `topology`, `report` and explicit `resolve HOST` CLI commands.

### Native workspace

- Replace the overview tile grid with a Nuvora-inspired hero, connection stage and metric band; introduce consistent light/dark tokens and hairline surfaces.
- Add a fuzzy command palette (Cmd-K), keyboard selection and page/action routing.
- Add persistent appearance/density/refresh/masking preferences and pinned-interface ordering.
- Add a menu bar view with ongoing collection, adapter state, traffic rates, pause/resume and reopening the workspace.
- Add observed bridge topology, JSON baseline import, field-by-field configuration comparison and an in-memory timeline capped at 200 events.
- Add explicit TCP, subnet, DNS and evidence-report tools, plus an investigation guide.
- Validate imported inventory uniqueness/size, preserve action failures separately from refresh errors, and fix redaction of demo exports.

### Validation

40 Rust tests and four pseudo-terminal tests run locally. Rustfmt, Clippy and release compilation run locally. 19 native test methods are supplied; Swift syntax is checked across 16 files. Native compilation, hardware workflows and authorization are still pending a Mac run.

## 0.1.0

Initial Mac-only Rust CLI and SwiftUI source project, interface parsing, live counters, Wi-Fi, configuration previews and Apache 2.0 licensing.
