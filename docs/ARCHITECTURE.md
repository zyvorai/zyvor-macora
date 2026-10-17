# Implementation

The SwiftUI application runs the bundled Rust `macora show --json --all` collector at the selected 1/2/5/10-second interval. Rust calls fixed absolute macOS tool paths with argv arrays and `LC_ALL=C`; it never invokes a shell for inspection. `ifconfig -a` is the source of interface data, `networksetup -listallhardwareports` supplies human hardware names, and `netstat -ibn` supplies cumulative link counters. The raw capture route preserves all source fields even when a field has no dedicated JSON property.

JSON schema version 1 connects the two languages. The Swift model rejects unknown versions. The UI calculates byte deltas using actual elapsed sample time and unsigned reset handling. Only Network-group counters contribute to its overview; bridges and tunnels can repeat the same packets. No counter means no invented rate. The chart stores at most 90 samples in memory.

Settings use a separate Swift `CommandPlan`. Existing service/interface names and typed address/port/range inputs are validated before preview. Applying requires a deliberate button. The shell command is constructed from independently single-quoted arguments, then passed as an argv value to a **constant** AppleScript program. No user value is inserted into AppleScript source. macOS presents and owns the administrator password dialog. MACORA does not capture this password or cache authorization. This is a developer-distributed app design, not a sandboxed App Store helper architecture.

CoreWLAN scanning and joining operate only after user actions. Location permission is requested by a dedicated button; denied permission is surfaced. Personal-network passwords are passed to CoreWLAN in memory and omitted from commands, exports and event history. Strings cannot be securely zeroized by Swift, so no memory-zeroization guarantee is made. Enterprise identity provisioning remains in System Settings.

Full configuration changes persist according to Apple's networking implementation; the app supplies no automatic undo. CLI snapshots record interface state, not a complete restorable settings profile. The UI event list records successful configuration actions during the current session, not a durable security audit log.

No app server, remote connection, analytics SDK, package runtime dependency, packet capture or private networking API is included. System inspection may still reveal personal addresses and service data, so exports have explicit full/redacted actions and private permissions. IP/MAC redaction is narrow and does not cover arbitrary text copied from scoped DNS or route inspectors.

Source builds support the host Mac architecture. Cross-platform Rust parsing/test support exists for CI, but live collection and the product are Mac-only. OS-specific functionality is deliberately explicit rather than a successful empty result on unsupported systems.


The Rust interactive session is a separate state machine that retains a capture, baseline, selection, query, history and a pending plan. The tokenizer returns argv strings without shell evaluation. Invalid or context-changing inputs discard pending configuration, and history cannot grant mutation authorization. Applying requires a live session, a tty, exact command confirmation and caller-owned privileges; the existing CLI revalidates the device against a fresh live capture.

The TUI uses the same snapshot model, independent pure state/rendering, an input channel and stty-based terminal ownership. It has no settings-apply action. Raw mode disables terminal-generated signals, so Ctrl-C/Ctrl-D are handled as quit keys and the terminal guard restores saved attributes. Fatal external termination cannot run the guard. The Python PTY tests verify real key routing and tty restoration, beyond pure render tests.

The native store owns one polling task shared by the window and menu bar. UI preferences contain no credentials. Typed Swift comparisons ignore byte counters, sort address/flag sets, and track presence, admin/link state, MTU, media, MAC, bridge members and address attributes. Comparison baselines remain distinct from live configuration inventory. The UI observes at most 200 field changes and keeps them only in memory.
