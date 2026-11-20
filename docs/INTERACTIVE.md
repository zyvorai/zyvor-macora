# Interactive network workspace

## Persistent commands

```sh
macora interactive
# alias: macora repl
# offline, with no network operations or apply:
macora interactive --from tests/fixtures/macos.ifconfig
```

One session holds the current capture, selected interface, filter, baseline and pending plan. `refresh` reloads the source; ordinary show/detail/doctor commands read the current session capture. This is an exploration session, not an auto-refreshing REPL; use the TUI for periodic polling.

```text
select en0
show
detail
family 6
show
family all
baseline
refresh
changes
redact on
report
export 'network evidence.json'
history
!2
quit
```

`menu` / `help` / `?` shows the commands and numeric shortcuts. History remains in memory (maximum 500 entries); recall uses `!NUMBER`, not arrow-key line editing. Quoted paths work. Pipes, redirection, backticks and command chains are refused. Dollar expressions are literal strings, never shell expansion. EOF exits cleanly; the operating system's default Ctrl-C behavior exits the canonical-input REPL.

Settings always start as a plan:

```text
select en0
plan mtu 1400
# Inspect the exact command and its consequence.
apply
# A terminal is required. Type the displayed APPLY /sbin/ifconfig ... line exactly.
```

Apply consumes the pending plan. Selection, filter, family, expansion, baseline, refresh and cancel invalidate it. Invalid plans clear older plans. History recall cannot authorize apply. The CLI does not summon an administrator prompt: an unprivileged caller gets the normal system failure. Use the native app for macOS-owned authorization. Offline sessions never apply settings, even under sudo.

Private exports use mode 0600 and create-new semantics. Do not treat a snapshot as a complete settings backup or rollback profile. IP/MAC masking is narrow and does not anonymize arbitrary labels or history text unrelated to modeled identifiers.

## Full-screen dashboard

```sh
macora tui
macora tui --all --redact --interval 1
macora tui --from tests/fixtures/macos.ifconfig --interval 1
```

| Key | Action |
| --- | --- |
| 1–6 | Overview, Interfaces, Detail, Topology, Diagnostics, Changes |
| Tab / left / right | Cycle pages |
| j / k / down / up | Select interface on Interfaces; scroll other pages |
| Enter | Open selected interface detail |
| / | Enter a filter; Enter accepts and Escape cancels |
| : | Enter a palette action |
| f | Favorite selected interface in this terminal session |
| s | Sort by active state, name or MTU |
| a | Include/collapse system, tunnel and loopback interfaces |
| p | Pause/resume automatic refresh |
| r | Refresh once, including while paused |
| b | Capture configuration baseline |
| d | Open baseline changes |
| ? | Help overlay |
| q / Ctrl-C / Ctrl-D | Exit and restore terminal state |

Palette actions: `overview`, `interfaces`, `detail`, `topology`, `doctor`, `changes`, `select en0`, `baseline`, `pause`, `all`, `redact`, `help`. Input is line-based: type an action and press Enter; it is not a shell and does not accept settings commands. TUI filter entry currently accepts ASCII, while the REPL supports UTF-8 lines.

The TUI never mutates settings. It saves tty attributes using stty, uses an alternate screen, handles Ctrl-C as input, and restores the original attributes on normal exit and Rust panic unwinding. SIGKILL, process abort and unhandled termination signals cannot run cleanup; `stty sane` restores a damaged terminal. The product's live source is Mac-only; offline fixture testing is also available on Linux. Wide Unicode device labels may not align perfectly because the dependency-free renderer counts characters rather than terminal cell width.

![Actual Rust TUI renderer, sanitized offline fixture](tui-preview.png)
