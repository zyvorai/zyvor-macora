# CLI Guide

Commands and options are **plain words**, not dashes: `macora report html redact`, not `--html --redact`. (The old `--flag` spellings still work in scripts.) Colour turns off in pipes and with `NO_COLOR`; `color always` or `color never` overrides.

## Coming from ifconfig

| You type | Macora equivalent | Notes |
| --- | --- | --- |
| `ifconfig -l` | `macora list` | Same names in the same order. `list up` / `list down` filter. |
| `ifconfig -a` | `macora raw` | `ifconfig`'s own text. Identical apart from MAC lines. |
| `ifconfig en0` | `macora en0 raw` | One interface, original text. |
| (no equivalent) | `macora en0` | Same interface, explained in words. |
| `ifconfig en0 mtu 1400` | `sudo macora en0 mtu 1400 apply` | Without `apply` it only previews. |

## Everyday commands

```sh
macora                         # connected adapters first, system groups collapsed
macora all                     # everything, including tunnels and loopback
macora explain en0             # flags, IPv6 and role in plain words
macora doctor                  # local findings, no network traffic
macora diagnose                # why is my network slow?  (see Diagnose Guide)
macora ping 1.1.1.1 count 5
macora trace example.com
macora resolve example.com
macora probe 1.1.1.1:443 timeout 1500
macora subnet 192.168.1.8/24
macora routes | dns | wifi | services | neighbors | sockets | path | topology
macora tui                     # full-screen workspace; / search, : palette, ? help
```

## Words you can add

`all` · `up` / `down` · `compact` · `ipv4` / `ipv6` · `raw` · `json` · `redact` · `html` / `md` / `prometheus` · `from FILE` · `interface NAME` · `group NAME` · `count N` · `interval SECONDS` · `timeout MS` · `theme NAME` · `color MODE` · `apply`

`macora help` lists them all; `man macora` has the full page.

## Reports, history and automation

```sh
macora report html redact > report.html     # shareable, addresses masked
macora snapshot before.macora               # private capture (mode 0600), never overwrites
macora diff before.macora after.macora redact
macora all from before.macora               # inspect a capture offline
macora watch count 10 interval 1 json       # NDJSON frames with counters and rates
macora watch prometheus                     # text exposition for scraping
macora completions zsh > ~/.zsh/completions/_macora    # also bash, fish
```

## Changing settings is always a two-step

`macora en0 mtu 1400` prints a preview and changes nothing. Add `apply` (and administrator rights) to run it. The device may still refuse; the error is shown, never hidden.

## What redaction does and does not do

`redact` masks IP and MAC addresses that are in the interface model. It does not anonymise interface labels, service names, SSIDs, proxy settings or text you copy yourself. Raw captures and full exports are deliberately left unredacted.
