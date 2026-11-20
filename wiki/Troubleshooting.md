# Troubleshooting

## `macora` shows an old version after I installed a new one

An older copy earlier on your `PATH` wins. Check with `which -a macora`, then remove the old file or put Homebrew's location first. Test the installed path directly, for example `$(brew --prefix macora)/bin/macora version`.

## macOS says the command line tool cannot be opened

Only the DMG and pkg are notarized. For the tarball: `xattr -d com.apple.quarantine macora` once.

## `diagnose` says the path MTU is 1480 (or 1492)

It measured the biggest packet that gets out to the internet. 1492 is typical of PPPoE and 1480 of an IPv6-in-IPv4 tunnel; VPNs and some security software lower it too. Compare with the router: if full-size packets reach the router but not the internet, something between your Mac and the internet shortens them. Two Macs on the same router can measure differently when one has a tunnel, extension or filter in the path. This is a warning, not an error; many networks work fine at 1480.

## `diagnose` warns "router does not answer ping"

Many routers and guest networks ignore ping while passing traffic. If Internet and DNS are OK it is only a warning.

## `diagnose to HOST` fails on HTTPS for `localhost`

`https://localhost` needs something listening on port 443. A DNS warning for `localhost` is expected: DNS servers do not know it, but the system resolver does.

## Wi-Fi details are missing

They come from `system_profiler`; on a macOS version that omits a field the step falls back to an INFO line. `quick` skips Wi-Fi details on purpose.

## Opening the app from a script hangs

`Macora.app/Contents/MacOS/Macora` starts the GUI and never returns, even with `--help`. Use the bundled command line tool, `Macora.app/Contents/Resources/macora`.

## Completions suggest odd words after `diagnose to`

Fixed in 0.4.1: after `to` the next word is a host, and nothing is offered. Update, and regenerate with `macora completions zsh|bash|fish`.

## Still stuck?

Run `macora diagnose redact md`, read it for anything private, and [open an issue](https://github.com/zyvorai/zyvor-macora/issues/new/choose) with the output and `macora version`.
