use crate::{
    collect, diagnose, export, insight,
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

USAGE
  macora                    Interfaces, grouped and explained
  macora en0                One interface; add raw for ifconfig's own text
  macora list [up|down]     Interface names only, like ifconfig -l
  macora en0 mtu 1400       Preview a change; add apply to run it
  macora COMMAND [WORDS]    e.g. macora report html redact

COMMANDS
  interactive / repl    Persistent exploration, filtering, history and plan previews
  tui                   Full-screen terminal dashboard; / search, : palette, ? help
  topology              Observed bridge membership and interface roles
  report                Markdown local-evidence report (json, html also work)
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
  diagnose              Why is my network slow? Link, router, internet, sign-in page, DNS, HTTPS,
                        path MTU, Wi-Fi and IPv6, with a verdict and what to try
  ping HOST             Packet loss and latency (count N, default 4)
  trace HOST            Hop-by-hop path via traceroute
  watch                 Live counters and traffic rates; Ctrl-C to stop
  snapshot FILE         Save private ifconfig capture, never overwrite
  diff BEFORE AFTER     Compare two ifconfig captures
  subnet IPv4/PREFIX    Network, mask, host range; handles /31 and /32
  probe IP:PORT         Explicit TCP reachability check (IPv6: [::1]:443)
  set IFACE up|down     Preview administrative state change
  set IFACE mtu NUMBER  Preview MTU change; add apply to execute on macOS
  completions SHELL     Print bash, zsh or fish completions

WORDS  (after a command or interface)
  all                   Include tunnels, Apple system links and loopback
  up | down             Only interfaces that are up / down
  compact               Hide media, MAC and cumulative counters
  ipv4 | ipv6           One address family
  raw                   The original ifconfig text, unexplained
  json                  Machine-readable output (watch: NDJSON)
  redact                Mask IP/MAC values
  html | md | prometheus  Output format (report; watch and show for prometheus)
  from FILE             Inspect saved macOS ifconfig output, offline
  interface NAME        Select one interface
  group NAME            Network, Bridges, Virtual, Tunnels, System, Loopback
  count N               Watch frames (omitted means unlimited)
  interval SECONDS      Watch interval 0.2..60 (default 2)
  timeout MS            Probe timeout 1..30000 (default 2000)
  theme NAME            tui palette: default, mono, high-contrast
  color MODE            auto (default), always, never; NO_COLOR is honored
  apply                 Execute a previewed set command; requires privileges
  to HOST               diagnose: also check one site (DNS, HTTPS timing, path)
  quick                 diagnose: ~2 s check without Wi-Fi details and the MTU search
  speed                 diagnose: also run Apple's networkQuality (~15 s, uses bandwidth)
  strict                diagnose: exit 1 on warnings too (default: only on failures)
  count N interval S    diagnose: repeat N times and summarize what flapped

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
    /// ifconfig -l: print interface names only.
    pub list: bool,
    /// ifconfig -d: interfaces that are down.
    pub down: bool,
    /// Print the original ifconfig text instead of the explained view.
    pub raw: bool,
    /// `diagnose to HOST`: also check one site.
    pub to: Option<String>,
    /// `diagnose speed`: run Apple's networkQuality (opt-in; it uses real bandwidth).
    pub speed: bool,
    /// `diagnose strict`: exit 1 on warnings too.
    pub strict: bool,
    /// `diagnose quick`: skip Wi-Fi details and the path-MTU search.
    pub quick: bool,
}
impl Args {
    fn up_and_down(&self) -> bool {
        self.filter.up && self.down
    }
}
/// Words that stand for a flag: `macora en0 raw`, `macora report html redact`.
const FLAG_WORDS: &[(&str, &str)] = &[
    ("all", "--all"),
    ("up", "--up"),
    ("down", "--down"),
    ("compact", "--compact"),
    ("json", "--json"),
    ("redact", "--redact"),
    ("apply", "--apply"),
    ("raw", "--raw"),
    ("list", "--list"),
    ("speed", "--speed"),
    ("quick", "--quick"),
    ("strict", "--strict"),
    ("ipv4", "--ipv4"),
    ("ipv6", "--ipv6"),
];
/// Words that name an output format: `macora report html`.
const FORMAT_WORDS: &[&str] = &["html", "md", "prometheus"];
/// Words followed by a value: `macora watch count 5 interval 1`.
const VALUE_WORDS: &[(&str, &str)] = &[
    ("from", "--from"),
    ("interface", "--interface"),
    ("group", "--group"),
    ("interval", "--interval"),
    ("count", "--count"),
    ("timeout", "--timeout-ms"),
    ("format", "--format"),
    ("theme", "--theme"),
    ("color", "--color"),
    ("to", "--to"),
];
const VALUE_FLAGS: &[&str] = &[
    "--from",
    "--interface",
    "--group",
    "--interval",
    "--count",
    "--timeout-ms",
    "--format",
    "--theme",
    "--color",
    "--to",
];
fn flag_word(t: &str) -> Option<&'static str> {
    FLAG_WORDS.iter().find(|(w, _)| *w == t).map(|(_, f)| *f)
}
/// Positional arguments each command takes before option words begin.
fn positionals(cmd: &str, rest: &[&String]) -> usize {
    match cmd {
        "explain" | "snapshot" | "subnet" | "probe" | "resolve" | "ping" | "trace"
        | "completions" => 1,
        "diff" => 2,
        "set" => {
            if rest.get(1).map(|s| s.as_str()) == Some("mtu") {
                3
            } else {
                2
            }
        }
        _ => 0,
    }
}
/// `macora color always diagnose` means `macora diagnose color always`: a command wins wherever it appears
/// after leading option words.
fn lead_with_command(args: Vec<String>) -> Vec<String> {
    let mut j = 0;
    while j < args.len() {
        let t = args[j].as_str();
        if export::COMMANDS.contains(&t) {
            let mut out = vec![args[j].clone()];
            out.extend(args[..j].iter().cloned());
            out.extend(args[j + 1..].iter().cloned());
            return out;
        }
        if t.starts_with('-') {
            j += if VALUE_FLAGS.contains(&t) { 2 } else { 1 };
        } else if VALUE_WORDS.iter().any(|(w, _)| *w == t) {
            j += 2;
        } else if flag_word(t).is_some() || FORMAT_WORDS.contains(&t) {
            j += 1;
        } else {
            break;
        }
    }
    args
}
/// Translates the word grammar into the flags the parser understands. Dash flags pass through
/// unchanged, so existing scripts keep working.
pub fn words_to_flags(args: Vec<String>) -> Vec<String> {
    let args = lead_with_command(args);
    let mut out: Vec<String> = Vec::with_capacity(args.len() + 2);
    let mut cmd: Option<String> = None;
    let mut left = 0usize;
    let mut i = 0;
    while i < args.len() {
        let t = &args[i];
        i += 1;
        if t.starts_with('-') {
            out.push(t.clone());
            if VALUE_FLAGS.contains(&t.as_str()) && i < args.len() {
                out.push(args[i].clone());
                i += 1;
            }
            continue;
        }
        if cmd.is_none() {
            let rest: Vec<&String> = args[i..].iter().filter(|a| !a.starts_with('-')).collect();
            let known =
                export::COMMANDS.contains(&t.as_str()) || ["help", "version"].contains(&t.as_str());
            let modifier = flag_word(t).is_some()
                || FORMAT_WORDS.contains(&t.as_str())
                || VALUE_WORDS.iter().any(|(w, _)| w == t);
            let name_like = t.len() <= 32
                && t.chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_.:-".contains(c));
            if known {
                left = positionals(t, &rest);
                cmd = Some(t.clone());
                out.push(t.clone());
                continue;
            } else if !modifier && name_like {
                // ifconfig style: `en0` shows it, `en0 up|down|mtu N` previews a change.
                match rest.first().map(|s| s.as_str()) {
                    Some("mtu") => {
                        out.extend(["set".to_string(), t.clone()]);
                        left = 2;
                        cmd = Some("set".into());
                    }
                    Some("up") | Some("down") => {
                        out.extend(["set".to_string(), t.clone()]);
                        left = 1;
                        cmd = Some("set".into());
                    }
                    _ => {
                        out.extend([
                            "show".to_string(),
                            "--interface".into(),
                            t.clone(),
                            "--all".into(),
                        ]);
                        cmd = Some("show".into());
                    }
                }
                continue;
            } else if modifier {
                out.push("show".into());
                cmd = Some("show".into());
            } else {
                out.push(t.clone());
                continue;
            }
        } else if left > 0 {
            out.push(t.clone());
            left -= 1;
            continue;
        }
        if let Some(flag) = flag_word(t) {
            out.push(flag.into());
        } else if FORMAT_WORDS.contains(&t.as_str()) {
            out.extend(["--format".to_string(), t.clone()]);
        } else if let Some((_, flag)) = VALUE_WORDS.iter().find(|(w, _)| *w == t) {
            out.push((*flag).into());
            if i < args.len() {
                out.push(args[i].clone());
                i += 1;
            }
        } else if cmd.as_deref() == Some("show")
            && !out.iter().any(|x| x == "--interface")
            && t.len() <= 32
            && t.chars()
                .all(|c| c.is_ascii_alphanumeric() || "_.:-".contains(c))
        {
            // `macora all en0`, `macora color always en0`: a name after the options is the interface.
            out.extend(["--interface".to_string(), t.clone(), "--all".into()]);
        } else {
            out.push(t.clone());
        }
    }
    out
}
pub fn arguments(args: Vec<String>) -> Result<Args, String> {
    let args = words_to_flags(args);
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
            "--raw" => out.raw = true,
            "--speed" => out.speed = true,
            "--strict" => out.strict = true,
            "--quick" => out.quick = true,
            "--to" => out.to = Some(value(&mut args)?),
            "--list" => out.list = true,
            "--down" => out.down = true,
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
            return Err(format!(
                "Interface {name} not found (if you meant a command, see --help)"
            ));
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
    if a.up_and_down() {
        return Err("Choose -u (up) or -d (down), not both".into());
    }
    let cmd = a.words.first().map(String::as_str).unwrap_or("show");
    let t = Theme {
        color: !a.json
            && a.color.as_deref() != Some("never")
            && (a.color.as_deref() == Some("always")
                || (io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none())),
    };
    if let Some(f) = a.format.as_deref() {
        let ok = match f {
            "md" | "html" => cmd == "report" || cmd == "diagnose",
            "prometheus" => cmd == "watch" || cmd == "show",
            _ => true,
        };
        if !ok {
            return Err(format!("--format {f} is not supported for {cmd}"));
        }
    }
    if (a.list || a.raw || a.down) && cmd != "show" {
        return Err(
            "-l, -d and --raw apply to the interface view (macora [-a|-l|-u|-d] [INTERFACE])"
                .into(),
        );
    }
    if a.theme.is_some() && cmd != "tui" {
        return Err("--theme only applies to tui".into());
    }
    if ["diagnose", "ping", "trace"].contains(&cmd) && a.from.is_some() {
        return Err(format!("{cmd} is a live check; from FILE is not supported"));
    }
    if ["ping", "trace"].contains(&cmd) && a.redact {
        return Err(format!(
            "{cmd} output has nothing to redact; use redact with diagnose"
        ));
    }
    if (a.speed || a.strict || a.quick || a.to.is_some()) && cmd != "diagnose" {
        return Err("speed, quick, strict and to HOST apply to diagnose".into());
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
            "diagnose",
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
        "diagnose" => diagnose_command(&a, &t),
        "help" => emit(HELP),
        "version" => emit(concat!("macora ", env!("CARGO_PKG_VERSION"))),
        "show" => {
            let mut s = select(get(&a)?, &a.filter)?;
            if a.down {
                s.interfaces.retain(|i| !i.up());
            }
            if a.list || a.raw {
                // Like ifconfig: system order, not alphabetical.
                s.interfaces.sort_by_key(|i| i.order);
            }
            if a.list {
                let names: Vec<&str> = s.interfaces.iter().map(|i| i.name.as_str()).collect();
                return emit(&names.join(" "));
            }
            let mut output = if a.raw {
                s.interfaces
                    .iter()
                    .flat_map(|i| i.raw.iter().map(String::as_str))
                    .collect::<Vec<_>>()
                    .join("\n")
            } else if a.format.as_deref() == Some("prometheus") {
                export::prometheus(&s, &BTreeMap::new())
            } else if a.json {
                s.json()
            } else {
                view::interfaces(&s, &a.filter, &t, &BTreeMap::new())
            };
            if a.from.is_some() && !a.json && !a.raw {
                output = format!(
                    "{}\n{output}",
                    t.paint("93", "OFFLINE CAPTURE — not live telemetry")
                );
            }
            if a.redact {
                // Table view: keep columns aligned. Raw/JSON/Prometheus text has no columns to keep.
                let table = !a.raw && !a.json && a.format.as_deref() != Some("prometheus");
                output = if table {
                    view::redact_aligned(output, &s)
                } else {
                    view::redact(output, &s)
                };
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
                    text = if a.json || a.format.as_deref() == Some("prometheus") {
                        view::redact(text, &s)
                    } else {
                        view::redact_aligned(text, &s)
                    };
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

fn diagnose_command(a: &Args, t: &Theme) -> Result<(), String> {
    if let Some(h) = &a.to {
        if !probe::valid_host(h) || h.contains('%') || h.contains(':') {
            return Err("Use a hostname after `to`, like: macora diagnose to github.com".into());
        }
    }
    if !cfg!(target_os = "macos") {
        return Err("diagnose requires macOS".into());
    }
    let opts = diagnose::Options {
        speed: a.speed,
        quick: a.quick,
        target: a.to.clone(),
    };
    let rounds = a.count.unwrap_or(1).min(60);
    let pause = a.interval.unwrap_or(5.0);
    let mut worst = diagnose::Status::Ok;
    let mut history: Vec<diagnose::Report> = Vec::new();
    for round in 0..rounds {
        let report = diagnose::run(&collect::snapshot()?, &opts);
        let show = |text: String| -> String {
            if a.redact {
                diagnose::render::redact(text, &report.secrets)
            } else {
                text
            }
        };
        if rounds == 1 {
            let text = match a.format.as_deref() {
                _ if a.json => diagnose::render::json(&report),
                Some("md") => diagnose::render::markdown(&report),
                Some("html") => diagnose::render::html(&report),
                _ => diagnose::render::text(&report, t),
            };
            emit(&show(text))?;
        } else if a.json {
            emit(&show(diagnose::render::json(&report)))?;
        } else {
            emit(&show(round_line(round + 1, rounds, &report, t)))?;
        }
        worst = worse(worst, report.worst());
        history.push(report);
        if round + 1 < rounds {
            std::thread::sleep(Duration::from_secs_f64(pause));
        }
    }
    if rounds > 1 && !a.json {
        let secrets = history
            .last()
            .map(|r| r.secrets.clone())
            .unwrap_or_default();
        let summary = repeat_summary(&history);
        emit(&if a.redact {
            diagnose::render::redact(summary, &secrets)
        } else {
            summary
        })?;
    }
    diagnose_exit(worst, a.strict)
}

/// The more serious of two outcomes (Fail > Warn > everything else).
pub fn worse(a: diagnose::Status, b: diagnose::Status) -> diagnose::Status {
    use diagnose::Status::*;
    match (a, b) {
        (Fail, _) | (_, Fail) => Fail,
        (Warn, _) | (_, Warn) => Warn,
        _ => Ok,
    }
}

/// `diagnose` exits 1 on a failure, and on a warning too when `strict` was asked for.
pub fn diagnose_exit(worst: diagnose::Status, strict: bool) -> Result<(), String> {
    match worst {
        diagnose::Status::Fail => Err("Diagnosis found a failing layer".into()),
        diagnose::Status::Warn if strict => Err("Diagnosis found a warning (strict)".into()),
        _ => Ok(()),
    }
}

/// One compact line per round when `diagnose count N` repeats the check.
fn round_line(n: u32, of: u32, r: &diagnose::Report, t: &Theme) -> String {
    let (label, code) = match r.worst() {
        diagnose::Status::Fail => ("FAIL", "1;38;5;196"),
        diagnose::Status::Warn => ("WARN", "1;38;5;208"),
        _ => ("OK  ", "1;38;5;46"),
    };
    let bad: Vec<String> = r
        .steps
        .iter()
        .filter(|s| matches!(s.status, diagnose::Status::Fail | diagnose::Status::Warn))
        .map(|s| format!("{} {}", s.name, s.status.label()))
        .collect();
    let lat = |name: &str| {
        r.steps
            .iter()
            .find(|s| s.name == name)
            .and_then(|s| s.ms)
            .map_or(String::new(), |m| format!("{name} {m:.0} ms"))
    };
    let detail = if bad.is_empty() {
        [lat("Gateway"), lat("Internet"), lat("DNS")]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" · ")
    } else {
        bad.join(" · ")
    };
    format!(
        "[{n}/{of}] {}  {}",
        t.paint(code, label),
        view::safe(&detail)
    )
}

/// After a repeated run: which layers were unsteady, and the latency range.
pub fn repeat_summary(history: &[diagnose::Report]) -> String {
    let rounds = history.len();
    let clean = history
        .iter()
        .filter(|r| r.worst() == diagnose::Status::Ok)
        .count();
    let mut out = format!(
        "\n{rounds} rounds: {clean} clean, {} with problems.",
        rounds - clean
    );
    let mut names: Vec<&'static str> = Vec::new();
    for r in history {
        for s in &r.steps {
            if !names.contains(&s.name) {
                names.push(s.name);
            }
        }
    }
    let mut unsteady: Vec<String> = Vec::new();
    for name in &names {
        let bad = history
            .iter()
            .filter(|r| {
                r.steps.iter().any(|s| {
                    s.name == *name
                        && matches!(s.status, diagnose::Status::Fail | diagnose::Status::Warn)
                })
            })
            .count();
        if bad > 0 {
            unsteady.push(format!("{name} ({bad} of {rounds})"));
        }
    }
    if unsteady.is_empty() {
        out += "\nNothing flapped.";
    } else {
        out += &format!("\nUnsteady: {}.", unsteady.join(", "));
    }
    for name in ["Gateway", "Internet", "DNS"] {
        let v: Vec<f64> = history
            .iter()
            .filter_map(|r| r.steps.iter().find(|s| s.name == name).and_then(|s| s.ms))
            .collect();
        if v.len() > 1 {
            let lo = v.iter().cloned().fold(f64::MAX, f64::min);
            let hi = v.iter().cloned().fold(0.0, f64::max);
            let avg = v.iter().sum::<f64>() / v.len() as f64;
            out += &format!("\n{name}: min {lo:.0} / avg {avg:.0} / max {hi:.0} ms");
        }
    }
    out
}
