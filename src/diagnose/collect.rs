//! Runs the probes and fills in [`Facts`]. All process and socket access goes through [`Runner`], so
//! tests can replay recorded tool output. Independent latency probes run in parallel threads; the
//! Wi-Fi query (a ~5 s scan that distorts latency) and the opt-in speed test run afterwards, alone.
use super::{facts::*, parse, pmtu};
use crate::{collect as sys, model::Snapshot, probe};
use std::{
    net::{TcpStream, ToSocketAddrs},
    thread,
    time::{Duration, Instant},
};

pub trait Runner: Sync {
    /// Runs a program (no shell). Returns `(exited successfully, stdout, stderr)`.
    fn run(
        &self,
        program: &str,
        args: &[&str],
        secs: u64,
    ) -> Result<(bool, String, String), String>;
    /// Opens a TCP connection and returns the time it took in milliseconds.
    fn tcp(&self, addr: &str, timeout_ms: u64) -> Result<f64, String>;
}

pub struct RealRunner;
impl Runner for RealRunner {
    fn run(
        &self,
        program: &str,
        args: &[&str],
        secs: u64,
    ) -> Result<(bool, String, String), String> {
        sys::capture(program, args, secs)
    }
    fn tcp(&self, addr: &str, timeout_ms: u64) -> Result<f64, String> {
        let target = addr
            .to_socket_addrs()
            .map_err(|e| e.to_string())?
            .next()
            .ok_or("no address")?;
        let started = Instant::now();
        TcpStream::connect_timeout(&target, Duration::from_millis(timeout_ms))
            .map(|_| started.elapsed().as_secs_f64() * 1000.0)
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => "timed out".into(),
                std::io::ErrorKind::ConnectionRefused => "connection refused".into(),
                _ => e.to_string(),
            })
    }
}

#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Run Apple's `networkQuality` (≈20 s, uses real bandwidth). Off unless asked for.
    pub speed: bool,
    /// Check one specific site as well (`diagnose to HOST`).
    pub target: Option<String>,
    /// Skip the two slow checks (Wi-Fi details ~5 s, path MTU ~3 s) for a ~3 s run.
    pub quick: bool,
}

const INTERNET_TCP: [&str; 3] = ["1.1.1.1:443", "8.8.8.8:443", "9.9.9.9:443"];
const REFERENCE_DNS: &str = "1.1.1.1";
const HTTPS_HOST: &str = "www.apple.com";
const CURL_FORMAT: &str = "dns=%{time_namelookup} connect=%{time_connect} tls=%{time_appconnect} ttfb=%{time_starttransfer} total=%{time_total} ip=%{remote_ip} http=%{http_code}";

fn out(r: &dyn Runner, program: &str, args: &[&str], secs: u64) -> Option<String> {
    r.run(program, args, secs).ok().map(|(_, o, _)| o)
}

fn ping(
    r: &dyn Runner,
    host: &str,
    count: u32,
    size: Option<u32>,
    interval: Option<f64>,
    wait: u32,
) -> Option<probe::PingStats> {
    if !probe::valid_host(host) {
        return None;
    }
    let (program, args) = probe::ping_command(host, count, size, interval, wait);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let secs = (count as f64 * interval.unwrap_or(1.0)) as u64 + wait as u64 + 3;
    probe::parse_ping(&out(r, program, &args, secs)?).ok()
}

fn route(r: &dyn Runner, v6: bool) -> Route {
    let args: Vec<&str> = if v6 {
        vec!["-n", "get", "-inet6", "default"]
    } else {
        vec!["-n", "get", "default"]
    };
    out(r, "/sbin/route", &args, 5)
        .map(|t| parse::route(&t))
        .unwrap_or_default()
}

fn link_fact(snap: &Snapshot, r4: &Route, r6: &Route) -> LinkFact {
    let routed = r4.interface.as_ref().or(r6.interface.as_ref());
    let by_name = |n: &str| snap.interfaces.iter().find(|i| i.name == n);
    let routed_if = routed.and_then(|n| by_name(n));
    let tunnel = routed_if
        .filter(|i| i.group() == "Tunnels")
        .map(|i| i.name.clone());
    // A tunnel is not the link: look at the physical adapter underneath it.
    let physical = routed_if
        .filter(|i| i.group() != "Tunnels")
        .or_else(|| {
            snap.interfaces
                .iter()
                .find(|i| i.group() == "Network" && i.active() && !i.addresses.is_empty())
        })
        .or_else(|| {
            snap.interfaces
                .iter()
                .find(|i| i.group() == "Network" && i.active())
        });
    let Some(i) = physical else {
        return LinkFact {
            tunnel,
            ..Default::default()
        };
    };
    let label = i.label();
    LinkFact {
        iface: Some(i.name.clone()),
        active: i.active(),
        ipv4: i
            .addresses
            .iter()
            .filter(|a| !a.ip.contains(':') && !a.ip.starts_with("169.254."))
            .map(|a| a.ip.clone())
            .collect(),
        self_assigned: i.addresses.iter().any(|a| a.ip.starts_with("169.254.")),
        global_ipv6: i
            .addresses
            .iter()
            .any(|a| a.ip.contains(':') && a.scope() == "global-range"),
        is_wifi: label.contains("Wi-Fi") || label.contains("AirPort"),
        label,
        tunnel,
    }
}

fn fresh_name() -> String {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!("macora-{:x}.example.com", n & 0xff_ffff)
}

fn dig(r: &dyn Runner, server: &str, name: &str) -> parse::Dig {
    let at = format!("@{server}");
    let args = [
        "+time=2",
        "+tries=1",
        "+noall",
        "+comments",
        "+stats",
        at.as_str(),
        name,
    ];
    match r.run("/usr/bin/dig", &args, 6) {
        Ok((_, o, _)) => parse::dig(&o),
        Err(_) => parse::Dig {
            status: None,
            ms: None,
            timed_out: true,
        },
    }
}

fn describe(d: &parse::Dig) -> String {
    match (&d.status, d.timed_out) {
        (Some(s), _) => s.clone(),
        (None, true) => "timed out".into(),
        (None, false) => "no answer".into(),
    }
}

/// Asks one server for a real name and for a random one, which forces a fresh recursive lookup.
fn probe_resolver(r: &dyn Runner, server: &str, fresh: &str) -> DnsProbe {
    if !probe::valid_host(server) {
        return DnsProbe {
            server: server.into(),
            detail: "not a usable address".into(),
            ..Default::default()
        };
    }
    let (a, b) = (dig(r, server, "example.com"), dig(r, server, fresh));
    let ok = a.answered() && b.answered();
    DnsProbe {
        server: server.into(),
        ok,
        ms: a.ms,
        fresh_ms: b.ms,
        detail: if ok {
            "answered".into()
        } else if !a.answered() {
            describe(&a)
        } else {
            describe(&b)
        },
    }
}

fn http(r: &dyn Runner, host: &str, family: u8) -> Http {
    let url = format!("https://{host}/");
    let flag = if family == 6 { "-6" } else { "-4" };
    let args = [
        "-s",
        "-o",
        "/dev/null",
        "--max-time",
        "8",
        flag,
        "-w",
        CURL_FORMAT,
        url.as_str(),
    ];
    match r.run("/usr/bin/curl", &args, 12) {
        Ok((_, o, e)) => match parse::curl_timing(&o, host, family) {
            Some(mut h) => {
                if !h.ok {
                    let why = e.trim().lines().last().unwrap_or("").trim();
                    h.error = Some(if why.is_empty() {
                        "no HTTP response".into()
                    } else {
                        why.to_string()
                    });
                }
                h
            }
            None => Http {
                host: host.into(),
                family,
                error: Some(
                    e.trim()
                        .lines()
                        .last()
                        .unwrap_or("curl gave no timing")
                        .to_string(),
                ),
                ..Default::default()
            },
        },
        Err(e) => Http {
            host: host.into(),
            family,
            error: Some(e),
            ..Default::default()
        },
    }
}

fn captive(r: &dyn Runner) -> Captive {
    let args = [
        "-s",
        "--max-time",
        "4",
        "-w",
        "\n%{http_code}",
        "http://captive.apple.com/hotspot-detect.html",
    ];
    let Ok((_, body, e)) = r.run("/usr/bin/curl", &args, 8) else {
        return Captive::Unknown("curl could not run".into());
    };
    let (text, code) = body.rsplit_once('\n').unwrap_or((&body, "000"));
    let code: u32 = code.trim().parse().unwrap_or(0);
    if code == 0 {
        let why = e.trim().lines().last().unwrap_or("no response");
        return Captive::Unknown(why.to_string());
    }
    if code == 200 && text.contains("Success") {
        return Captive::Clear;
    }
    let plain: String = text
        .split(['<', '>'])
        .enumerate()
        .filter(|(i, _)| i % 2 == 0)
        .map(|(_, s)| s.trim())
        .collect::<Vec<_>>()
        .join(" ");
    let excerpt: String = plain.chars().filter(|c| !c.is_control()).take(60).collect();
    Captive::Intercepted(if excerpt.trim().is_empty() {
        format!("HTTP {code}")
    } else {
        format!("HTTP {code}: {}", excerpt.trim())
    })
}

/// Does a do-not-fragment packet of `size` bytes reach `target`? Two packets 0.2 s apart: a success
/// returns in a few milliseconds, a drop costs the one-second wait, and one stray loss cannot fool it.
fn df_works(r: &dyn Runner, target: &str, size: u32) -> bool {
    let payload = size.saturating_sub(28);
    ping(r, target, 2, Some(payload), Some(0.2), 1).is_some_and(|p| p.received > 0)
}

fn measure_pmtu(r: &dyn Runner, target: &str, link_mtu: u32) -> Option<PmtuResult> {
    if !ping(r, target, 2, None, Some(0.2), 2).is_some_and(|p| p.received > 0) {
        return None;
    }
    let works = |size: u32| df_works(r, target, size);
    Some(match pmtu::search(link_mtu, &works).mtu {
        Some(m) => PmtuResult::Measured(m),
        None => {
            PmtuResult::Unmeasurable(format!("not even {}-byte packets get through", pmtu::FLOOR))
        }
    })
}

fn path_mtu(r: &dyn Runner, link_mtu: u32) -> Pmtu {
    for target in ["1.1.1.1", "8.8.8.8"] {
        if let Some(result) = measure_pmtu(r, target, link_mtu) {
            return Pmtu {
                target: target.into(),
                link_mtu,
                result,
            };
        }
    }
    Pmtu {
        target: "1.1.1.1".into(),
        link_mtu,
        result: PmtuResult::Unmeasurable(
            "no reply to ping from 1.1.1.1 or 8.8.8.8 (ICMP may be blocked)".into(),
        ),
    }
}

fn target_facts(r: &dyn Runner, host: &str, servers: &[String]) -> TargetFacts {
    let lookup = |server: &str| {
        let d = dig(r, server, host);
        DnsProbe {
            server: server.into(),
            ok: d.answered() && d.status.as_deref() == Some("NOERROR"),
            ms: d.ms,
            fresh_ms: None,
            detail: describe(&d),
        }
    };
    let (dns, reference, http4, ping_stats) = thread::scope(|s| {
        let dns: Vec<_> = servers
            .iter()
            .filter(|sv| probe::valid_host(sv))
            .map(|sv| s.spawn(move || lookup(sv)))
            .collect();
        let reference = s.spawn(|| lookup(REFERENCE_DNS));
        let h = s.spawn(|| http(r, host, 4));
        let p = s.spawn(|| ping(r, host, 10, None, Some(0.2), 6));
        (
            dns.into_iter()
                .filter_map(|h| h.join().ok())
                .collect::<Vec<_>>(),
            reference.join().ok(),
            h.join().ok(),
            p.join().ok().flatten(),
        )
    });
    // Measure the path to the site's own address when we learned it.
    let pmtu = http4
        .as_ref()
        .and_then(|h| h.ip.clone())
        .filter(|ip| !ip.contains(':') && probe::valid_host(ip))
        .map(|ip| {
            let mut m = path_mtu_to(r, &ip, 1500);
            m.target = ip;
            m
        });
    TargetFacts {
        host: host.into(),
        dns,
        dns_reference: reference,
        http: http4,
        ping: ping_stats,
        pmtu,
    }
}

fn path_mtu_to(r: &dyn Runner, target: &str, link_mtu: u32) -> Pmtu {
    Pmtu {
        target: target.into(),
        link_mtu,
        result: measure_pmtu(r, target, link_mtu).unwrap_or(PmtuResult::Unmeasurable(
            "the host does not answer ping".into(),
        )),
    }
}

pub fn collect(r: &dyn Runner, snap: &Snapshot, opts: &Options) -> Facts {
    let (route4, route6) = thread::scope(|s| {
        let a = s.spawn(|| route(r, false));
        let b = s.spawn(|| route(r, true));
        (a.join().unwrap_or_default(), b.join().unwrap_or_default())
    });
    let link = link_fact(snap, &route4, &route6);
    let usable = |g: &Option<String>| g.clone().filter(|g| probe::valid_host(g));
    let gateway = usable(&route4.gateway).or_else(|| usable(&route6.gateway));
    let link_mtu = route4.mtu.or(route6.mtu).unwrap_or(1500);
    let servers: Vec<String> = out(r, "/usr/sbin/scutil", &["--dns"], 5)
        .map(|t| parse::dns_servers(&t))
        .unwrap_or_default()
        .into_iter()
        .take(4)
        .collect();
    let fresh = fresh_name();
    let iface = link.iface.clone().unwrap_or_default();
    let wifi_wanted = link.is_wifi && !iface.is_empty() && !opts.quick;
    let v6_wanted = link.global_ipv6 && (route6.gateway.is_some() || route6.interface.is_some());

    let mut facts = thread::scope(|s| {
        let capt = s.spawn(|| captive(r));
        let gw = s.spawn(|| {
            gateway
                .as_deref()
                .and_then(|g| ping(r, g, 10, None, Some(0.2), 6))
        });
        let net = s.spawn(|| {
            thread::scope(|t| {
                let tcps: Vec<_> = INTERNET_TCP
                    .iter()
                    .map(|a| t.spawn(move || (a.to_string(), r.tcp(a, 3000))))
                    .collect();
                let p = t.spawn(|| ping(r, "1.1.1.1", 10, None, Some(0.2), 6));
                let tcps: Vec<(String, Tcp)> = tcps
                    .into_iter()
                    .filter_map(|h| h.join().ok())
                    .map(|(a, res)| {
                        (
                            a,
                            match res {
                                Ok(ms) => Tcp::Ok(ms),
                                Err(e) => Tcp::Fail(e),
                            },
                        )
                    })
                    .collect();
                (tcps, p.join().ok().flatten())
            })
        });
        let dns = s.spawn(|| {
            thread::scope(|t| {
                let per: Vec<_> = servers
                    .iter()
                    .map(|sv| t.spawn(|| probe_resolver(r, sv, &fresh)))
                    .collect();
                let reference = t.spawn(|| probe_resolver(r, REFERENCE_DNS, &fresh));
                let system = t.spawn(|| {
                    let started = Instant::now();
                    match r.run(
                        "/usr/bin/dscacheutil",
                        &["-q", "host", "-a", "name", "example.com"],
                        6,
                    ) {
                        Ok((_, o, _)) if !o.trim().is_empty() => {
                            Ok(started.elapsed().as_secs_f64() * 1000.0)
                        }
                        _ => Err("example.com did not resolve".to_string()),
                    }
                });
                (
                    per.into_iter()
                        .filter_map(|h| h.join().ok())
                        .collect::<Vec<_>>(),
                    reference.join().ok(),
                    system.join().ok(),
                )
            })
        });
        let h4 = s.spawn(|| http(r, HTTPS_HOST, 4));
        let h6 = s.spawn(|| v6_wanted.then(|| http(r, HTTPS_HOST, 6)));
        let pm = s.spawn(|| (!opts.quick).then(|| path_mtu(r, link_mtu)));
        let v6 = s.spawn(|| {
            v6_wanted.then(|| match r.tcp("[2606:4700:4700::1111]:443", 3000) {
                Ok(ms) => Tcp::Ok(ms),
                Err(e) => Tcp::Fail(e),
            })
        });
        let px = s.spawn(|| {
            out(r, "/usr/sbin/scutil", &["--proxy"], 5)
                .map(|t| parse::proxy(&t))
                .unwrap_or_default()
        });
        let tg = s.spawn(|| opts.target.as_deref().map(|h| target_facts(r, h, &servers)));

        let (internet_tcp, internet_ping) = net.join().unwrap_or_default();
        let (dns_probes, dns_reference, dns_system) = dns.join().unwrap_or_default();
        Facts {
            link: link.clone(),
            route: route4.clone(),
            route6: route6.clone(),
            gateway: gateway.clone(),
            gateway_ping: gw.join().ok().flatten(),
            internet_tcp,
            internet_ping,
            captive: capt.join().ok(),
            dns_servers: servers.clone(),
            dns: dns_probes,
            dns_reference,
            dns_system,
            http4: h4.join().ok(),
            http6: h6.join().ok().flatten(),
            pmtu: pm.join().ok().flatten(),
            wifi: None,
            proxy: px.join().unwrap_or_default(),
            ipv6: Ipv6Fact {
                global_address: link.global_ipv6,
                default_route: route6.gateway.is_some() || route6.interface.is_some(),
                tcp: v6.join().ok().flatten(),
            },
            speed: None,
            target: tg.join().ok().flatten(),
            quick: opts.quick,
        }
    });
    // The Wi-Fi query scans for other networks, which takes the radio off-channel for a moment: measured
    // here, a 4 ms router latency became 40-60 ms with ~50 ms of jitter while it ran. So it runs only
    // after every latency probe has finished, never beside them.
    if wifi_wanted {
        facts.wifi = out(
            r,
            "/usr/sbin/system_profiler",
            &["SPAirPortDataType", "-json"],
            20,
        )
        .and_then(|t| parse::wifi(&t, &iface));
    }
    // The speed test saturates the link, so it runs alone after everything else has measured.
    if opts.speed {
        facts.speed = Some(match r.run("/usr/bin/networkQuality", &["-c"], 60) {
            Ok((_, o, _)) => parse::network_quality(&o),
            Err(e) => Err(e),
        });
    }
    facts
}
