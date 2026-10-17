use crate::{
    collect, export, insight,
    model::{json_string, Snapshot},
    parse, probe,
    view::{self, Filter, Theme},
};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{self, IsTerminal, Write},
    net::{SocketAddr, TcpStream},
    time::{Duration, Instant},
};

pub const HELP: &str = "MACORA by Zyvor — Your Mac’s network, made clear.

USAGE  macora [COMMAND] [OPTIONS]

  interactive / repl    Persistent exploration, filtering, history and plan previews
  tui                   Full-screen terminal dashboard; / search, : palette, ? help
  topology              Observed bridge membership and interface roles
  report                Markdown local-evidence report (--json supported)
  resolve HOST          Explicit macOS DNS lookup via dscacheutil
  show                  Grouped interfaces (default)
  explain INTERFACE     Explain flags, IPv6 and interface roles
  routes                IPv4 and IPv6 routing tables
  path                  Default route and macOS network reachability state
  dns                   All scoped macOS DNS resolvers
  services              Network service order and hardware ports
  wifi                  Wi-Fi power/association and preferred networks
  neighbors             ARP and IPv6 neighbor tables
  sockets               TCP/UDP sockets visible to your user
  doctor                Local interface findings (no external probes)
  diagnose              Why is my network slow? Gateway, internet, DNS and MTU checks
  ping HOST             Packet loss and latency (--count, default 4)
  trace HOST            Hop-by-hop path via traceroute
  completions SHELL     Print bash, zsh or fish completions
  watch                 Live counters and traffic rates; Ctrl-C to stop
  snapshot FILE         Save private ifconfig capture, never overwrite
  diff BEFORE AFTER     Compare two ifconfig captures
  subnet IPv4/PREFIX    Network, mask, host range; handles /31 and /32
  probe IP:PORT         Explicit TCP reachability check (IPv6: [::1]:443)
  set IFACE up|down     Preview administrative state change
  set IFACE mtu NUMBER  Preview MTU change; --apply executes on macOS

OPTIONS
  --all                 Expand tunnels, Apple system links and loopback
  --up                  Administratively up interfaces only
  --interface NAME      Select one interface
  --group NAME          Network, Bridges, Virtual, Tunnels, System, Loopback
  --ipv4 | --ipv6       Address family filter
  --compact             Hide media, MAC and cumulative counters
  --json                Machine-readable output (watch: NDJSON)
  --format FMT          table (default), json, ndjson; report: md, html; watch/show: prometheus
  --theme NAME          tui colors: default, mono, high-contrast
  --redact              Mask IP/MAC values in show/explain/doctor/diff
  --from FILE           Inspect saved macOS ifconfig output, offline
  --color MODE          auto (default), always, never; honors NO_COLOR
  --interval SECONDS    Watch interval 0.2..60 (default 2)
  --count NUMBER        Watch frames; omitted means unlimited
  --timeout-ms NUMBER   Probe timeout 1..30000 (default 2000)
  --apply               Execute a previewed set command; requires privileges
  -h, --help            Help
  -V, --version         Version

Use the SwiftUI app for DNS, proxy, DHCP/static IP, Wi-Fi and service management.
Live collection and configuration require macOS. No telemetry, no background server.
";
#[derive(Default)]
pub struct Args {
    pub words: Vec<String>,
    pub filter: Filter,
    pub json: bool,
    pub format: Option<String>,
    pub theme: Option<String>,
    pub redact: bool,
    pub from: Option<String>,
    pub color: Option<String>,
    pub interval: Option<f64>,
    pub count: Option<u32>,
    pub timeout_ms: Option<u64>,
    pub apply: bool,
}
pub fn arguments(args: Vec<String>) -> Result<Args, String> {
    let mut out = Args::default();
    let mut args = args.into_iter();
    while let Some(a) = args.next() {
        let value = |args: &mut std::vec::IntoIter<String>| {
            args.next().ok_or_else(|| format!("{a} needs a value"))
        };
        match a.as_str() {
            "--all" => out.filter.all = true,
            "--up" => out.filter.up = true,
            "--compact" => out.filter.compact = true,
            "--json" => out.json = true,
            "--redact" => out.redact = true,
            "--apply" => out.apply = true,
            "--ipv4" | "--ipv6" => {
                let family = if a == "--ipv4" { 4 } else { 6 };
                if out.filter.family.is_some() {
                    return Err("Choose one address family".into());
                }
                out.filter.family = Some(family);
            }
            "--interface" => out.filter.interface = Some(value(&mut args)?),
            "--group" => {
                let g = value(&mut args)?;
                if ![
                    "Network", "Bridges", "Virtual", "Tunnels", "System", "Loopback",
                ]
                .iter()
                .any(|s| s.eq_ignore_ascii_case(&g))
                {
                    return Err("Unknown group".into());
                }
                out.filter.group = Some(g);
            }
            "--format" => {
                let f = value(&mut args)?;
                if !export::FORMATS.contains(&f.as_str()) {
                    return Err(format!(
                        "Format must be one of: {}",
                        export::FORMATS.join(", ")
                    ));
                }
                out.json |= f == "json" || f == "ndjson";
                out.format = Some(f);
            }
            "--theme" => {
                let t = value(&mut args)?;
                if !export::THEMES.contains(&t.as_str()) {
                    return Err(format!(
                        "Theme must be one of: {}",
                        export::THEMES.join(", ")
                    ));
                }
                out.theme = Some(t);
            }
            "--from" => out.from = Some(value(&mut args)?),
            "--no-color" => out.color = Some("never".into()),
            "--color" => {
                let s = value(&mut args)?;
                if !["auto", "always", "never"].contains(&s.as_str()) {
                    return Err("Color must be auto, always or never".into());
                }
                out.color = Some(s);
            }
            "--interval" => {
                let n: f64 = value(&mut args)?.parse().map_err(|_| "Invalid interval")?;
                if !n.is_finite() || !(0.2..=60.0).contains(&n) {
                    return Err("Interval must be 0.2..60 seconds".into());
                }
                out.interval = Some(n);
            }
            "--count" => {
                let n = value(&mut args)?
                    .parse::<u32>()
                    .map_err(|_| "Invalid count")?;
                if n == 0 {
                    return Err("Count must be positive".into());
                }
                out.count = Some(n);
            }
            "--timeout-ms" => {
                let n = value(&mut args)?
                    .parse::<u64>()
                    .map_err(|_| "Invalid timeout")?;
                if !(1..=30000).contains(&n) {
                    return Err("Timeout must be 1..30000 ms".into());
                }
                out.timeout_ms = Some(n);
            }
            "--help" | "-h" => out.words = vec!["help".into()],
            "--version" | "-V" => out.words = vec!["version".into()],
            s if s.starts_with('-') => return Err(format!("Unknown option {s}; use --help")),
            _ => out.words.push(a),
        }
    }
    Ok(out)
}
fn emit(text: &str) -> Result<(), String> {
    let mut stdout = io::stdout().lock();
    stdout
        .write_all(text.as_bytes())
        .and_then(|_| stdout.write_all(b"\n"))
        .map_err(|e| {
            if e.kind() == io::ErrorKind::BrokenPipe {
                "broken pipe".into()
            } else {
                e.to_string()
            }
        })
}
fn load(path: &str) -> Result<Snapshot, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    if text.len() > 8 * 1024 * 1024 {
        return Err("Capture exceeds 8 MiB".into());
    }
    parse::ifconfig(&text)
}
fn get(a: &Args) -> Result<Snapshot, String> {
    match &a.from {
        Some(p) => load(p),
        None => collect::snapshot(),
    }
}
fn select(mut s: Snapshot, f: &Filter) -> Result<Snapshot, String> {
    if let Some(name) = &f.interface {
        if !s.interfaces.iter().any(|i| i.name == *name) {
            return Err(format!("Interface {name} not found"));
        }
    }
    s.interfaces.retain(|i| f.matches(i));
    for i in &mut s.interfaces {
        i.addresses.retain(|a| match f.family {
            Some(4) => !a.ip.contains(':'),
            Some(6) => a.ip.contains(':'),
            _ => true,
        });
    }
    Ok(s)
}
pub fn set_plan(words: &[String], s: &Snapshot) -> Result<Vec<String>, String> {
    if words.len() < 3 {
        return Err("Use set IFACE up|down|mtu NUMBER".into());
    }
    let name = &words[1];
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "_.:-".contains(c))
        || !s.interfaces.iter().any(|i| i.name == *name)
    {
        return Err("Select an existing interface".into());
    }
    match words[2].as_str() {
        "up" | "down" if words.len() == 3 => Ok(vec![name.clone(), words[2].clone()]),
        "mtu" if words.len() == 4 => {
            let n: u32 = words[3].parse().map_err(|_| "Invalid MTU")?;
            if !(576..=9000).contains(&n) {
                return Err("MTU must be 576..9000; device limits may be narrower".into());
            }
            Ok(vec![name.clone(), "mtu".into(), n.to_string()])
        }
        _ => Err("Use set IFACE up|down|mtu NUMBER".into()),
    }
}
fn system_view(cmd: &str) -> Result<String, String> {
    if !cfg!(target_os = "macos") {
        return Err("Live network management requires macOS".into());
    }
    match cmd {
        "routes" => collect::routes(),
        "dns" => collect::dns(),
        "services" => collect::command("/usr/sbin/networksetup", &["-listnetworkserviceorder"]),
        "path" => Ok(format!(
            "DEFAULT IPv4\n{}\nDEFAULT IPv6\n{}\nREACHABILITY\n{}",
            collect::command("/sbin/route", &["-n", "get", "default"]).unwrap_or_else(|e| e),
            collect::command("/sbin/route", &["-n", "get", "-inet6", "default"])
                .unwrap_or_else(|e| e),
            collect::command("/usr/sbin/scutil", &["--nwi"])?
        )),
        "neighbors" => Ok(format!(
            "IPv4 ARP\n{}\nIPv6 NEIGHBORS\n{}",
            collect::command("/usr/sbin/arp", &["-an"])?,
            collect::command("/usr/sbin/ndp", &["-an"])?
        )),
        "sockets" => collect::command("/usr/sbin/lsof", &["-nP", "-i"]),
        "wifi" => {
            let ports = parse::hardware_ports(&collect::command(
                "/usr/sbin/networksetup",
                &["-listallhardwareports"],
            )?);
            let device = ports
                .iter()
                .find(|(_, label)| label.as_str() == "Wi-Fi" || label.as_str() == "AirPort")
                .map(|(d, _)| d)
                .ok_or("No Wi-Fi hardware port found")?;
            Ok(format!(
                "{}\n{}\n{}\nSSID visibility may depend on macOS privacy permissions.\n",
                collect::command("/usr/sbin/networksetup", &["-getairportpower", device])?,
                collect::command("/usr/sbin/networksetup", &["-getairportnetwork", device])
                    .unwrap_or_else(|e| e),
                collect::command(
                    "/usr/sbin/networksetup",
                    &["-listpreferredwirelessnetworks", device]
                )
                .unwrap_or_else(|e| e)
            ))
        }
        _ => Err("Unknown system command".into()),
    }
}
pub fn run(args: Vec<String>) -> Result<(), String> {
    let a = arguments(args)?;
    let cmd = a.words.first().map(String::as_str).unwrap_or("show");
    let t = Theme {
        color: !a.json
            && a.color.as_deref() != Some("never")
            && (a.color.as_deref() == Some("always")
                || (io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none())),
    };
    if let Some(f) = a.format.as_deref() {
        let ok = match f {
            "md" | "html" => cmd == "report",
            "prometheus" => cmd == "watch" || cmd == "show",
            _ => true,
        };
        if !ok {
            return Err(format!("--format {f} is not supported for {cmd}"));
        }
    }
    if a.theme.is_some() && cmd != "tui" {
        return Err("--theme only applies to tui".into());
    }
    if ["diagnose", "ping", "trace"].contains(&cmd) && (a.from.is_some() || a.redact) {
        return Err(format!(
            "{cmd} is a live check; --from and --redact are not supported"
        ));
    }
    if a.apply && cmd != "set" {
        return Err("--apply is only valid with set".into());
    }
    if a.redact
        && ![
            "show",
            "explain",
            "doctor",
            "diff",
            "watch",
            "report",
            "topology",
            "interactive",
            "repl",
            "tui",
        ]
        .contains(&cmd)
    {
        return Err(
            "--redact is supported for interface views, reports and interactive workspaces".into(),
        );
    }
    if a.from.is_some()
        && ![
            "show",
            "explain",
            "doctor",
            "snapshot",
            "set",
            "report",
            "topology",
            "interactive",
            "repl",
            "tui",
        ]
        .contains(&cmd)
    {
        return Err("--from supports offline interface views, reports, snapshots and interactive workspaces".into());
    }
    if ["interactive", "repl", "tui"].contains(&cmd)
        && (a.filter.up
            || a.filter.compact
            || a.filter.family.is_some()
            || a.filter.interface.is_some()
            || a.filter.group.is_some())
    {
        return Err("Use the workspace's select, filter, family and all controls inside interactive/TUI mode".into());
    }
    let expected = match cmd {
        "explain" | "snapshot" | "subnet" | "probe" | "resolve" | "ping" | "trace"
        | "completions" => Some(2),
        "diff" => Some(3),
        "set" => None,
        _ => Some(1),
    };
    if let Some(n) = expected {
        if a.words.len() > n || (n > 1 && a.words.len() != n) {
            return Err(format!("Incorrect arguments for {cmd}; use --help"));
        }
    }
    match cmd {
        "interactive" | "repl" => {
            if a.json || a.apply {
                return Err("Interactive sessions use their own export/plan commands".into());
            }
            crate::session::run(a.from.clone(), t.color, a.redact, a.filter.all)
        }
        "tui" => {
            if a.json || a.apply {
                return Err(
                    "TUI is a terminal inspection surface; use show --json for export".into(),
                );
            }
            crate::tui::run(
                a.from.clone(),
                a.interval.unwrap_or(2.0),
                t.color,
                a.redact,
                a.filter.all,
                a.theme.as_deref().unwrap_or("default"),
            )
        }
        "report" | "topology" => {
            let s = get(&a)?;
            let mut output = if cmd == "report" && a.format.as_deref() == Some("html") {
                export::html_report(&s)
            } else if cmd == "report" {
                crate::explore::report(&s, a.json)
            } else {
                crate::explore::topology(&s, a.json)
            };
            if a.redact {
                output = view::redact(output, &s);
            }
            emit(&output)
        }
        "resolve" => {
            let host = &a.words[1];
            if host.len() > 253
                || host.starts_with('-')
                || !host
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || ".-_".contains(c))
            {
                return Err("Use a hostname, not a URL or shell expression".into());
            }
            if !cfg!(target_os = "macos") {
                return Err("System DNS resolution requires macOS".into());
            }
            let output =
                collect::command("/usr/bin/dscacheutil", &["-q", "host", "-a", "name", host])?;
            if output.trim().is_empty() {
                return Err("No DNS result returned".into());
            }
            emit(&if a.json {
                format!(
                    "{{\"host\":{},\"output\":{}}}",
                    json_string(host),
                    json_string(&output)
                )
            } else {
                view::safe(&output)
            })
        }
        "completions" => emit(export::completions(&a.words[1])?.trim_end()),
        "ping" => {
            let count = a.count.unwrap_or(4).min(50);
            let stats = probe::ping_stats(&a.words[1], count, None)?;
            emit(&if a.json {
                probe::ping_json(&a.words[1], &stats)
            } else {
                probe::ping_text(&a.words[1], &stats)
            })?;
            if stats.received == 0 {
                return Err("No replies received".into());
            }
            Ok(())
        }
        "trace" => {
            let out = probe::trace(&a.words[1])?;
            emit(&if a.json {
                format!(
                    "{{\"host\":{},\"output\":{}}}",
                    json_string(&a.words[1]),
                    json_string(&out)
                )
            } else {
                view::safe(out.trim_end())
            })
        }
        "diagnose" => {
            if !cfg!(target_os = "macos") {
                return Err("diagnose requires macOS".into());
            }
            let steps = probe::diagnose(&collect::snapshot()?);
            emit(&if a.json {
                probe::steps_json(&steps)
            } else {
                steps
                    .iter()
                    .map(|s| {
                        let code = match s.status {
                            probe::Status::Ok => "92",
                            probe::Status::Warn => "93",
                            probe::Status::Fail => "91",
                            probe::Status::Skip => "90",
                        };
                        format!(
                            "{} {:<9} {}",
                            t.paint(code, &format!("{:<5}", s.status.label())),
                            s.name,
                            view::safe(&s.detail)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
                    + &format!("\n\n{}  {}", t.title("VERDICT"), probe::verdict(&steps))
            })?;
            if steps.iter().any(|s| s.status == probe::Status::Fail) {
                return Err("Diagnosis found a failing layer".into());
            }
            Ok(())
        }
        "help" => emit(HELP),
        "version" => emit(concat!("macora ", env!("CARGO_PKG_VERSION"))),
        "show" => {
            let s = select(get(&a)?, &a.filter)?;
            let mut output = if a.format.as_deref() == Some("prometheus") {
                export::prometheus(&s, &BTreeMap::new())
            } else if a.json {
                s.json()
            } else {
                view::interfaces(&s, &a.filter, &t, &BTreeMap::new())
            };
            if a.from.is_some() && !a.json {
                output = format!(
                    "{}\n{output}",
                    t.paint("93", "OFFLINE CAPTURE — not live telemetry")
                );
            }
            if a.redact {
                output = view::redact(output, &s);
            }
            emit(&output)
        }
        "explain" | "doctor" => {
            let s = get(&a)?;
            let mut output = if cmd == "explain" {
                let i = s
                    .interfaces
                    .iter()
                    .find(|i| i.name == a.words[1])
                    .ok_or("Interface not found")?;
                let text = insight::explain(i);
                if a.json {
                    format!("{{\"explanation\":{}}}", json_string(&text))
                } else {
                    t.paint("36", &text)
                }
            } else {
                let findings = insight::findings(&s);
                if a.json {
                    insight::list_json("findings", &findings)
                } else {
                    findings
                        .iter()
                        .map(|s| t.paint(if s.starts_with("WARN") { "93" } else { "36" }, s))
                        .collect::<Vec<_>>()
                        .join("\n")
                }
            };
            if a.redact {
                output = view::redact(output, &s);
            }
            emit(&output)
        }
        "routes" | "dns" | "services" | "path" | "wifi" | "neighbors" | "sockets" => {
            let text = system_view(cmd)?;
            emit(&if a.json {
                format!(
                    "{{\"command\":{},\"output\":{}}}",
                    json_string(cmd),
                    json_string(&text)
                )
            } else {
                format!("{}\n{}", t.title(&cmd.to_uppercase()), t.paint("36", &text))
            })
        }
        "snapshot" => {
            let text = match &a.from {
                Some(p) => fs::read_to_string(p).map_err(|e| e.to_string())?,
                None => collect::raw()?,
            };
            parse::ifconfig(&text)?;
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options
                .open(&a.words[1])
                .map_err(|e| format!("Cannot create capture: {e}"))?;
            file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
            emit(&if a.json {
                format!("{{\"saved\":{}}}", json_string(&a.words[1]))
            } else {
                format!(
                    "Saved {} (private network identifiers included)",
                    view::safe(&a.words[1])
                )
            })
        }
        "diff" => {
            let before = load(&a.words[1])?;
            let after = load(&a.words[2])?;
            let changes = insight::diff(&before, &after);
            let mut text = if a.json {
                insight::list_json("changes", &changes)
            } else if changes.is_empty() {
                "No interface configuration changes.".into()
            } else {
                changes
                    .iter()
                    .map(|s| t.paint("36", s))
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            if a.redact {
                text = view::redact(view::redact(text, &before), &after);
            }
            emit(&text)
        }
        "subnet" => emit(&insight::subnet(&a.words[1], a.json)?),
        "probe" => {
            let address = a.words[1]
                .parse::<SocketAddr>()
                .map_err(|_| "Use a numeric IP:port, e.g. 1.1.1.1:443 or [::1]:443")?;
            let start = Instant::now();
            let result = TcpStream::connect_timeout(
                &address,
                Duration::from_millis(a.timeout_ms.unwrap_or(2000)),
            );
            let ms = start.elapsed().as_secs_f64() * 1000.0;
            let message = match &result {
                Ok(_) => "TCP connection established".into(),
                Err(e) => e.to_string(),
            };
            emit(&if a.json {
                format!(
                    "{{\"target\":{},\"reachable\":{},\"elapsed_ms\":{:.2},\"message\":{}}}",
                    json_string(&address.to_string()),
                    result.is_ok(),
                    ms,
                    json_string(&message)
                )
            } else {
                format!("{} · {:.2} ms · {}", address, ms, message)
            })?;
            if result.is_err() {
                return Err("TCP probe failed".into());
            }
            Ok(())
        }
        "set" => {
            if a.from.is_some() && a.apply {
                return Err("Cannot apply a change based on an offline capture".into());
            }
            let plan = set_plan(&a.words, &get(&a)?)?;
            let rendered = format!("/sbin/ifconfig {}", plan.join(" "));
            if a.apply {
                if !cfg!(target_os = "macos") {
                    return Err("Configuration requires macOS".into());
                }
                collect::command(
                    "/sbin/ifconfig",
                    &plan.iter().map(String::as_str).collect::<Vec<_>>(),
                )?;
            }
            emit(&if a.json {
                format!(
                    "{{\"applied\":{},\"command\":{}}}",
                    a.apply,
                    json_string(&rendered)
                )
            } else {
                format!(
                    "{}: {}{}",
                    if a.apply { "Applied" } else { "Preview" },
                    rendered,
                    if a.apply {
                        ""
                    } else {
                        "\nRe-run with --apply and appropriate privileges. Changes may disconnect this Mac."
                    }
                )
            })
        }
        "watch" => {
            let mut old: Option<(Snapshot, Instant)> = None;
            for frame in 0..a.count.unwrap_or(u32::MAX) {
                let s = select(collect::snapshot()?, &a.filter)?;
                let now = Instant::now();
                let rates = old
                    .as_ref()
                    .map(|(previous, at)| {
                        view::rates(previous, &s, now.duration_since(*at).as_secs_f64())
                    })
                    .unwrap_or_default();
                let mut text = if a.format.as_deref() == Some("prometheus") {
                    export::prometheus(&s, &rates)
                } else if a.json {
                    let rate_json = rates.iter().map(|(name, (rx, tx))| format!("{}:{{\"rx_bytes_per_second\":{:.2},\"tx_bytes_per_second\":{:.2}}}", json_string(name), rx, tx)).collect::<Vec<_>>().join(",");
                    format!(
                        "{{\"frame\":{},\"snapshot\":{},\"rates\":{{{}}}}}",
                        frame,
                        s.json(),
                        rate_json
                    )
                } else {
                    view::interfaces(&s, &a.filter, &t, &rates)
                };
                if a.redact {
                    text = view::redact(text, &s);
                }
                if !a.json && io::stdout().is_terminal() {
                    text = format!("\x1b[2J\x1b[H{text}");
                }
                emit(&text)?;
                old = Some((s, now));
                if frame + 1 < a.count.unwrap_or(u32::MAX) {
                    std::thread::sleep(Duration::from_secs_f64(a.interval.unwrap_or(2.0)));
                }
            }
            Ok(())
        }
        _ => Err(format!("Unknown command {cmd}; use --help")),
    }
}
