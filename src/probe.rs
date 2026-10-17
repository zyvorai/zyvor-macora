//! Explicit active checks: ping and traceroute (the `diagnose` walkthrough lives in `crate::diagnose`).
//! Every function here only runs when the user asks for it; nothing is sent in the background.
use crate::{collect, model::json_string};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PingStats {
    pub sent: u32,
    pub received: u32,
    pub loss_pct: f64,
    pub min_ms: Option<f64>,
    pub avg_ms: Option<f64>,
    pub max_ms: Option<f64>,
    /// Standard deviation of the round-trip times, i.e. jitter. macOS prints `stddev` for IPv4 and
    /// `std-dev` for IPv6, Linux prints `mdev`; a single packet gives `nan`, which is dropped.
    pub stddev_ms: Option<f64>,
}

/// Accepts a hostname or numeric address; rejects anything a shell or option parser could misread.
pub fn valid_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && !host.starts_with('-')
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ".-_:%".contains(c))
        && host.matches('%').count() <= 1
}

/// Parses BSD/macOS and Linux `ping` summaries.
pub fn parse_ping(text: &str) -> Result<PingStats, String> {
    let mut sent = None;
    let mut received = None;
    let mut loss = None;
    let mut rtt = (None, None, None, None);
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
                    .filter_map(|v| v.parse::<f64>().ok())
                    .filter(|v| v.is_finite())
                    .collect();
                if nums.len() >= 3 {
                    rtt = (
                        Some(nums[0]),
                        Some(nums[1]),
                        Some(nums[2]),
                        nums.get(3).copied(),
                    );
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
        stddev_ms: rtt.3,
    })
}

/// The program and arguments for a quiet ping. IPv6 hosts need `ping6` on macOS, which has no `-t`
/// and sets do-not-fragment implicitly; for IPv4 a size always means a do-not-fragment probe.
pub fn ping_command(
    host: &str,
    count: u32,
    size: Option<u32>,
    interval: Option<f64>,
    wait_secs: u32,
) -> (&'static str, Vec<String>) {
    let v6 = host.contains(':');
    let mut args: Vec<String> = vec!["-c".into(), count.to_string(), "-n".into(), "-q".into()];
    if let Some(i) = interval {
        args.extend(["-i".into(), format!("{i}")]);
    }
    if !v6 {
        args.extend(["-t".into(), wait_secs.to_string()]);
    }
    if let Some(size) = size {
        if !v6 {
            args.push("-D".into());
        }
        args.extend(["-s".into(), size.to_string()]);
    }
    args.push(host.into());
    (if v6 { "/sbin/ping6" } else { "/sbin/ping" }, args)
}

pub fn ping_stats(host: &str, count: u32, packet_size: Option<u32>) -> Result<PingStats, String> {
    if !valid_host(host) {
        return Err("Use a hostname or IP address, not a URL or shell expression".into());
    }
    if !cfg!(target_os = "macos") {
        return Err("ping requires macOS".into());
    }
    let (program, args) = ping_command(host, count, packet_size, None, 10);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (_, out, err) = collect::capture(program, &args, 15)?;
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
