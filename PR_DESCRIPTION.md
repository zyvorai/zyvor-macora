Title: Add MACORA interactive workspaces and redesign the native network UI

MACORA 0.2 adds a persistent Rust REPL and a full-screen terminal dashboard, and redesigns the SwiftUI overview around a Nuvora-inspired hero, connection stage and metric band. The native app gains Cmd-K action search, menu bar monitoring, pinned adapters, appearance/density/refresh preferences, topology, JSON baseline import, typed configuration comparison, an observed-change timeline and explicit TCP/subnet/DNS/report tools.

The REPL holds one exploration context and exposes read-only commands alongside deliberate interface plan/cancel/apply. Offline sessions cannot apply settings, command history cannot authorize apply, and live mutation requires an exact command confirmation on a terminal. The TUI itself remains read-only and restores tty state on normal exit, handled Ctrl-C/Ctrl-D and Rust unwinding.

All new source remains Apache 2.0. Nuvora and GuestKit were inspected as design/interaction references through GitHub; no Nuvora code or assets are incorporated.

Validation executed in Linux: 40 Rust tests, four real pseudo-terminal tests, Rustfmt, Clippy with warnings denied, offline release compilation, Swift syntax parsing across 16 files, JSON/plist contract checks and design preview inspection. The 19 Swift test methods are included but native SDK compilation, Swift test execution, CoreWLAN, menu bar/window interaction, administrator authorization and actual network mutations still need a Mac run. GitHub Actions supplies that build path but has not been run from this source package.

Risks: network changes can disconnect the Mac and do not roll back automatically; terminal cell width is approximate for wide Unicode labels; unhandled external termination cannot run tty cleanup; masking covers modeled identifiers, not arbitrary inspector text. VPN/802.1X, firewall engines and virtual-device creation remain system-managed boundaries described in docs/FEATURES.md.
