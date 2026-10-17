//! Turns [`Facts`] into steps, a verdict and next steps. Pure: no I/O, so every scenario is a test.
//!
//! The verdict follows the layers from the bottom up: a sign-in page, then link, router, internet,
//! DNS, HTTPS, path MTU, Wi-Fi quality, speed and IPv6. Failures outrank warnings; within each, the
//! lowest layer wins, because everything above it is a symptom.
use super::facts::*;
use crate::probe::PingStats;

#[derive(Debug, Clone)]
pub struct Report {
    pub steps: Vec<Step>,
    pub verdict: String,
    pub advice: Vec<String>,
    /// Other problems found besides the one the verdict names.
    pub also: Vec<String>,
    pub target: Option<String>,
    /// Addresses that identify this network, for `redact`.
    pub secrets: Vec<String>,
}
impl Report {
    pub fn worst(&self) -> Status {
        if self.steps.iter().any(|s| s.status == Status::Fail) {
            Status::Fail
        } else if self.steps.iter().any(|s| s.status == Status::Warn) {
            Status::Warn
        } else {
            Status::Ok
        }
    }
}

struct Finding {
    rank: u8,
    status: Status,
    headline: String,
    advice: Vec<String>,
}

const WARN_LOSS: f64 = 0.0;
const WARN_JITTER_ROUTER: f64 = 20.0;
const WARN_JITTER_INTERNET: f64 = 30.0;
const WARN_TCP_MS: f64 = 400.0;
const WARN_DNS_MS: f64 = 200.0;
const WARN_DNS_FRESH_MS: f64 = 600.0;

fn ms(v: f64) -> String {
    if v < 10.0 {
        format!("{v:.1} ms")
    } else {
        format!("{v:.0} ms")
    }
}
fn opt_ms(v: Option<f64>) -> String {
    v.map_or("n/a".into(), ms)
}

fn ping_summary(p: &PingStats) -> String {
    let mut s = format!("{:.0}% loss", p.loss_pct);
    if let Some(a) = p.avg_ms {
        s += &format!(", {} avg", ms(a));
    }
    if let Some(j) = p.stddev_ms {
        s += &format!(", jitter {}", ms(j));
    }
    s
}

fn internet_ok(f: &Facts) -> bool {
    f.internet_tcp.iter().any(|(_, t)| matches!(t, Tcp::Ok(_)))
        || f.internet_ping.as_ref().is_some_and(|p| p.received > 0)
        || f.http4.as_ref().is_some_and(|h| h.ok)
}

fn weak_wifi(f: &Facts) -> bool {
    f.wifi
        .as_ref()
        .is_some_and(|w| w.rssi.is_some_and(|r| r < -70) || w.snr().is_some_and(|s| s < 20))
}

const WIFI_ADVICE: &str =
    "Wi-Fi signal is weak: move closer to the router, remove obstacles, or use its 5 GHz network.";

fn link_step(f: &Facts) -> (Step, Option<Finding>) {
    let l = &f.link;
    let Some(iface) = &l.iface else {
        return fail_link("No active network adapter. Check Wi-Fi or the cable.");
    };
    let name = format!("{iface} ({})", l.label);
    if !l.active {
        return fail_link(&format!("{name} has no link. Check Wi-Fi or the cable."));
    }
    if l.ipv4.is_empty() && !l.global_ipv6 {
        let why = if l.self_assigned {
            "only a self-assigned 169.254.x.x address, so DHCP did not answer"
        } else {
            "no usable address"
        };
        return fail_link(&format!("{name} is connected but has {why}."));
    }
    let mut addrs = l.ipv4.clone();
    if l.global_ipv6 {
        addrs.push("global IPv6".into());
    }
    (
        Step::new("Link", Status::Ok, format!("{name}: {}", addrs.join(", "))),
        None,
    )
}
fn fail_link(detail: &str) -> (Step, Option<Finding>) {
    (
        Step::new("Link", Status::Fail, detail),
        Some(Finding {
            rank: 1,
            status: Status::Fail,
            headline: "No usable local connection. Check Wi-Fi or the cable, then that DHCP gave this Mac an address.".into(),
            advice: vec![
                "Turn Wi-Fi off and on, or replug the cable.".into(),
                "Renew the DHCP lease: System Settings → Network → your connection → Details → TCP/IP → Renew DHCP Lease.".into(),
            ],
        }),
    )
}

fn route_step(f: &Facts) -> Option<Step> {
    if let Some(t) = &f.link.tunnel {
        return Some(Step::new(
            "Route",
            Status::Info,
            format!("Default route goes through the tunnel {t}; MTU and speed depend on it (macora does not identify the VPN)."),
        ));
    }
    let r = &f.route;
    r.interface.as_ref().map(|i| {
        let via = r
            .gateway
            .as_deref()
            .map_or(String::new(), |g| format!(", gateway {g}"));
        Step::new(
            "Route",
            Status::Info,
            format!("Default route uses {i} ({}){via}.", f.link.label),
        )
    })
}

fn gateway_step(f: &Facts) -> (Step, Option<Finding>) {
    let wifi_note = weak_wifi(f);
    let Some(g) = &f.gateway else {
        if f.link.tunnel.is_some() {
            return (
                Step::new(
                    "Gateway",
                    Status::Info,
                    "The default route is a tunnel and has no router address to ping.",
                ),
                None,
            );
        }
        return (
            Step::new(
                "Gateway",
                Status::Fail,
                "No default route: nothing tells this Mac where to send traffic.",
            ),
            Some(Finding {
                rank: 2,
                status: Status::Fail,
                headline: "This Mac has no route to the internet: the router did not hand one out."
                    .into(),
                advice: vec![
                    "Renew the DHCP lease, or reconnect to the network.".into(),
                    "Check that the router is on and has a working WAN link.".into(),
                ],
            }),
        );
    };
    let Some(p) = &f.gateway_ping else {
        return (
            Step::new("Gateway", Status::Info, format!("{g}: could not run ping.")),
            None,
        );
    };
    let detail = format!("{g}: {}", ping_summary(p));
    let flow = internet_ok(f);
    if p.loss_pct >= 100.0 || p.received == 0 {
        if flow {
            return (
                Step::new(
                    "Gateway",
                    Status::Warn,
                    format!("{detail}; the router ignores ping but traffic flows"),
                ),
                Some(Finding {
                    rank: 2,
                    status: Status::Warn,
                    headline:
                        "The router does not answer ping, but your traffic reaches the internet."
                            .into(),
                    advice: vec![
                        "Usually harmless: many routers and guest Wi-Fi networks block ping."
                            .into(),
                    ],
                }),
            );
        }
        let mut advice = vec![
            "Check that the router is on, then restart it.".into(),
            "Try another device on the same network to see if it is the router.".into(),
        ];
        if wifi_note {
            advice.insert(0, WIFI_ADVICE.into());
        }
        return (
            Step::new("Gateway", Status::Fail, detail).ms(None),
            Some(Finding {
                rank: 2,
                status: Status::Fail,
                headline: "This Mac cannot reach its router. The problem is local: Wi-Fi signal, cable, or the router itself.".into(),
                advice,
            }),
        );
    }
    let limit = if f.link.is_wifi { 50.0 } else { 20.0 };
    let bad_loss = p.loss_pct > WARN_LOSS;
    let bad_lat = p.avg_ms.is_some_and(|a| a > limit);
    let bad_jit = p.stddev_ms.is_some_and(|j| j > WARN_JITTER_ROUTER);
    if bad_loss || bad_lat || bad_jit {
        let what = match (bad_loss, bad_lat, bad_jit) {
            (true, _, _) => "packet loss to the router",
            (_, true, _) => "high latency to the router",
            _ => "unsteady latency (jitter) to the router",
        };
        let mut advice =
            vec!["Restart the router, or connect by cable to see if the problem goes away.".into()];
        if f.link.is_wifi {
            advice.insert(0, if wifi_note { WIFI_ADVICE.to_string() } else { "Wi-Fi interference or a busy channel can cause this; try the 5 GHz network or another channel.".into() });
        }
        return (
            Step::new("Gateway", Status::Warn, detail).ms(p.avg_ms),
            Some(Finding {
                rank: 2,
                status: Status::Warn,
                headline: format!("Connected, but with {what}. Slowness starts on your local network, before the internet."),
                advice,
            }),
        );
    }
    (Step::new("Gateway", Status::Ok, detail).ms(p.avg_ms), None)
}

fn internet_step(f: &Facts) -> (Step, Option<Finding>) {
    let oks: Vec<(&str, f64)> = f
        .internet_tcp
        .iter()
        .filter_map(|(a, t)| match t {
            Tcp::Ok(m) => Some((a.as_str(), *m)),
            _ => None,
        })
        .collect();
    let down: Vec<String> = f
        .internet_tcp
        .iter()
        .filter_map(|(a, t)| match t {
            Tcp::Fail(e) => Some(format!("{} {e}", a.trim_end_matches(":443"))),
            _ => None,
        })
        .collect();
    let ping_alive = f.internet_ping.as_ref().is_some_and(|p| p.received > 0);
    let mut parts: Vec<String> = oks
        .iter()
        .map(|(a, m)| format!("{} {}", a.trim_end_matches(":443"), ms(*m)))
        .collect();
    parts.extend(down.iter().map(|d| format!("{d} ✗")));
    let mut detail = parts.join(" · ");
    if let Some(p) = &f.internet_ping {
        detail += &format!("; ping 1.1.1.1: {}", ping_summary(p));
    }
    if oks.is_empty() && !ping_alive && !f.http4.as_ref().is_some_and(|h| h.ok) {
        return (
            Step::new("Internet", Status::Fail, detail),
            Some(Finding {
                rank: 3,
                status: Status::Fail,
                headline: "The router answers but the internet does not. Restart the router/modem or contact your ISP.".into(),
                advice: vec![
                    "Restart the modem and router (unplug for 30 seconds).".into(),
                    "If it stays down, check your ISP's status page or call them.".into(),
                ],
            }),
        );
    }
    if oks.is_empty() {
        return (
            Step::new("Internet", Status::Warn, format!("{detail}; ping works but TCP port 443 is blocked")),
            Some(Finding {
                rank: 3,
                status: Status::Warn,
                headline: "Ping works but web connections (TCP 443) are blocked, so a firewall or filter is in the way.".into(),
                advice: vec!["Check for a VPN, firewall or content filter, or a network that restricts HTTPS.".into()],
            }),
        );
    }
    let best = oks.iter().map(|(_, m)| *m).fold(f64::MAX, f64::min);
    let loss = f.internet_ping.as_ref().map_or(0.0, |p| p.loss_pct);
    let jitter = f
        .internet_ping
        .as_ref()
        .and_then(|p| p.stddev_ms)
        .unwrap_or(0.0);
    if loss > 2.0 || jitter > WARN_JITTER_INTERNET || best > WARN_TCP_MS {
        let mut advice = vec![
            "Test again in a minute; if it persists, restart the router or contact your ISP."
                .into(),
        ];
        if weak_wifi(f) {
            advice.insert(0, WIFI_ADVICE.into());
        }
        return (
            Step::new("Internet", Status::Warn, detail).ms(Some(best)),
            Some(Finding {
                rank: 3,
                status: Status::Warn,
                headline: "Connected, but the internet path has packet loss, high latency or unsteady latency (jitter).".into(),
                advice,
            }),
        );
    }
    (
        Step::new("Internet", Status::Ok, detail).ms(Some(best)),
        None,
    )
}

fn portal_step(f: &Facts) -> (Option<Step>, Option<Finding>) {
    match &f.captive {
        None => (None, None),
        Some(Captive::Clear) => (
            Some(Step::new("Portal", Status::Ok, "No sign-in page is intercepting web traffic.")),
            None,
        ),
        Some(Captive::Unknown(why)) => (
            Some(Step::new("Portal", Status::Info, format!("Could not check for a sign-in page ({why})."))),
            None,
        ),
        Some(Captive::Intercepted(what)) => (
            Some(Step::new("Portal", Status::Fail, format!("Web traffic is being intercepted ({what}).")))
            ,
            Some(Finding {
                rank: 0,
                status: Status::Fail,
                headline: "A sign-in page (captive portal) is intercepting web traffic. Open a browser and sign in to this network.".into(),
                advice: vec![
                    "Open any http:// website in a browser to bring up the sign-in page.".into(),
                    "Until you sign in, apps will look offline even though the connection is up.".into(),
                ],
            }),
        ),
    }
}

fn dns_label(p: &DnsProbe) -> String {
    if p.ok {
        format!("{} {}/{}", p.server, opt_ms(p.ms), opt_ms(p.fresh_ms))
    } else {
        format!("{} ✗ {}", p.server, p.detail)
    }
}

fn dns_step(f: &Facts) -> (Step, Option<Finding>) {
    let mut parts: Vec<String> = f.dns.iter().map(dns_label).collect();
    if let Some(r) = &f.dns_reference {
        parts.push(format!("reference {}", dns_label(r)));
    }
    match &f.dns_system {
        Some(Ok(m)) => parts.push(format!("system resolver {}", ms(*m))),
        Some(Err(e)) => parts.push(format!("system resolver ✗ {e}")),
        None => {}
    }
    let detail = if parts.is_empty() {
        "No DNS information.".to_string()
    } else {
        parts.join(" · ")
    };
    let sys_ok = matches!(f.dns_system, Some(Ok(_)));
    let sys_ms = match f.dns_system {
        Some(Ok(m)) => Some(m),
        _ => None,
    };
    let ref_ok = f.dns_reference.as_ref().is_some_and(|r| r.ok);
    let working: Vec<&DnsProbe> = f.dns.iter().filter(|p| p.ok).collect();
    let best = working
        .iter()
        .filter_map(|p| p.ms)
        .fold(None, |a: Option<f64>, m| Some(a.map_or(m, |x| x.min(m))))
        .or(sys_ms);

    if f.dns_system.as_ref().is_some_and(|r| r.is_err()) || (f.dns.is_empty() && !sys_ok) {
        return (
            Step::new("DNS", Status::Fail, detail),
            Some(Finding {
                rank: 4,
                status: Status::Fail,
                headline: "Internet reachable by IP but names do not resolve. Try another DNS server or check VPN/DNS settings.".into(),
                advice: vec![
                    "Set DNS to 1.1.1.1 or 8.8.8.8: System Settings → Network → your connection → Details → DNS.".into(),
                    "If a VPN is on, turn it off and test again.".into(),
                ],
            }),
        );
    }
    if !f.dns.is_empty() && working.is_empty() {
        let (status, headline) = if sys_ok {
            (Status::Warn, "None of your configured DNS servers answer direct queries, though the system resolver still works.")
        } else {
            (
                Status::Fail,
                "None of your DNS servers answer, so names cannot be looked up.",
            )
        };
        let mut advice = vec!["Set DNS to 1.1.1.1 or 8.8.8.8: System Settings → Network → your connection → Details → DNS.".to_string()];
        if ref_ok {
            advice.push("1.1.1.1 does answer, so switching resolvers should fix it.".into());
        }
        return (
            Step::new("DNS", status, detail).ms(best),
            Some(Finding {
                rank: 4,
                status,
                headline: headline.into(),
                advice,
            }),
        );
    }
    let dead: Vec<&DnsProbe> = f.dns.iter().filter(|p| !p.ok).collect();
    let slow = working.iter().find(|p| {
        p.ms.is_some_and(|m| m > WARN_DNS_MS)
            || p.fresh_ms.is_some_and(|m| m > WARN_DNS_FRESH_MS)
            || matches!((p.fresh_ms, f.dns_reference.as_ref().and_then(|r| r.fresh_ms)), (Some(a), Some(b)) if a > 3.0 * b + 100.0)
    });
    let sys_slow = sys_ms.is_some_and(|m| m > 300.0);
    if !dead.is_empty() || slow.is_some() || sys_slow {
        let headline = if let Some(d) = dead.first() {
            format!("DNS server {} is not answering (others are). Names may resolve slowly while macOS retries.", d.server)
        } else if let Some(p) = slow {
            format!("Connected, but DNS is slow ({} answers in {}). A different resolver may speed up page loads.", p.server, opt_ms(p.fresh_ms.or(p.ms)))
        } else {
            "Connected, but DNS is slow. A different resolver may speed up page loads.".into()
        };
        let mut advice = vec![
            "Try 1.1.1.1 or 8.8.8.8: System Settings → Network → your connection → Details → DNS."
                .to_string(),
        ];
        if let Some(r) = f.dns_reference.as_ref().filter(|r| r.ok) {
            advice.push(format!(
                "For comparison, 1.1.1.1 answers in {}.",
                opt_ms(r.fresh_ms.or(r.ms))
            ));
        }
        return (
            Step::new("DNS", Status::Warn, detail).ms(best),
            Some(Finding {
                rank: 4,
                status: Status::Warn,
                headline,
                advice,
            }),
        );
    }
    (Step::new("DNS", Status::Ok, detail).ms(best), None)
}

fn stages(h: &Http) -> String {
    let mut s = format!("dns {} · tcp {}", ms(h.dns_ms), ms(h.tcp_stage()));
    if h.tls_ms > 0.0 {
        s += &format!(" · tls {}", ms(h.tls_stage()));
    }
    s += &format!(
        " · first byte {} (total {})",
        ms(h.wait_stage()),
        ms(h.total_ms)
    );
    s
}

/// Which stage of an HTTPS request is unusually slow, if any.
fn slow_stage(h: &Http) -> Option<(&'static str, f64, &'static str)> {
    if h.dns_ms > 300.0 {
        Some((
            "DNS lookup",
            h.dns_ms,
            "Try a different DNS server (1.1.1.1 or 8.8.8.8).",
        ))
    } else if h.tcp_stage() > 400.0 {
        Some((
            "TCP connect",
            h.tcp_stage(),
            "The network path to the site is slow or lossy; check Wi-Fi/ISP.",
        ))
    } else if h.tls_stage() > 500.0 {
        Some((
            "TLS handshake",
            h.tls_stage(),
            "A proxy, antivirus or filter inspecting HTTPS can slow the handshake.",
        ))
    } else if h.wait_stage() > 1000.0 {
        Some((
            "server response",
            h.wait_stage(),
            "The site itself is slow to answer; your network is not the bottleneck.",
        ))
    } else {
        None
    }
}

fn https_step(f: &Facts) -> (Option<Step>, Option<Finding>) {
    let Some(h4) = &f.http4 else {
        return (None, None);
    };
    if !h4.ok {
        let why = h4.error.clone().unwrap_or_else(|| "failed".into());
        return (
            Some(Step::new(
                "HTTPS",
                Status::Warn,
                format!("{} over IPv4 failed: {why}", h4.host),
            )),
            Some(Finding {
                rank: 5,
                status: Status::Warn,
                headline: format!(
                    "Web connections to {} fail even though lower layers work ({why}).",
                    h4.host
                ),
                advice: vec![
                    "Check for a proxy, VPN, firewall or content filter that blocks HTTPS.".into(),
                ],
            }),
        );
    }
    let mut detail = format!("{} over IPv4: {}", h4.host, stages(h4));
    if let Some(h6) = &f.http6 {
        if h6.ok {
            detail += &format!("; over IPv6 total {}", ms(h6.total_ms));
        } else {
            detail += &format!("; over IPv6 ✗ {}", h6.error.clone().unwrap_or_default());
        }
    }
    if let Some((stage, v, tip)) = slow_stage(h4) {
        return (
            Some(Step::new("HTTPS", Status::Warn, detail).ms(Some(h4.total_ms))),
            Some(Finding {
                rank: 5,
                status: Status::Warn,
                headline: format!(
                    "Connected, but the {stage} is slow ({}) when loading {}.",
                    ms(v),
                    h4.host
                ),
                advice: vec![tip.into()],
            }),
        );
    }
    (
        Some(Step::new("HTTPS", Status::Ok, detail).ms(Some(h4.total_ms))),
        None,
    )
}

fn mtu_step(f: &Facts) -> (Option<Step>, Option<Finding>) {
    let Some(p) = &f.pmtu else {
        let skipped = Step::new("MTU", Status::Skip, "Skipped in quick mode.");
        return (f.quick.then_some(skipped), None);
    };
    match &p.result {
        PmtuResult::Measured(m) if *m >= p.link_mtu => (
            Some(Step::new("MTU", Status::Ok, format!("Path MTU to {} is {m} bytes, the full link size.", p.target))),
            None,
        ),
        PmtuResult::Measured(m) => (
            Some(Step::new(
                "MTU",
                Status::Warn,
                format!("Path MTU to {} is {m} bytes, below this Mac's {} ({}).", p.target, p.link_mtu, super::pmtu::cause_hint(*m)),
            )),
            Some(Finding {
                rank: 6,
                status: Status::Warn,
                headline: format!("Packets larger than {m} bytes are dropped on the way to the internet (this Mac sends up to {}).", p.link_mtu),
                advice: vec![
                    format!("Set this connection's MTU to {m}: System Settings → Network → Details → Hardware → MTU: Manual."),
                    "Or ask your ISP/VPN admin to clamp TCP MSS; large uploads and some sites stall when this is wrong.".into(),
                ],
            }),
        ),
        PmtuResult::Unmeasurable(why) => (Some(Step::new("MTU", Status::Info, format!("Path MTU could not be measured: {why}."))), None),
    }
}

fn wifi_step(f: &Facts) -> (Option<Step>, Option<Finding>) {
    if !f.link.is_wifi {
        return (None, None);
    }
    let Some(w) = &f.wifi else {
        let step = if f.quick {
            Step::new(
                "Wi-Fi",
                Status::Skip,
                "Skipped in quick mode (reading Wi-Fi details takes about 5 seconds).",
            )
        } else {
            Step::new(
                "Wi-Fi",
                Status::Info,
                "Wi-Fi details are unavailable on this macOS version.",
            )
        };
        return (Some(step), None);
    };
    if !w.associated {
        return (
            Some(Step::new(
                "Wi-Fi",
                Status::Info,
                "Wi-Fi is on but not joined to a network.",
            )),
            None,
        );
    }
    let mut parts: Vec<String> = Vec::new();
    if let (Some(b), Some(c)) = (&w.band, w.channel) {
        let width = w
            .width
            .as_deref()
            .map_or(String::new(), |x| format!(" ({x})"));
        parts.push(format!("{b} ch {c}{width}"));
    }
    if let Some(p) = &w.phy {
        parts.push(p.clone());
    }
    if let (Some(r), Some(n)) = (w.rssi, w.noise) {
        parts.push(format!("signal {r} dBm, noise {n} dBm (SNR {} dB)", r - n));
    }
    if let Some(rate) = w.rate_mbps {
        parts.push(format!("link rate {rate} Mbps"));
    }
    let detail = parts.join(" · ");
    let rssi = w.rssi.unwrap_or(0);
    let snr = w.snr();
    let band24 = w.band.as_deref().is_some_and(|b| b.starts_with("2.4"));
    let (status, headline) = if w.rssi.is_some() && rssi < -85 {
        (
            Status::Fail,
            "Wi-Fi signal is extremely weak; the connection will be unreliable.",
        )
    } else if (w.rssi.is_some() && rssi < -75) || snr.is_some_and(|s| s < 20) {
        (
            Status::Warn,
            "Wi-Fi signal is weak or noisy, which limits speed and causes drops.",
        )
    } else if band24 {
        (Status::Info, "")
    } else {
        (Status::Ok, "")
    };
    let mut detail = detail;
    if band24 && status == Status::Info {
        detail += " · the 2.4 GHz band is slower and more crowded than 5 GHz";
    }
    let finding = (matches!(status, Status::Warn | Status::Fail)).then(|| Finding {
        rank: 7,
        status,
        headline: headline.into(),
        advice: vec![
            WIFI_ADVICE.into(),
            "Check for microwaves, baby monitors or neighbouring networks on the same channel."
                .into(),
        ],
    });
    (Some(Step::new("Wi-Fi", status, detail)), finding)
}

fn speed_step(f: &Facts) -> (Option<Step>, Option<Finding>) {
    match &f.speed {
        None => (None, None),
        Some(Err(e)) => (
            Some(Step::new(
                "Speed",
                Status::Info,
                format!("The speed test could not run: {e}"),
            )),
            None,
        ),
        Some(Ok(s)) => {
            let class = match s.rpm {
                Some(r) if r >= 800.0 => "high",
                Some(r) if r >= 200.0 => "medium",
                Some(_) => "low",
                None => "unknown",
            };
            let mut detail = format!("down {:.0} Mbps · up {:.0} Mbps", s.down_mbps, s.up_mbps);
            if let Some(r) = s.rpm {
                detail += &format!(" · responsiveness {r:.0} RPM ({class})");
            }
            if let Some(b) = s.base_rtt_ms {
                detail += &format!(" · base RTT {}", ms(b));
            }
            detail += " (Apple networkQuality)";
            let low = s.rpm.is_some_and(|r| r < 200.0) || s.down_mbps < 5.0;
            let finding = low.then(|| Finding {
                rank: 8,
                status: Status::Warn,
                headline: if s.down_mbps < 5.0 {
                    format!("Download speed is very low ({:.1} Mbps).", s.down_mbps)
                } else {
                    "Responsiveness under load is low: video calls and games lag when someone downloads (bufferbloat).".into()
                },
                advice: vec!["Enable SQM/smart queue management on the router if it has it, or limit uploads while you work.".into()],
            });
            (
                Some(
                    Step::new("Speed", if low { Status::Warn } else { Status::Ok }, detail)
                        .ms(s.base_rtt_ms),
                ),
                finding,
            )
        }
    }
}

fn ipv6_step(f: &Facts) -> (Step, Option<Finding>) {
    let v = &f.ipv6;
    if !v.global_address {
        return (
            Step::new(
                "IPv6",
                Status::Info,
                "No global IPv6 address; an IPv4-only network is fine.",
            ),
            None,
        );
    }
    if !v.default_route {
        return (
            Step::new("IPv6", Status::Warn, "Has a global IPv6 address but no IPv6 default route."),
            Some(Finding {
                rank: 9,
                status: Status::Warn,
                headline: "IPv6 is half-configured (an address but no route), which can delay connections while apps fall back to IPv4.".into(),
                advice: vec!["Reconnect to the network, or set IPv6 to \"Link-local only\" for this connection if the router has no IPv6.".into()],
            }),
        );
    }
    match &v.tcp {
        Some(Tcp::Ok(m)) => (Step::new("IPv6", Status::Ok, format!("IPv6 reaches the internet ({}).", ms(*m))).ms(Some(*m)), None),
        Some(Tcp::Fail(e)) => (
            Step::new("IPv6", Status::Warn, format!("IPv6 is configured but cannot reach the internet ({e}).")),
            Some(Finding {
                rank: 9,
                status: Status::Warn,
                headline: "IPv6 is configured but does not work, so some sites take several seconds to load while apps fall back to IPv4.".into(),
                advice: vec!["Ask your ISP about IPv6, or set IPv6 to \"Link-local only\" for this connection.".into()],
            }),
        ),
        None => (Step::new("IPv6", Status::Info, "IPv6 reachability was not tested."), None),
    }
}

fn proxy_step(f: &Facts) -> Option<Step> {
    let p = &f.proxy;
    if !p.any() {
        return None;
    }
    let mut what: Vec<String> = Vec::new();
    for (name, v) in [("HTTP", &p.http), ("HTTPS", &p.https), ("SOCKS", &p.socks)] {
        if let Some(v) = v {
            what.push(format!("{name} proxy {v}"));
        }
    }
    if let Some(u) = &p.pac {
        what.push(format!("auto-config script {u}"));
    }
    if p.auto_discovery {
        what.push("automatic proxy discovery".into());
    }
    Some(Step::new(
        "Proxy",
        Status::Info,
        format!(
            "This Mac is configured to use {}; browsers and apps go through it.",
            what.join(", ")
        ),
    ))
}

fn target_steps(f: &Facts) -> Vec<(Step, Option<Finding>)> {
    let Some(t) = &f.target else { return vec![] };
    let mut out = Vec::new();
    // DNS for the site, per resolver and against the reference.
    let mut parts: Vec<String> = t
        .dns
        .iter()
        .map(|p| {
            if p.ok {
                format!("{} {}", p.server, opt_ms(p.ms))
            } else {
                format!("{} ✗ {}", p.server, p.detail)
            }
        })
        .collect();
    if let Some(r) = &t.dns_reference {
        parts.push(if r.ok {
            format!("reference {} {}", r.server, opt_ms(r.ms))
        } else {
            format!("reference {} ✗ {}", r.server, r.detail)
        });
    }
    let detail = format!("{}: {}", t.host, parts.join(" · "));
    let any = t.dns.iter().any(|p| p.ok);
    let ref_ok = t.dns_reference.as_ref().is_some_and(|r| r.ok);
    if !any && !t.dns.is_empty() {
        let (headline, advice) = if ref_ok {
            (
                format!(
                    "Your DNS servers cannot resolve {} but 1.1.1.1 can.",
                    t.host
                ),
                "Switch DNS to 1.1.1.1 or 8.8.8.8.",
            )
        } else {
            (format!("{} does not resolve anywhere; check the spelling, or the site's DNS may be down.", t.host), "Try again later, or check the site's status page.")
        };
        out.push((
            Step::new("Site DNS", Status::Fail, detail),
            Some(Finding {
                rank: 4,
                status: Status::Fail,
                headline,
                advice: vec![advice.into()],
            }),
        ));
    } else {
        let slow = t
            .dns
            .iter()
            .find(|p| p.ok && p.ms.is_some_and(|m| m > WARN_DNS_MS));
        let st = if slow.is_some() {
            Status::Warn
        } else {
            Status::Ok
        };
        let fnd = slow.map(|p| Finding {
            rank: 4,
            status: Status::Warn,
            headline: format!(
                "Looking up {} is slow on {} ({}).",
                t.host,
                p.server,
                opt_ms(p.ms)
            ),
            advice: vec!["Try a different DNS server (1.1.1.1 or 8.8.8.8).".into()],
        });
        out.push((Step::new("Site DNS", st, detail), fnd));
    }
    if let Some(h) = &t.http {
        if !h.ok {
            let why = h.error.clone().unwrap_or_else(|| "failed".into());
            out.push((
                Step::new("Site HTTPS", Status::Fail, format!("{}: {why}", t.host)),
                Some(Finding {
                    rank: 5,
                    status: Status::Fail,
                    headline: format!("Cannot open https://{}: {why}.", t.host),
                    advice: vec![
                        "If other sites work, the problem is the site or a filter that blocks it."
                            .into(),
                    ],
                }),
            ));
        } else if let Some((stage, v, tip)) = slow_stage(h) {
            out.push((
                Step::new(
                    "Site HTTPS",
                    Status::Warn,
                    format!("{}: {}", t.host, stages(h)),
                )
                .ms(Some(h.total_ms)),
                Some(Finding {
                    rank: 5,
                    status: Status::Warn,
                    headline: format!("{} is slow in the {stage} ({}).", t.host, ms(v)),
                    advice: vec![tip.into()],
                }),
            ));
        } else {
            out.push((
                Step::new(
                    "Site HTTPS",
                    Status::Ok,
                    format!("{}: {}", t.host, stages(h)),
                )
                .ms(Some(h.total_ms)),
                None,
            ));
        }
    }
    let mut path = Vec::new();
    let mut warn = false;
    if let Some(p) = &t.ping {
        path.push(ping_summary(p));
        warn |= p.loss_pct > 2.0 || p.stddev_ms.is_some_and(|j| j > 30.0);
    } else {
        path.push("does not answer ping".into());
    }
    if let Some(m) = &t.pmtu {
        match &m.result {
            PmtuResult::Measured(v) if *v < m.link_mtu => {
                path.push(format!("path MTU {v}"));
                warn = true;
            }
            PmtuResult::Measured(v) => path.push(format!("path MTU {v}")),
            PmtuResult::Unmeasurable(_) => {}
        }
    }
    out.push((
        Step::new("Site path", if warn { Status::Warn } else { Status::Ok }, format!("{}: {}", t.host, path.join(" · "))).ms(t.ping.as_ref().and_then(|p| p.avg_ms)),
        warn.then(|| Finding {
            rank: 6,
            status: Status::Warn,
            headline: format!("The network path to {} has loss, jitter or a reduced packet size.", t.host),
            advice: vec!["Compare with another site; if only this one is affected, the problem is between your ISP and the site.".into()],
        }),
    ));
    out
}

pub fn analyze(f: &Facts) -> Report {
    let mut steps: Vec<Step> = Vec::new();
    let mut findings: Vec<Finding> = Vec::new();
    let mut push = |s: Step, fnd: Option<Finding>| {
        steps.push(s);
        findings.extend(fnd);
    };

    let (link, lf) = link_step(f);
    let link_failed = link.status == Status::Fail;
    push(link, lf);
    if link_failed {
        for name in ["Gateway", "Internet", "DNS", "HTTPS", "MTU"] {
            push(
                Step::new(name, Status::Skip, "Skipped: there is no working link."),
                None,
            );
        }
    } else {
        if let Some(r) = route_step(f) {
            push(r, None);
        }
        let (w, wf) = wifi_step(f);
        if let Some(w) = w {
            push(w, wf);
        }
        let (g, gf) = gateway_step(f);
        push(g, gf);
        let (i, inf) = internet_step(f);
        push(i, inf);
        let (p, pf) = portal_step(f);
        if let Some(p) = p {
            push(p, pf);
        }
        let (d, df) = dns_step(f);
        push(d, df);
        let (h, hf) = https_step(f);
        if let Some(h) = h {
            push(h, hf);
        }
        let (m, mf) = mtu_step(f);
        if let Some(m) = m {
            push(m, mf);
        }
        let (v6, v6f) = ipv6_step(f);
        push(v6, v6f);
        if let Some(px) = proxy_step(f) {
            push(px, None);
        }
        for (s, fnd) in target_steps(f) {
            push(s, fnd);
        }
        let (sp, spf) = speed_step(f);
        if let Some(sp) = sp {
            push(sp, spf);
        }
    }

    // Failures outrank warnings; within each, the lowest layer wins.
    findings.sort_by_key(|x| (x.status != Status::Fail, x.rank));
    let (verdict, advice, also) = match findings.split_first() {
        Some((top, rest)) => (
            top.headline.clone(),
            top.advice.iter().take(3).cloned().collect(),
            rest.iter().map(|x| x.headline.clone()).collect(),
        ),
        None => {
            let mut v = "No problem found on this path right now. If an app is slow, the issue is probably the remote service.".to_string();
            if let Some(t) = &f.target {
                v = format!("Your connection looks healthy and {} looks fine from here. If it is still slow, the problem is likely on the site's side.", t.host);
            }
            (v, vec![], vec![])
        }
    };
    Report {
        steps,
        verdict,
        advice,
        also,
        target: f.target.as_ref().map(|t| t.host.clone()),
        secrets: f.secrets(),
    }
}
