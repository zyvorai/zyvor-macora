# `macora diagnose`

"Why is my network slow?" Most debugging is the same walk by hand: is the link up, does the router answer, does the internet answer, do names resolve, does a web request work, do big packets get through. `diagnose` does that walk bottom-up, stops blaming at the **lowest layer that is actually wrong**, and tells you what to try.

```sh
macora diagnose                    # the full walk, about 8 seconds
macora diagnose quick              # about 2 seconds: skips Wi-Fi details and the packet-size search
macora diagnose to github.com      # also check one site
macora diagnose speed              # also run Apple's networkQuality (about 15 seconds, uses bandwidth)
macora diagnose redact md          # a shareable report with your addresses masked
macora diagnose count 10 interval 30   # repeat, then summarize what flapped
macora diagnose strict             # exit 1 on warnings too (scripts, monitoring)
```

## What it checks

| Step | Question | How it is measured |
| --- | --- | --- |
| **Link** | Is an adapter connected and holding an address? | Interface state and addresses (IPv4 *or* global IPv6). A tunnel is not the link: the physical adapter under it is checked. |
| **Route** | Which connection carries internet traffic? | `route get default`. Info only; a tunnel is reported without naming the VPN. |
| **Wi-Fi** | How good is the signal? | Signal, noise and SNR, band, channel width, PHY and link rate from `system_profiler SPAirPortDataType`. Warn at SNR < 20 dB or signal < −75 dBm, fail below −85 dBm. |
| **Gateway** | Can this Mac reach the router, and how steadily? | 10 pings 0.2 s apart: loss, average and jitter. Warn on any loss, > 50 ms (Wi-Fi) or > 20 ms (cable) average, or > 20 ms jitter. A router that ignores ping but passes traffic is a **warning**, not a failure. |
| **Internet** | Can it reach public servers? | TCP 443 to 1.1.1.1, 8.8.8.8 and 9.9.9.9, plus 10 pings to 1.1.1.1. It fails only when none answers; one blocked target is not blamed on your ISP. |
| **Portal** | Is a sign-in page intercepting web traffic? | `http://captive.apple.com/hotspot-detect.html` must answer `Success`. Checked before everything else in the verdict. |
| **DNS** | Do names resolve, and how fast? | `dig` against **each configured server** (a real name and a random one that forces a fresh lookup), compared with 1.1.1.1, plus the system resolver (what apps use). |
| **HTTPS** | Where does a web request spend its time? | `curl` timing for www.apple.com over IPv4 and IPv6: DNS, TCP, TLS, first byte. |
| **MTU** | How big a packet fits all the way out? | Do-not-fragment pings of many sizes to 1.1.1.1, in parallel rounds. Reports the number (for example 1480) and the usual cause (PPPoE 1492, 6in4 1480, VPNs 1400s). |
| **IPv6** | Does IPv6 work, or does it delay connections? | A global address, a default route, and a TCP connection over IPv6. |
| **Proxy** | Is traffic sent through a proxy? | `scutil --proxy`. Shown only when one is configured. |
| **Speed** | How fast, and how well under load? | Apple's `networkQuality -c`. Only with `speed`. Responsiveness under 200 RPM is flagged (bufferbloat). |
| **Site DNS / HTTPS / path** | Why is *this site* slow? | With `to HOST`: per-server lookups plus the system resolver (so `localhost`, `.local` and VPN names work), the HTTPS timing breakdown, and ping, jitter and path MTU to the site. |

## The verdict

Findings are ranked: **failures outrank warnings; within each, the lowest layer wins**, in this order: sign-in page, link, router, internet, DNS, HTTPS, MTU, Wi-Fi quality, speed, IPv6. The verdict names that one problem, **What to try** gives up to three concrete steps, and **Also noticed** lists the rest.

Exit status: `1` when any step **fails**, `0` otherwise. Warnings do not fail a script unless you pass `strict`.

## Output formats

Text (default), `json`, `md`, `html`. Every format honours `redact`, which masks your router, local addresses and configured DNS servers; the public probe targets (1.1.1.1, 8.8.8.8, 9.9.9.9) stay readable.

JSON keys (older keys are unchanged): `steps[]` with `step`, `status` (`OK` `WARN` `FAIL` `SKIP` `INFO`), `detail` and `ms` (headline latency or `null`); `verdict`; `advice[]`; `also[]`; `target`.

## What it sends

Only when you run it: about 30 ICMP echo requests (router, 1.1.1.1 and path-MTU probes), TCP connections to three public addresses, a handful of DNS lookups to your servers and to 1.1.1.1, and two small HTTPS requests plus one HTTP request to Apple's captive-portal host. `speed` adds Apple's `networkQuality` test (tens of megabytes). Nothing is stored or uploaded, and nothing changes any setting.

## Caveats, stated plainly

- **The Wi-Fi query runs last.** Reading Wi-Fi details makes macOS scan for networks; measured here it turned a 4 ms router into 40–60 ms with 50 ms of jitter while it ran. So it never runs beside the latency probes, which is also why a full run is about 8 seconds and `quick` is about 2.
- **A target that ignores ping cannot be measured for path MTU.** The step says so (INFO) instead of guessing.
- **Router and internet latency vary on busy Wi-Fi.** One run is a snapshot; use `count` and `interval` to see whether something is unsteady.
- **Wi-Fi field names come from `system_profiler` on macOS 27**; on a macOS version that omits them the step degrades to an INFO line.
- The tool reports what it measured. It does not claim a VPN's owner, and a link being up is never presented as proof of internet access.

## How it is tested

`diagnose` is split into pure parts so each can be tested without a broken network: parsers for every tool's output, a path-MTU search, an analyzer from `Facts` to steps/verdict/advice, and renderers. The collector runs against a fake `Runner` that replays recorded output.

- Parsers and the JSON reader: recorded (sanitized) output including failure shapes, plus truncation/mutation fuzzing that must never panic.
- Path-MTU search: exact for **every** MTU from 576 to 1500, never probes a size twice, bounded rounds and probes, tolerates a flaky oracle.
- About 70 scenarios built by breaking a healthy baseline one way at a time (no link, IPv6-only, tunnel route, router ignores ping, one blocked target, dead or slow DNS, captive portal, slow TLS, PMTU 1492/1480/1400, weak or 2.4 GHz Wi-Fi, jitter, proxy, speed, site problems).
- Collector end-to-end against a fake runner, including every tool failing at once.
- Redaction is checked in text, JSON, Markdown and HTML.
- The Rust step names, statuses and the app's list are compared by `scripts/check-contract.py`.
- Mutation checks: deliberately breaking 16 behaviours (for example removing the router-ignores-ping downgrade) must make the suite fail; the survivors found that way became tests.
