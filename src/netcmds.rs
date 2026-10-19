//! Commands built on fixed macOS tools: Wi-Fi scan, per-app traffic, LAN discovery, hop statistics,
//! DNS comparison, firewall status and an iperf3 wrapper. Parsers are pure so they can be tested on
//! sanitized text; the runners only call absolute-path executables, never a shell.
use crate::{collect, json, model::json_string, probe};
use std::collections::BTreeMap;

// ---------------------------------------------------------------- Wi-Fi scan

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Ap {
    pub ssid: String,
    pub channel: u32,
    pub band: String,
    pub width: String,
    pub phy: String,
    pub security: String,
    pub signal: Option<i32>,
    pub current: bool,
}

fn apply_ap_field(ap: &mut Ap, key: &str, value: &str) {
    match key {
        "PHY Mode" => ap.phy = value.into(),
        "Security" => ap.security = value.into(),
        "Channel" => {
            ap.channel = value
                .split_whitespace()
                .next()
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);
            if let Some((_, rest)) = value.split_once('(') {
                let mut parts = rest.trim_end_matches(')').split(',').map(str::trim);
                ap.band = parts.next().unwrap_or("").into();
                ap.width = parts.next().unwrap_or("").into();
            }
        }
        "Signal / Noise" => {
            ap.signal = value.split_whitespace().next().and_then(|n| n.parse().ok())
        }
        _ => {}
    }
}

/// Parses the "Current Network Information" and "Other Local Wi-Fi Networks" blocks of
/// `system_profiler SPAirPortDataType`.
pub fn parse_wifi_scan(text: &str) -> Vec<Ap> {
    let mut aps: Vec<Ap> = Vec::new();
    let mut section: Option<(usize, bool)> = None;
    let mut cur: Option<Ap> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if let Some((sec_indent, is_current)) = section {
            if indent <= sec_indent {
                aps.extend(cur.take());
                section = None;
            } else if indent == sec_indent + 2 && trimmed.ends_with(':') {
                aps.extend(cur.take());
                cur = Some(Ap {
                    ssid: trimmed.trim_end_matches(':').to_string(),
                    current: is_current,
                    ..Ap::default()
                });
                continue;
            } else {
                if let (Some(ap), Some((k, v))) = (cur.as_mut(), trimmed.split_once(": ")) {
                    apply_ap_field(ap, k, v);
                }
                continue;
            }
        }
        match trimmed {
            "Current Network Information:" => section = Some((indent, true)),
            "Other Local Wi-Fi Networks:" => section = Some((indent, false)),
            _ => {}
        }
    }
    aps.extend(cur.take());
    // The connected network is listed again among the others; keep the detailed entry only.
    let current: Vec<(String, u32)> = aps
        .iter()
        .filter(|a| a.current)
        .map(|a| (a.ssid.clone(), a.channel))
        .collect();
    aps.retain(|a| a.current || !current.contains(&(a.ssid.clone(), a.channel)));
    aps.retain(|a| a.channel != 0);
    aps
}

/// Networks competing for a channel: on 2.4 GHz channels closer than 5 apart overlap.
pub fn channel_load(aps: &[Ap], band: &str, channel: u32) -> usize {
    aps.iter()
        .filter(|a| a.band == band)
        .filter(|a| {
            if band == "2GHz" {
                a.channel.abs_diff(channel) < 5
            } else {
                a.channel == channel
            }
        })
        .count()
}

fn candidates(band: &str) -> &'static [u32] {
    match band {
        "2GHz" => &[1, 6, 11],
        "5GHz" => &[36, 40, 44, 48, 149, 153, 157, 161],
        _ => &[],
    }
}

/// Plain-language advice about channel crowding.
pub fn channel_advice(aps: &[Ap]) -> Vec<String> {
    let mut out = Vec::new();
    let current = aps.iter().find(|a| a.current);
    let bands: Vec<String> = match current {
        Some(c) => vec![c.band.clone()],
        None => vec!["2GHz".into(), "5GHz".into()],
    };
    for band in bands {
        let best = candidates(&band)
            .iter()
            .map(|c| (channel_load(aps, &band, *c), *c))
            .min();
        let Some((best_load, best_ch)) = best else {
            continue;
        };
        match current {
            Some(c) => {
                let mine = channel_load(aps, &band, c.channel).saturating_sub(1);
                if mine <= best_load {
                    out.push(format!(
                        "Channel {} ({band}) shares air with {mine} other network(s); that is among the quietest options.",
                        c.channel
                    ));
                } else {
                    out.push(format!(
                        "Channel {} ({band}) shares air with {mine} other network(s); channel {best_ch} has {best_load}. Changing the channel is done on the router.",
                        c.channel
                    ));
                }
            }
            None => out.push(format!(
                "Quietest {band} channel nearby: {best_ch} ({best_load} network(s))."
            )),
        }
    }
    out
}

pub fn render_scan(aps: &[Ap], redact: bool) -> String {
    let mut rows: Vec<&Ap> = aps.iter().collect();
    rows.sort_by_key(|a| (!a.current, std::cmp::Reverse(a.signal.unwrap_or(-200))));
    let mut out = format!("WI-FI SCAN  {} network(s)\n", rows.len());
    out += &format!(
        "{:<2} {:<28} {:>3} {:<5} {:<6} {:>7}  {}\n",
        "", "SSID", "CH", "BAND", "WIDTH", "SIGNAL", "SECURITY"
    );
    for (i, a) in rows.iter().enumerate() {
        let name = if redact {
            if a.current {
                "(connected network)".to_string()
            } else {
                format!("network-{}", i + 1)
            }
        } else {
            crate::view::safe(&a.ssid)
        };
        let sig = a
            .signal
            .map(|s| format!("{s} dBm"))
            .unwrap_or_else(|| "-".into());
        out += &format!(
            "{:<2} {:<28} {:>3} {:<5} {:<6} {:>7}  {}\n",
            if a.current { "*" } else { "" },
            name,
            a.channel,
            a.band.trim_end_matches("GHz").to_string() + "G",
            a.width,
            sig,
            a.security
        );
    }
    out += "\nCHANNELS\n";
    let mut load: BTreeMap<(String, u32), usize> = BTreeMap::new();
    for a in aps {
        *load.entry((a.band.clone(), a.channel)).or_default() += 1;
    }
    for ((band, ch), n) in &load {
        out += &format!(
            "  {band:<5} ch {ch:<3} {n} network(s), {} competing\n",
            channel_load(aps, band, *ch)
        );
    }
    for line in channel_advice(aps) {
        out += &format!("\n{line}");
    }
    out + "\n"
}

pub fn scan_json(aps: &[Ap], redact: bool) -> String {
    let items: Vec<String> = aps
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let name = if redact {
                format!("network-{}", i + 1)
            } else {
                a.ssid.clone()
            };
            format!(
                "{{\"ssid\":{},\"channel\":{},\"band\":{},\"width\":{},\"security\":{},\"signal_dbm\":{},\"connected\":{}}}",
                json_string(&name),
                a.channel,
                json_string(&a.band),
                json_string(&a.width),
                json_string(&a.security),
                a.signal.map(|s| s.to_string()).unwrap_or_else(|| "null".into()),
                a.current
            )
        })
        .collect();
    format!("{{\"networks\":[{}]}}", items.join(","))
}

pub fn wifi_scan() -> Result<Vec<Ap>, String> {
    let (ok, out, err) = collect::capture("/usr/sbin/system_profiler", &["SPAirPortDataType"], 30)?;
    if !ok {
        return Err(err.trim().to_string());
    }
    let aps = parse_wifi_scan(&out);
    if aps.is_empty() {
        return Err("No networks found. Wi-Fi may be off, or macOS may be hiding scan results until Location Services allows your terminal.".into());
    }
    Ok(aps)
}

// ------------------------------------------------------------ per-app traffic

/// Parses `nettop -P -d -x -J bytes_in,bytes_out` CSV; the last sample wins.
pub fn parse_nettop(text: &str) -> Vec<(String, u64, u64)> {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines
        .iter()
        .rposition(|l| l.starts_with(",bytes_in"))
        .map(|i| i + 1)
        .unwrap_or(0);
    let mut rows = Vec::new();
    for line in &lines[start..] {
        let mut f = line.split(',');
        let (Some(name), Some(i), Some(o)) = (f.next(), f.next(), f.next()) else {
            continue;
        };
        let (Ok(i), Ok(o)) = (i.parse::<u64>(), o.parse::<u64>()) else {
            continue;
        };
        let name = name.rsplit_once('.').map(|x| x.0).unwrap_or(name);
        rows.push((name.to_string(), i, o));
    }
    rows
}

pub fn human_rate(bytes: f64) -> String {
    if bytes >= 1048576.0 {
        format!("{:.1} MiB/s", bytes / 1048576.0)
    } else if bytes >= 1024.0 {
        format!("{:.1} KiB/s", bytes / 1024.0)
    } else {
        format!("{bytes:.0} B/s")
    }
}

pub fn render_apps(rows: &[(String, u64, u64)], seconds: f64, json: bool) -> String {
    let mut rows: Vec<&(String, u64, u64)> = rows.iter().filter(|r| r.1 + r.2 > 0).collect();
    rows.sort_by_key(|r| std::cmp::Reverse(r.1 + r.2));
    rows.truncate(15);
    if json {
        let items: Vec<String> = rows
            .iter()
            .map(|r| {
                format!(
                    "{{\"process\":{},\"rx_bytes_per_s\":{:.0},\"tx_bytes_per_s\":{:.0}}}",
                    json_string(&r.0),
                    r.1 as f64 / seconds,
                    r.2 as f64 / seconds
                )
            })
            .collect();
        return format!("{{\"seconds\":{seconds},\"apps\":[{}]}}", items.join(","));
    }
    if rows.is_empty() {
        return format!("No process moved traffic in {seconds} s.");
    }
    let mut out = format!(
        "TRAFFIC BY PROCESS  (sampled {seconds} s)\n{:<28} {:>14} {:>14}\n",
        "PROCESS", "DOWN", "UP"
    );
    for r in rows {
        out += &format!(
            "{:<28} {:>14} {:>14}\n",
            crate::view::safe(&r.0),
            human_rate(r.1 as f64 / seconds),
            human_rate(r.2 as f64 / seconds)
        );
    }
    out
}

pub fn apps(seconds: u32) -> Result<Vec<(String, u64, u64)>, String> {
    let s = seconds.clamp(1, 30).to_string();
    let (ok, out, err) = collect::capture(
        "/usr/bin/nettop",
        &[
            "-P",
            "-L",
            "2",
            "-s",
            &s,
            "-d",
            "-x",
            "-J",
            "bytes_in,bytes_out",
        ],
        seconds as u64 + 15,
    )?;
    if !ok && out.is_empty() {
        return Err(err.trim().to_string());
    }
    Ok(parse_nettop(&out))
}

// ------------------------------------------------------------ LAN discovery

#[derive(Debug, Clone, PartialEq)]
pub struct Host {
    pub ip: String,
    pub mac: String,
    pub name: Option<String>,
}

fn normalise_mac(m: &str) -> Option<String> {
    let parts: Vec<&str> = m.split(':').collect();
    if parts.len() != 6 {
        return None;
    }
    let mut out = Vec::new();
    for p in parts {
        let v = u8::from_str_radix(p, 16).ok()?;
        out.push(format!("{v:02x}"));
    }
    Some(out.join(":"))
}

/// Parses `arp -an`: `? (192.168.1.1) at 80:ea:b:c5:3b:4c on en0 ifscope [ethernet]`.
pub fn parse_arp(text: &str, iface: Option<&str>) -> Vec<Host> {
    let mut hosts = Vec::new();
    for line in text.lines() {
        let (Some(open), Some(close)) = (line.find('('), line.find(')')) else {
            continue;
        };
        let ip = &line[open + 1..close];
        let rest = line[close + 1..].trim();
        let Some(rest) = rest.strip_prefix("at ") else {
            continue;
        };
        let mut f = rest.split_whitespace();
        let Some(mac) = f.next().and_then(normalise_mac) else {
            continue;
        };
        if mac == "ff:ff:ff:ff:ff:ff" || mac.starts_with("01:00:5e") {
            continue;
        }
        if let Some(want) = iface {
            if f.clone().skip_while(|w| *w != "on").nth(1) != Some(want) {
                continue;
            }
        }
        hosts.push(Host {
            ip: ip.into(),
            mac,
            name: None,
        });
    }
    hosts
}

const VENDORS: &[(&str, &str)] = &[
    ("b8:27:eb", "Raspberry Pi"),
    ("dc:a6:32", "Raspberry Pi"),
    ("e4:5f:01", "Raspberry Pi"),
    ("00:50:56", "VMware"),
    ("00:0c:29", "VMware"),
    ("08:00:27", "VirtualBox"),
    ("52:54:00", "QEMU/KVM"),
    ("3c:5a:b4", "Google"),
    ("f4:f5:d8", "Google"),
    ("18:b4:30", "Nest"),
    ("00:17:88", "Philips Hue"),
    ("50:c7:bf", "TP-Link"),
    ("3c:07:54", "Apple"),
    ("f0:18:98", "Apple"),
    ("a4:83:e7", "Apple"),
];

pub fn vendor(mac: &str) -> &'static str {
    if let Some(first) = mac.get(0..2).and_then(|h| u8::from_str_radix(h, 16).ok()) {
        if first & 0x02 != 0 {
            return "private (randomized)";
        }
    }
    VENDORS
        .iter()
        .find(|(p, _)| mac.starts_with(p))
        .map(|(_, v)| *v)
        .unwrap_or("")
}

fn ipv4_parts(ip: &str) -> Option<u32> {
    let o: Vec<u8> = ip.split('.').filter_map(|p| p.parse().ok()).collect();
    (o.len() == 4).then(|| u32::from_be_bytes([o[0], o[1], o[2], o[3]]))
}

fn ipv4_string(n: u32) -> String {
    let b = n.to_be_bytes();
    format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3])
}

/// Host addresses of `ip/prefix` without network and broadcast; refuses ranges over 1022 hosts.
pub fn sweep_range(ip: &str, prefix: u8) -> Result<Vec<String>, String> {
    let base = ipv4_parts(ip).ok_or("Not an IPv4 address")?;
    if !(22..=30).contains(&prefix) {
        return Err(format!(
            "A /{prefix} network is too large or too small to sweep; /22 to /30 only"
        ));
    }
    let mask = u32::MAX << (32 - prefix);
    let net = base & mask;
    let last = net | !mask;
    Ok((net + 1..last).map(ipv4_string).collect())
}

pub fn render_lan(hosts: &[Host], iface: &str, redact: bool, json: bool) -> String {
    let mut hosts = hosts.to_vec();
    hosts.sort_by_key(|h| ipv4_parts(&h.ip));
    let mask = |m: &str| {
        if m.len() < 17 {
            "-".to_string()
        } else if redact {
            format!("xx:xx:xx:{}", &m[9..])
        } else {
            m.to_string()
        }
    };
    if json {
        let items: Vec<String> = hosts
            .iter()
            .map(|h| {
                format!(
                    "{{\"ip\":{},\"mac\":{},\"name\":{},\"vendor\":{}}}",
                    json_string(&h.ip),
                    json_string(&mask(&h.mac)),
                    h.name
                        .as_ref()
                        .map(|n| json_string(n))
                        .unwrap_or_else(|| "null".into()),
                    json_string(vendor(&h.mac))
                )
            })
            .collect();
        return format!(
            "{{\"interface\":{},\"hosts\":[{}]}}",
            json_string(iface),
            items.join(",")
        );
    }
    let mut out = format!(
        "LAN  {} device(s) answered on {iface}\n{:<16} {:<18} {:<28} {}\n",
        hosts.len(),
        "ADDRESS",
        "MAC",
        "NAME",
        "VENDOR"
    );
    for h in &hosts {
        out += &format!(
            "{:<16} {:<18} {:<28} {}\n",
            h.ip,
            mask(&h.mac),
            h.name.as_deref().map(crate::view::safe).unwrap_or_default(),
            vendor(&h.mac)
        );
    }
    if hosts.iter().any(|h| h.mac.is_empty()) {
        out += "MAC addresses come from the ARP table, which macOS did not show to this process; devices were found by ping instead.\n";
    }
    out + "Vendors are recognised for a few common makers only; private addresses are randomized by phones and laptops on purpose.\n"
}

fn reverse_name(ip: &str) -> Option<String> {
    let (ok, out, _) = collect::capture(
        "/usr/bin/dscacheutil",
        &["-q", "host", "-a", "ip_address", ip],
        3,
    )
    .ok()?;
    if !ok {
        return None;
    }
    out.lines()
        .find_map(|l| l.strip_prefix("name: "))
        .map(|n| n.trim().to_string())
}

/// Pings every address in the range (bounded concurrency) to fill the ARP table, then reads it.
/// Hosts that answered the ping are listed even when the ARP table cannot be read.
pub fn lan_sweep(ip: &str, prefix: u8, iface: &str) -> Result<Vec<Host>, String> {
    let targets = sweep_range(ip, prefix)?;
    let mut answered: Vec<String> = Vec::new();
    for chunk in targets.chunks(64) {
        let handles: Vec<_> = chunk
            .iter()
            .cloned()
            .map(|t| {
                std::thread::spawn(move || {
                    let ok = matches!(
                        collect::capture("/sbin/ping", &["-c", "1", "-W", "400", &t], 3),
                        Ok((true, _, _))
                    );
                    (t, ok)
                })
            })
            .collect();
        for h in handles {
            if let Ok((t, true)) = h.join() {
                answered.push(t);
            }
        }
    }
    let arp = collect::command("/usr/sbin/arp", &["-an"]).unwrap_or_default();
    let range: std::collections::HashSet<String> = targets.into_iter().collect();
    let mut hosts: Vec<Host> = parse_arp(&arp, Some(iface))
        .into_iter()
        .filter(|h| range.contains(&h.ip) || h.ip == ip)
        .collect();
    for t in answered {
        if !hosts.iter().any(|h| h.ip == t) {
            hosts.push(Host {
                ip: t,
                mac: String::new(),
                name: None,
            });
        }
    }
    let handles: Vec<_> = hosts
        .iter()
        .map(|h| {
            let ip = h.ip.clone();
            std::thread::spawn(move || reverse_name(&ip))
        })
        .collect();
    for (h, join) in hosts.iter_mut().zip(handles) {
        h.name = join.join().ok().flatten();
    }
    Ok(hosts)
}

// -------------------------------------------------------- hop statistics (mtr)

#[derive(Debug, Default, Clone)]
pub struct Hop {
    pub ip: String,
    pub sent: u32,
    pub rtts: Vec<f64>,
}

/// Folds one `traceroute -n -q 1` run into per-hop statistics.
pub fn fold_trace(text: &str, hops: &mut BTreeMap<u32, Hop>) {
    for line in text.lines() {
        let mut t = line.split_whitespace();
        let Some(n) = t.next().and_then(|n| n.parse::<u32>().ok()) else {
            continue;
        };
        let hop = hops.entry(n).or_default();
        hop.sent += 1;
        match (t.next(), t.next().and_then(|r| r.parse::<f64>().ok())) {
            (Some("*"), _) | (None, _) => {}
            (Some(ip), Some(rtt)) => {
                if hop.ip.is_empty() {
                    hop.ip = ip.into();
                }
                hop.rtts.push(rtt);
            }
            _ => {}
        }
    }
}

pub fn render_hops(host: &str, rounds: u32, hops: &BTreeMap<u32, Hop>, json: bool) -> String {
    let stat = |h: &Hop| {
        let n = h.rtts.len() as f64;
        let avg = if n > 0.0 {
            h.rtts.iter().sum::<f64>() / n
        } else {
            0.0
        };
        let best = h.rtts.iter().cloned().fold(f64::INFINITY, f64::min);
        let worst = h.rtts.iter().cloned().fold(0.0, f64::max);
        let jitter = if n > 1.0 {
            (h.rtts.iter().map(|r| (r - avg).powi(2)).sum::<f64>() / n).sqrt()
        } else {
            0.0
        };
        let loss = 100.0 * (h.sent as f64 - n) / h.sent.max(1) as f64;
        (loss, avg, if n > 0.0 { best } else { 0.0 }, worst, jitter)
    };
    if json {
        let items: Vec<String> = hops
            .iter()
            .map(|(n, h)| {
                let (loss, avg, best, worst, jitter) = stat(h);
                format!(
                    "{{\"hop\":{n},\"ip\":{},\"loss_pct\":{loss:.1},\"avg_ms\":{avg:.2},\"best_ms\":{best:.2},\"worst_ms\":{worst:.2},\"jitter_ms\":{jitter:.2}}}",
                    json_string(&h.ip)
                )
            })
            .collect();
        return format!(
            "{{\"host\":{},\"rounds\":{rounds},\"hops\":[{}]}}",
            json_string(host),
            items.join(",")
        );
    }
    let mut out = format!(
        "PATH TO {}  ({rounds} rounds)\n{:>3}  {:<40} {:>6} {:>8} {:>8} {:>8} {:>7}\n",
        crate::view::safe(host),
        "HOP",
        "ADDRESS",
        "LOSS",
        "AVG",
        "BEST",
        "WORST",
        "JITTER"
    );
    for (n, h) in hops {
        let (loss, avg, best, worst, jitter) = stat(h);
        let addr = if h.ip.is_empty() { "*" } else { &h.ip };
        out += &format!(
            "{n:>3}  {addr:<40} {loss:>5.0}% {avg:>6.1}ms {best:>6.1}ms {worst:>6.1}ms {jitter:>5.1}ms\n"
        );
    }
    out + "Loss at a middle hop that does not continue to the end is usually that router rate-limiting replies, not a real problem.\n"
}

pub fn hop_stats(host: &str, rounds: u32) -> Result<BTreeMap<u32, Hop>, String> {
    if !probe::valid_host(host) {
        return Err("Use a hostname or IP address, not a URL or shell expression".into());
    }
    let rounds = rounds.clamp(1, 30);
    let handles: Vec<_> = (0..rounds)
        .map(|_| {
            let host = host.to_string();
            std::thread::spawn(move || {
                collect::capture(
                    "/usr/sbin/traceroute",
                    &["-n", "-q", "1", "-w", "1", "-m", "20", &host],
                    40,
                )
                .map(|r| r.1)
            })
        })
        .collect();
    let mut hops = BTreeMap::new();
    let mut any = false;
    for h in handles {
        if let Ok(Ok(text)) = h.join() {
            any = true;
            fold_trace(&text, &mut hops);
        }
    }
    if !any || hops.is_empty() {
        return Err("traceroute produced no output".into());
    }
    Ok(hops)
}

// ----------------------------------------------------------------- DNS lookup

pub const RECORD_TYPES: &[&str] = &[
    "A", "AAAA", "MX", "TXT", "NS", "CNAME", "SOA", "CAA", "PTR", "SRV",
];

#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    pub server: String,
    pub records: Vec<String>,
    pub ms: Option<u32>,
    pub error: Option<String>,
}

/// Parses `dig +noall +answer +stats` output.
pub fn parse_dig(text: &str) -> (Vec<String>, Option<u32>) {
    let mut records = Vec::new();
    let mut ms = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix(";; Query time: ") {
            ms = rest.split_whitespace().next().and_then(|n| n.parse().ok());
        } else if !line.starts_with(';') && !line.trim().is_empty() {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() >= 5 {
                records.push(f[4..].join(" "));
            }
        }
    }
    records.sort();
    (records, ms)
}

pub fn lookup(host: &str, rtype: &str) -> Result<Vec<Answer>, String> {
    if !probe::valid_host(host) || host.contains('%') {
        return Err("Use a hostname, not a URL or shell expression".into());
    }
    let rtype = rtype.to_ascii_uppercase();
    if !RECORD_TYPES.contains(&rtype.as_str()) {
        return Err(format!(
            "Record type must be one of {}",
            RECORD_TYPES.join(", ")
        ));
    }
    let servers: [(&str, Option<&str>); 4] = [
        ("system resolver", None),
        ("1.1.1.1", Some("@1.1.1.1")),
        ("8.8.8.8", Some("@8.8.8.8")),
        ("9.9.9.9", Some("@9.9.9.9")),
    ];
    let handles: Vec<_> = servers
        .iter()
        .map(|(label, at)| {
            let (label, at) = (label.to_string(), at.map(|s| s.to_string()));
            let (host, rtype) = (host.to_string(), rtype.clone());
            std::thread::spawn(move || {
                let mut args: Vec<&str> =
                    vec!["+time=2", "+tries=1", "+noall", "+answer", "+stats"];
                if let Some(a) = &at {
                    args.push(a);
                }
                args.extend([host.as_str(), rtype.as_str()]);
                match collect::capture("/usr/bin/dig", &args, 8) {
                    Ok((_, out, _)) if out.contains("Query time") => {
                        let (records, ms) = parse_dig(&out);
                        Answer {
                            server: label,
                            records,
                            ms,
                            error: None,
                        }
                    }
                    Ok((_, _, err)) => Answer {
                        server: label,
                        records: vec![],
                        ms: None,
                        error: Some(if err.trim().is_empty() {
                            "no response".into()
                        } else {
                            err.trim().into()
                        }),
                    },
                    Err(e) => Answer {
                        server: label,
                        records: vec![],
                        ms: None,
                        error: Some(e),
                    },
                }
            })
        })
        .collect();
    Ok(handles.into_iter().filter_map(|h| h.join().ok()).collect())
}

pub fn render_lookup(host: &str, rtype: &str, answers: &[Answer], json: bool) -> String {
    if json {
        let items: Vec<String> = answers
            .iter()
            .map(|a| {
                let recs: Vec<String> = a.records.iter().map(|r| json_string(r)).collect();
                format!(
                    "{{\"server\":{},\"records\":[{}],\"ms\":{},\"error\":{}}}",
                    json_string(&a.server),
                    recs.join(","),
                    a.ms.map(|m| m.to_string()).unwrap_or_else(|| "null".into()),
                    a.error
                        .as_ref()
                        .map(|e| json_string(e))
                        .unwrap_or_else(|| "null".into())
                )
            })
            .collect();
        return format!(
            "{{\"host\":{},\"type\":{},\"answers\":[{}]}}",
            json_string(host),
            json_string(rtype),
            items.join(",")
        );
    }
    let mut out = format!(
        "LOOKUP {} {}\n",
        crate::view::safe(host),
        rtype.to_ascii_uppercase()
    );
    for a in answers {
        let time =
            a.ms.map(|m| format!("{m} ms"))
                .unwrap_or_else(|| "-".into());
        match &a.error {
            Some(e) => {
                out += &format!(
                    "  {:<16} {:>7}  failed: {}\n",
                    a.server,
                    time,
                    crate::view::safe(e)
                )
            }
            None if a.records.is_empty() => {
                out += &format!("  {:<16} {:>7}  no records\n", a.server, time)
            }
            None => {
                out += &format!(
                    "  {:<16} {:>7}  {}\n",
                    a.server,
                    time,
                    crate::view::safe(&a.records.join(", "))
                )
            }
        }
    }
    let answered: Vec<&Answer> = answers
        .iter()
        .filter(|a| a.error.is_none() && !a.records.is_empty())
        .collect();
    if answered.windows(2).any(|w| w[0].records != w[1].records) {
        out += "Resolvers returned different answers. Large sites often do this on purpose (nearest server); for other names it can mean a stale cache or a filtering resolver.\n";
    } else if answered.len() == answers.len() {
        out += "All resolvers agree.\n";
    }
    out
}

// ------------------------------------------------------------------- firewall

/// Parses `socketfilterfw --get...` lines into `(label, enabled)`.
pub fn parse_firewall(text: &str) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let l = line.trim();
        let lower = l.to_ascii_lowercase();
        if lower.starts_with("usage") || l.is_empty() {
            continue;
        }
        let on = lower.contains("enabled")
            || lower.ends_with(" on")
            || lower.contains("state = 1")
            || lower.contains("state = 2");
        let label = if lower.contains("stealth") {
            "Stealth mode (ignore unsolicited probes)"
        } else if lower.contains("block all") {
            "Block all incoming connections"
        } else if lower.starts_with("firewall is") {
            "Application firewall"
        } else {
            continue;
        };
        out.push((label.to_string(), on));
    }
    out
}

pub fn firewall_status() -> Result<String, String> {
    let (_, out, _) = collect::capture(
        "/usr/libexec/ApplicationFirewall/socketfilterfw",
        &["--getglobalstate", "--getstealthmode", "--getblockall"],
        5,
    )?;
    let items = parse_firewall(&out);
    if items.is_empty() {
        return Err("Could not read the application firewall state".into());
    }
    let mut text = String::from("FIREWALL\n");
    for (label, on) in &items {
        text += &format!("  {:<42} {}\n", label, if *on { "on" } else { "off" });
    }
    let pf = collect::capture("/sbin/pfctl", &["-s", "info"], 5);
    match pf {
        Ok((true, out, _)) => {
            let status = out
                .lines()
                .find(|l| l.starts_with("Status:"))
                .unwrap_or("Status: unknown");
            text += &format!(
                "  {:<42} {}\n",
                "Packet filter (pf)",
                status
                    .trim_start_matches("Status: ")
                    .split_whitespace()
                    .next()
                    .unwrap_or("unknown")
                    .to_lowercase()
            );
        }
        _ => {
            text += &format!(
                "  {:<42} needs administrator rights to read (sudo macora firewall)\n",
                "Packet filter (pf)"
            )
        }
    }
    if items.first().map(|i| !i.1).unwrap_or(false) {
        text += "The application firewall is off. It only filters incoming connections; it does not block outgoing traffic (that needs an app such as LuLu or Little Snitch).\n";
    }
    Ok(text)
}

// ----------------------------------------------------------------- throughput

/// Reads sent/received megabits per second out of `iperf3 -J`.
pub fn parse_iperf(text: &str) -> Result<(f64, f64), String> {
    let v = json::parse(text)?;
    if let Some(e) = v.get("error").and_then(|e| e.as_str()) {
        return Err(e.to_string());
    }
    let end = v.get("end").ok_or("iperf3 gave no result")?;
    let rate = |k: &str| {
        end.get(k)
            .and_then(|s| s.get("bits_per_second"))
            .and_then(|b| b.as_f64())
            .map(|b| b / 1e6)
    };
    match (rate("sum_sent"), rate("sum_received")) {
        (Some(s), Some(r)) => Ok((s, r)),
        _ => Err("iperf3 result is missing throughput".into()),
    }
}

pub fn throughput(host: &str) -> Result<String, String> {
    if !probe::valid_host(host) || host.contains('%') {
        return Err("Use a hostname or IPv4 address".into());
    }
    let bin = ["/opt/homebrew/bin/iperf3", "/usr/local/bin/iperf3"]
        .into_iter()
        .find(|p| std::path::Path::new(p).exists())
        .ok_or("iperf3 not found. Install it with `brew install iperf3`, run `iperf3 -s` on the other machine, then try again.")?;
    let (_, out, err) = collect::capture(bin, &["-c", host, "-t", "5", "-J"], 30)?;
    if out.trim().is_empty() {
        return Err(err.trim().to_string());
    }
    let (sent, received) = parse_iperf(&out)?;
    Ok(format!(
        "THROUGHPUT to {}\n  upload   {sent:.0} Mbit/s\n  download {received:.0} Mbit/s  (measured by the receiving side)\n",
        crate::view::safe(host)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCAN: &str = "Wi-Fi:\n      Interfaces:\n        en0:\n          Current Network Information:\n            Home:\n              PHY Mode: 802.11ax\n              Channel: 36 (5GHz, 80MHz)\n              Security: WPA2 Personal\n              Signal / Noise: -67 dBm / -94 dBm\n          Other Local Wi-Fi Networks:\n            Home:\n              Channel: 36 (5GHz, 80MHz)\n              Signal / Noise: -56 dBm / -94 dBm\n            Cafe:\n              PHY Mode: 802.11b/g/n\n              Channel: 6 (2GHz, 40MHz)\n              Security: WPA/WPA2 Enterprise\n            Next:\n              Channel: 7 (2GHz, 20MHz)\n              Signal / Noise: -80 dBm / -94 dBm\n        awdl0:\n          Current Network Information:\n              Network Type: Infrastructure\n";

    #[test]
    fn scan_parses_current_and_others_without_duplicates() {
        let aps = parse_wifi_scan(SCAN);
        assert_eq!(aps.len(), 3);
        assert!(aps[0].current && aps[0].ssid == "Home" && aps[0].channel == 36);
        assert_eq!(aps[0].band, "5GHz");
        assert_eq!(aps[0].width, "80MHz");
        assert_eq!(aps[0].signal, Some(-67));
        let cafe = aps.iter().find(|a| a.ssid == "Cafe").unwrap();
        assert_eq!(cafe.signal, None);
        assert_eq!(cafe.security, "WPA/WPA2 Enterprise");
    }

    #[test]
    fn two_point_four_channels_overlap() {
        let aps = parse_wifi_scan(SCAN);
        assert_eq!(channel_load(&aps, "2GHz", 6), 2); // ch 6 and ch 7
        assert_eq!(channel_load(&aps, "2GHz", 1), 0);
        assert_eq!(channel_load(&aps, "5GHz", 36), 1);
        let advice = channel_advice(&aps).join("\n");
        assert!(advice.contains("Channel 36 (5GHz)"), "{advice}");
    }

    #[test]
    fn scan_render_can_hide_network_names() {
        let aps = parse_wifi_scan(SCAN);
        let text = render_scan(&aps, true);
        assert!(!text.contains("Cafe") && !text.contains("Next") && !text.contains("Home"));
        assert!(scan_json(&aps, true).contains("network-"));
        assert!(render_scan(&aps, false).contains("Cafe"));
    }

    #[test]
    fn nettop_takes_the_last_sample() {
        let text = ",bytes_in,bytes_out,\na.1,5,5,\n,bytes_in,bytes_out,\nsafari.1020,100,20,\nmDNS.responder.688,0,0,\n";
        let rows = parse_nettop(text);
        assert_eq!(rows[0], ("safari".to_string(), 100, 20));
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].0, "mDNS.responder");
        let one = ",bytes_in,bytes_out,\nx.1,7,8,\n";
        assert_eq!(parse_nettop(one).len(), 1);
        assert!(render_apps(&rows, 2.0, false).contains("safari"));
        assert!(render_apps(&rows, 2.0, true).contains("\"process\":\"safari\""));
    }

    #[test]
    fn arp_lines_become_hosts() {
        let text = "? (192.168.1.1) at 80:ea:b:c5:3b:4c on en0 ifscope [ethernet]\n? (192.168.1.9) at (incomplete) on en0 ifscope [ethernet]\n? (224.0.0.251) at 1:0:5e:0:0:fb on en0 ifscope permanent [ethernet]\n? (10.0.0.2) at b8:27:eb:1:2:3 on en5 ifscope [ethernet]\n";
        let hosts = parse_arp(text, Some("en0"));
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].mac, "80:ea:0b:c5:3b:4c");
        assert_eq!(parse_arp(text, None).len(), 2);
        assert_eq!(vendor("b8:27:eb:01:02:03"), "Raspberry Pi");
        assert_eq!(vendor("7e:1a:38:dd:8c:e6"), "private (randomized)");
        assert_eq!(vendor("80:ea:0b:c5:3b:4c"), "");
    }

    #[test]
    fn sweep_ranges_are_bounded() {
        let r = sweep_range("192.168.1.8", 24).unwrap();
        assert_eq!(
            (r.len(), r[0].as_str(), r[253].as_str()),
            (254, "192.168.1.1", "192.168.1.254")
        );
        assert!(sweep_range("10.0.0.5", 16).is_err());
        assert!(sweep_range("10.0.0.5", 31).is_err());
        assert_eq!(sweep_range("10.0.0.5", 22).unwrap().len(), 1022);
        assert!(sweep_range("nope", 24).is_err());
    }

    #[test]
    fn traceroute_rounds_fold_into_hop_stats() {
        let mut hops = BTreeMap::new();
        fold_trace(
            "traceroute to x\n 1  10.0.0.1  2.0 ms\n 2  *\n 3  8.8.8.8  10.0 ms\n",
            &mut hops,
        );
        fold_trace(
            " 1  10.0.0.1  4.0 ms\n 2  *\n 3  8.8.8.8  14.0 ms\n",
            &mut hops,
        );
        assert_eq!(hops[&1].rtts, vec![2.0, 4.0]);
        assert_eq!(hops[&2].sent, 2);
        assert!(hops[&2].rtts.is_empty());
        let text = render_hops("x", 2, &hops, false);
        assert!(text.contains("100%") && text.contains("10.0.0.1"));
        assert!(render_hops("x", 2, &hops, true).contains("\"hop\":3"));
    }

    #[test]
    fn dig_output_and_comparison() {
        let out = "example.com.\t73\tIN\tA\t104.20.23.154\nexample.com.\t73\tIN\tA\t172.66.147.243\n;; Query time: 9 msec\n;; SERVER: 1.1.1.1#53(1.1.1.1)\n";
        let (records, ms) = parse_dig(out);
        assert_eq!(records, vec!["104.20.23.154", "172.66.147.243"]);
        assert_eq!(ms, Some(9));
        let a = Answer {
            server: "1.1.1.1".into(),
            records: records.clone(),
            ms,
            error: None,
        };
        let b = Answer {
            server: "8.8.8.8".into(),
            records: vec!["1.2.3.4".into()],
            ms: Some(12),
            error: None,
        };
        let text = render_lookup("example.com", "a", &[a.clone(), b], false);
        assert!(text.contains("different answers"));
        assert!(render_lookup("example.com", "a", &[a.clone(), a], false).contains("agree"));
        assert!(lookup("http://x", "A").is_err());
        assert!(lookup("example.com", "ANY").is_err());
    }

    #[test]
    fn firewall_and_iperf_parsers() {
        let fw = parse_firewall("Firewall is disabled. (State = 0)\nFirewall stealth mode is off\nFirewall has block all state set to disabled.\n");
        assert_eq!(fw.len(), 3);
        assert!(fw.iter().all(|(_, on)| !on));
        let fw = parse_firewall("Firewall is enabled. (State = 1)\nFirewall stealth mode is on\n");
        assert!(fw.iter().all(|(_, on)| *on));
        let ok = r#"{"end":{"sum_sent":{"bits_per_second":9.4e8},"sum_received":{"bits_per_second":9.3e8}}}"#;
        let (s, r) = parse_iperf(ok).unwrap();
        assert!((s - 940.0).abs() < 0.1 && (r - 930.0).abs() < 0.1);
        assert!(parse_iperf(r#"{"error":"unable to connect"}"#).is_err());
        assert!(throughput("http://x").is_err());
    }
}
