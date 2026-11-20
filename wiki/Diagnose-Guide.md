# Diagnose Guide

`macora diagnose` answers "why is my network slow?" by checking, bottom-up: link, router, internet, sign-in page, DNS, HTTPS, path MTU, Wi-Fi and IPv6. It blames the **lowest layer that is actually wrong** and says what to try.

```sh
macora diagnose                    # the full walk, about 8 seconds
macora diagnose quick              # about 2 seconds: skips Wi-Fi details and the packet-size search
macora diagnose to github.com     # also check one site
macora diagnose speed              # also run Apple's networkQuality (about 15 s, uses bandwidth)
macora diagnose redact md          # a shareable report with your addresses masked
macora diagnose count 10 interval 30   # repeat, then summarise what flapped
macora diagnose strict             # exit 1 on warnings too (scripts, monitoring)
```

## Reading the result

Each line is `OK`, `WARN`, `FAIL`, `SKIP` or `INFO`. The **VERDICT** names the one problem that matters most; **WHAT TO TRY** gives up to three concrete steps; **ALSO NOTICED** lists the rest. Exit status is 1 when a step fails, 0 otherwise (`strict` also fails on warnings).

| Step | What it tells you |
| --- | --- |
| Link | An adapter is connected and holds an address. A tunnel is not the link. |
| Route | Which connection carries internet traffic. Informational. |
| Wi-Fi | Signal, noise, SNR, band, width, rate. Warns at SNR under 20 dB or signal weaker than -75 dBm. |
| Gateway | Your router: loss, average and jitter. A router that ignores ping but passes traffic is a warning, not a failure. |
| Internet | TCP to three public servers and a ping to 1.1.1.1. Fails only if none answers. |
| Portal | A sign-in page intercepting web traffic (captive portal). |
| DNS | Each configured DNS server, uncached, compared with 1.1.1.1, plus what apps actually use. |
| HTTPS | Where a web request spends its time: DNS, TCP, TLS, first byte; IPv4 and IPv6. |
| MTU | The biggest packet that gets out. 1500 is normal; 1492 is typical of PPPoE; 1480 of a 6in4 tunnel; VPNs often use the 1400s. |
| IPv6 | Whether IPv6 works, or delays connections. |
| Speed | Only with `speed`. Low responsiveness under load is flagged (bufferbloat). |
| Site DNS / HTTPS / path | With `to HOST`: why this one site is slow. `localhost`, `.local` and VPN names work. |

## Good to know

- One run is a snapshot. Use `count` and `interval` for intermittent problems.
- Reading Wi-Fi details makes macOS scan, which distorts latency, so it runs last. That is why a full run takes about 8 seconds and `quick` about 2.
- A target that ignores ping cannot have its path MTU measured; the step says so instead of guessing.
- The tool never claims who owns a VPN, and a link being up is never shown as proof of internet access.

Formats: text (default), `json`, `md`, `html`. Every format honours `redact`, which masks your router, local addresses and DNS servers; the public probe targets (1.1.1.1, 8.8.8.8, 9.9.9.9) stay readable.

Full reference: [docs/DIAGNOSE.md](https://github.com/zyvorai/zyvor-macora/blob/main/docs/DIAGNOSE.md).
