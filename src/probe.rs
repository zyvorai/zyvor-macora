//! Explicit active checks: ping, traceroute and the `diagnose` walkthrough.
//! Every function here only runs when the user asks for it; nothing is sent in the background.
use crate::{
    collect,
    model::{json_string, Snapshot},
};
use std::{
    net::{SocketAddr, TcpStream},
    time::{Duration, Instant},
};

#[derive(Debug, Clone, PartialEq)]
pub struct PingStats {
    pub sent: u32,
    pub received: u32,
    pub loss_pct: f64,
    pub min_ms: Option<f64>,
    pub avg_ms: Option<f64>,
    pub max_ms: Option<f64>,
}

/// Accepts a hostname or numeric address; rejects anything a shell or option parser could misread.
pub fn valid_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && !host.starts_with('-')
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ".-_:".contains(c))
}

/// Parses BSD/macOS and Linux `ping` summaries.
pub fn parse_ping(text: &str) -> Result<PingStats, String> {
    let mut sent = None;
    let mut received = None;
    let mut loss = None;
    let mut rtt = (None, None, None);
    for line in text.lines() {
        if line.contains("packets transmitted") {
            for part in line.split(',') {
                let part = part.trim();
                let n = part.split_whitespace().next().unwrap_or("");
                if part.contains("packets transmitted") {
                    sent = n.parse().ok();
                } else if part.contains("received") {
                    received = n.parse().ok();
                } else if part.contains("packet loss") {
                    loss = n.trim_end_matches('%').parse().ok();
                }
            }
        } else if let Some((_, values)) = line.split_once(" = ") {
            if line.contains("min/avg/max") {
                let nums: Vec<f64> = values
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .split('/')
                    .filter_map(|v| v.parse().ok())
                    .collect();
                if nums.len() >= 3 {
                    rtt = (Some(nums[0]), Some(nums[1]), Some(nums[2]));
                }
            }
        }
    }
    let (sent, received) = (
        sent.ok_or("No ping summary in output")?,
        received.ok_or("No ping summary in output")?,
    );
    Ok(PingStats {
        sent,
        received,
        loss_pct: loss.unwrap_or(if sent == 0 {
            100.0
        } else {
            100.0 * (sent - received.min(sent)) as f64 / sent as f64
        }),
        min_ms: rtt.0,
        avg_ms: rtt.1,
        max_ms: rtt.2,
    })
}

pub fn ping_stats(host: &str, count: u32, packet_size: Option<u32>) -> Result<PingStats, String> {
    if !valid_host(host) {
        return Err("Use a hostname or IP address, not a URL or shell expression".into());
    }
    if !cfg!(target_os = "macos") {
        return Err("ping requires macOS".into());
    }
    let count = count.to_string();
    let size = packet_size.map(|s| s.to_string());
    let mut args = vec!["-c", &count, "-n", "-q", "-t", "10"];
    if let Some(size) = &size {
        // Do-not-fragment probe used to detect path MTU problems.
        args.extend(["-D", "-s", size]);
    }
    args.push(host);
    let (_, out, err) = collect::capture("/sbin/ping", &args, 15)?;
    parse_ping(&out).map_err(|e| {
        let detail = err.trim();
        if detail.is_empty() {
            e
        } else {
            detail.to_string()
        }
    })
}

pub fn ping_json(host: &str, s: &PingStats) -> String {
    let n = |v: Option<f64>| v.map_or("null".to_string(), |v| format!("{v:.3}"));
    format!(
        "{{\"host\":{},\"sent\":{},\"received\":{},\"loss_percent\":{:.1},\"min_ms\":{},\"avg_ms\":{},\"max_ms\":{}}}",
        json_string(host), s.sent, s.received, s.loss_pct, n(s.min_ms), n(s.avg_ms), n(s.max_ms)
    )
}

pub fn ping_text(host: &str, s: &PingStats) -> String {
    let rtt = match (s.min_ms, s.avg_ms, s.max_ms) {
        (Some(a), Some(b), Some(c)) => format!("rtt min/avg/max {a:.1}/{b:.1}/{c:.1} ms"),
        _ => "no replies".into(),
    };
    format!(
        "{host}: {}/{} replies, {:.0}% loss · {rtt}",
        s.received, s.sent, s.loss_pct
    )
}

pub fn trace(host: &str) -> Result<String, String> {
    if !valid_host(host) {
        return Err("Use a hostname or IP address, not a URL or shell expression".into());
    }
    if !cfg!(target_os = "macos") {
        return Err("traceroute requires macOS".into());
    }
    let (_, out, err) = collect::capture(
        "/usr/sbin/traceroute",
        &["-n", "-q", "1", "-w", "1", "-m", "20", host],
        40,
    )?;
    if out.trim().is_empty() {
        return Err(err.trim().to_string());
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Status {
    Ok,
    Warn,
    Fail,
    Skip,
}
impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Status::Ok => "OK",
            Status::Warn => "WARN",
            Status::Fail => "FAIL",
            Status::Skip => "SKIP",
        }
    }
}
#[derive(Debug, Clone)]
pub struct Step {
    pub name: &'static str,
    pub status: Status,
    pub detail: String,
}

/// Plain-language conclusion from the first failing layer, working from the link upward.
pub fn verdict(steps: &[Step]) -> String {
    let bad = |n: &str, s: Status| steps.iter().any(|x| x.name == n && x.status == s);
    if bad("Link", Status::Fail) {
        "No usable local connection. Check Wi-Fi or cable, then that DHCP gave this Mac an address."
            .into()
    } else if bad("Gateway", Status::Fail) {
        "This Mac cannot reach its router. The problem is local: Wi-Fi signal, cable, or the router itself.".into()
    } else if bad("Internet", Status::Fail) {
        "The router answers but the internet does not. Restart the router/modem or contact your ISP.".into()
    } else if bad("DNS", Status::Fail) {
        "Internet reachable by IP but names do not resolve. Try another DNS server or check VPN/DNS settings.".into()
    } else if bad("Gateway", Status::Warn) || bad("Internet", Status::Warn) {
        "Connected, but with packet loss or high latency. Move closer to the router or check for interference/congestion.".into()
    } else if bad("DNS", Status::Warn) {
        "Connected, but DNS is slow. A different resolver may speed up page loads.".into()
    } else if bad("MTU", Status::Warn) {
        "Connected, but full-size packets are dropped. A VPN or tunnel may need a lower MTU.".into()
    } else {
        "No problem found on this path right now. If an app is slow, the issue is probably the remote service.".into()
    }
}

fn grade_latency(avg: Option<f64>, loss: f64, warn_ms: f64) -> (Status, String) {
    let detail = format!(
        "{:.0}% loss{}",
        loss,
        avg.map_or(String::new(), |a| format!(", {a:.1} ms average"))
    );
    if loss >= 100.0 {
        (Status::Fail, detail)
    } else if loss > 0.0 || avg.is_some_and(|a| a > warn_ms) {
        (Status::Warn, detail)
    } else {
        (Status::Ok, detail)
    }
}

pub fn default_gateway() -> Option<String> {
    let out = collect::command("/sbin/route", &["-n", "get", "default"]).ok()?;
    out.lines().find_map(|l| {
        l.trim()
            .strip_prefix("gateway:")
            .map(|g| g.trim().to_string())
    })
}

pub fn diagnose(s: &Snapshot) -> Vec<Step> {
    let mut steps = Vec::new();
    let has_link = s.interfaces.iter().any(|i| {
        i.group() == "Network"
            && i.active()
            && i.addresses
                .iter()
                .any(|a| !a.ip.contains(':') && !a.ip.starts_with("169.254."))
    });
    steps.push(Step {
        name: "Link",
        status: if has_link { Status::Ok } else { Status::Fail },
        detail: if has_link {
            "An active adapter has a routable IPv4 address".into()
        } else {
            "No active adapter with a usable IPv4 address".into()
        },
    });
    let skip = |name| Step {
        name,
        status: Status::Skip,
        detail: "Skipped: earlier step failed".into(),
    };
    let gateway = if has_link { default_gateway() } else { None };
    match &gateway {
        Some(g) => {
            let (status, detail) = match ping_stats(g, 3, None) {
                Ok(p) => grade_latency(p.avg_ms, p.loss_pct, 50.0),
                Err(e) => (Status::Fail, e),
            };
            steps.push(Step {
                name: "Gateway",
                status,
                detail: format!("{g}: {detail}"),
            });
        }
        None => steps.push(if has_link {
            Step {
                name: "Gateway",
                status: Status::Fail,
                detail: "No default gateway found".into(),
            }
        } else {
            skip("Gateway")
        }),
    }
    let gateway_ok = steps
        .last()
        .is_some_and(|s| matches!(s.status, Status::Ok | Status::Warn));
    if gateway_ok {
        let started = Instant::now();
        let target: SocketAddr = "1.1.1.1:443".parse().expect("static address");
        let reached = TcpStream::connect_timeout(&target, Duration::from_secs(3));
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        steps.push(match reached {
            Ok(_) if ms > 400.0 => Step {
                name: "Internet",
                status: Status::Warn,
                detail: format!("1.1.1.1:443 connected in {ms:.0} ms (slow)"),
            },
            Ok(_) => Step {
                name: "Internet",
                status: Status::Ok,
                detail: format!("1.1.1.1:443 connected in {ms:.0} ms"),
            },
            Err(e) => Step {
                name: "Internet",
                status: Status::Fail,
                detail: format!("1.1.1.1:443: {e}"),
            },
        });
    } else {
        steps.push(skip("Internet"));
    }
    if steps.last().is_some_and(|s| s.status != Status::Skip) {
        let started = Instant::now();
        let looked_up = collect::command(
            "/usr/bin/dscacheutil",
            &["-q", "host", "-a", "name", "example.com"],
        );
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        steps.push(match looked_up {
            Ok(o) if !o.trim().is_empty() => Step {
                name: "DNS",
                status: if ms > 300.0 { Status::Warn } else { Status::Ok },
                detail: format!("example.com resolved in {ms:.0} ms (may be cached)"),
            },
            _ => Step {
                name: "DNS",
                status: Status::Fail,
                detail: "example.com did not resolve".into(),
            },
        });
    } else {
        steps.push(skip("DNS"));
    }
    match (&gateway, gateway_ok) {
        (Some(g), true) => {
            // 1472 bytes of payload + 28 bytes of headers = a full 1500-byte frame.
            let status = match ping_stats(g, 2, Some(1472)) {
                Ok(p) if p.received > 0 => (
                    Status::Ok,
                    "1500-byte packets reach the gateway".to_string(),
                ),
                _ => (
                    Status::Warn,
                    "1500-byte packets did not reach the gateway".to_string(),
                ),
            };
            steps.push(Step {
                name: "MTU",
                status: status.0,
                detail: status.1,
            });
        }
        _ => steps.push(skip("MTU")),
    }
    steps
}

pub fn steps_text(steps: &[Step]) -> String {
    let mut out: String = steps
        .iter()
        .map(|s| format!("{:<5} {:<9} {}\n", s.status.label(), s.name, s.detail))
        .collect();
    out += &format!("\nVERDICT  {}", verdict(steps));
    out
}

pub fn steps_json(steps: &[Step]) -> String {
    let items: Vec<String> = steps
        .iter()
        .map(|s| {
            format!(
                "{{\"step\":{},\"status\":{},\"detail\":{}}}",
                json_string(s.name),
                json_string(s.status.label()),
                json_string(&s.detail)
            )
        })
        .collect();
    format!(
        "{{\"steps\":[{}],\"verdict\":{}}}",
        items.join(","),
        json_string(&verdict(steps))
    )
}
