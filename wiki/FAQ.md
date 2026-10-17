# FAQ

**Does it change my network settings?** No, unless you preview a change and then add `apply` with administrator rights. Monitoring and diagnostics are read-only.

**What does `diagnose` send over the network?** Only when you run it: about 30 ICMP echoes (router, 1.1.1.1 and path-MTU probes), TCP connections to three public addresses, a few DNS lookups to your servers and 1.1.1.1, two small HTTPS requests and one HTTP request to Apple's captive-portal host. `speed` adds Apple's `networkQuality` test (tens of megabytes). Nothing is stored or uploaded.

**Is anything sent to Zyvor?** No. There is no server, daemon or analytics.

**Is it safe to share a report?** Use `redact`. It masks your router, local addresses and DNS servers in text, JSON, Markdown and HTML. It does not anonymise SSIDs or text you add yourself; read it before posting.

**Does it work on Intel Macs or older macOS?** Releases are for Apple silicon and macOS 14+. Intel has not been verified.

**Why does it say "unavailable"?** Missing counters are never invented.

**Does a `utun` interface mean I am on a VPN?** Not necessarily. Macora does not claim VPN ownership from an interface's presence.

**Can the app change Wi-Fi or DNS?** Yes, with typed validation, a preview and explicit authorization. Those flows (administrator prompts, Wi-Fi join, Location permission) have not been exercised on hardware yet; see [docs/TESTING.md](https://github.com/zyvorai/zyvor-macora/blob/main/docs/TESTING.md).

**Where are the screenshots from?** The README screenshots use simulated demo data (`--demo`); the terminal output is real.
