# Changelog

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
