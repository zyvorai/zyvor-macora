//! Scenario tests for `diagnose`. Each starts from `Facts::healthy_example()` and breaks one thing, then
//! asserts every step's status, the verdict and the advice. The last section replays the real collector
//! against a fake `Runner` that serves recorded tool output, so flags, parsing and analysis are tested
//! together without a broken network.
use std::sync::atomic::{AtomicU32, Ordering};
use zyvor_macora::{
    diagnose::{
        analyze::{analyze, Report},
        collect::{collect, Options, Runner},
        facts::*,
        render,
    },
    json, parse,
    probe::PingStats,
    view::Theme,
};

fn run(f: Facts) -> Report {
    analyze(&f)
}
fn st(r: &Report, name: &str) -> Status {
    r.steps
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| {
            panic!(
                "no {name} step in {:?}",
                r.steps.iter().map(|s| s.name).collect::<Vec<_>>()
            )
        })
        .status
}
fn has(r: &Report, name: &str) -> bool {
    r.steps.iter().any(|s| s.name == name)
}
fn detail<'a>(r: &'a Report, name: &str) -> &'a str {
    &r.steps.iter().find(|s| s.name == name).unwrap().detail
}
fn ping(avg: f64, sd: f64, loss: f64) -> Option<PingStats> {
    Some(PingStats {
        sent: 10,
        received: (10.0 * (100.0 - loss) / 100.0).round() as u32,
        loss_pct: loss,
        min_ms: Some(avg),
        avg_ms: Some(avg),
        max_ms: Some(avg),
        stddev_ms: Some(sd),
    })
}
fn dead(server: &str) -> DnsProbe {
    DnsProbe {
        server: server.into(),
        ok: false,
        detail: "timed out".into(),
        ..Default::default()
    }
}
fn probe(server: &str, ms: f64, fresh: f64) -> DnsProbe {
    DnsProbe {
        server: server.into(),
        ok: true,
        ms: Some(ms),
        fresh_ms: Some(fresh),
        detail: "answered".into(),
    }
}

// ---------- baseline ----------

#[test]
fn healthy_network_is_all_clear() {
    let r = run(Facts::healthy_example());
    for s in &r.steps {
        assert!(
            matches!(s.status, Status::Ok | Status::Info),
            "{} is {:?}",
            s.name,
            s.status
        );
    }
    for name in [
        "Link", "Route", "Wi-Fi", "Gateway", "Internet", "Portal", "DNS", "HTTPS", "MTU", "IPv6",
    ] {
        assert!(has(&r, name), "missing {name}");
    }
    assert!(r.verdict.starts_with("No problem found"), "{}", r.verdict);
    assert!(r.advice.is_empty() && r.also.is_empty());
    assert_eq!(r.worst(), Status::Ok);
    assert!(
        !has(&r, "Proxy") && !has(&r, "Speed") && !has(&r, "Site DNS"),
        "optional steps stay out"
    );
}

// ---------- link ----------

#[test]
fn no_adapter_fails_the_link_and_skips_everything_above() {
    let mut f = Facts::healthy_example();
    f.link = LinkFact::default();
    let r = run(f);
    assert_eq!(st(&r, "Link"), Status::Fail);
    for n in ["Gateway", "Internet", "DNS", "HTTPS", "MTU"] {
        assert_eq!(st(&r, n), Status::Skip, "{n}");
        assert!(detail(&r, n).contains("no working link"));
    }
    assert!(r.verdict.contains("No usable local connection"));
    assert!(
        r.advice.iter().any(|a| a.contains("Renew")),
        "{:?}",
        r.advice
    );
    assert_eq!(r.worst(), Status::Fail);
}

#[test]
fn adapter_without_link_fails() {
    let mut f = Facts::healthy_example();
    f.link.active = false;
    let r = run(f);
    assert_eq!(st(&r, "Link"), Status::Fail);
    assert!(detail(&r, "Link").contains("no link"));
}

#[test]
fn self_assigned_address_means_dhcp_did_not_answer() {
    let mut f = Facts::healthy_example();
    f.link.ipv4.clear();
    f.link.global_ipv6 = false;
    f.link.self_assigned = true;
    let r = run(f);
    assert_eq!(st(&r, "Link"), Status::Fail);
    assert!(detail(&r, "Link").contains("169.254") && detail(&r, "Link").contains("DHCP"));
}

#[test]
fn connected_without_any_address_fails() {
    let mut f = Facts::healthy_example();
    f.link.ipv4.clear();
    f.link.global_ipv6 = false;
    assert!(detail(&run(f), "Link").contains("no usable address"));
}

#[test]
fn ipv6_only_network_has_a_working_link() {
    // Regression: the link check used to require IPv4 and reported FAIL on IPv6-only networks.
    let mut f = Facts::healthy_example();
    f.link.ipv4.clear();
    let r = run(f);
    assert_eq!(st(&r, "Link"), Status::Ok);
    assert!(detail(&r, "Link").contains("global IPv6"));
}

// ---------- route / gateway ----------

#[test]
fn no_default_route_is_a_gateway_failure() {
    let mut f = Facts::healthy_example();
    f.gateway = None;
    f.gateway_ping = None;
    f.route = Route::default();
    f.route6 = Route::default();
    let r = run(f);
    assert_eq!(st(&r, "Gateway"), Status::Fail);
    assert!(detail(&r, "Gateway").contains("No default route"));
    assert!(r.verdict.contains("no route to the internet"));
    assert!(!has(&r, "Route"));
}

#[test]
fn default_route_through_a_tunnel_is_information_not_failure() {
    // Regression: a tunnel default route has no `gateway:` line and used to be reported as FAIL.
    let mut f = Facts::healthy_example();
    f.link.tunnel = Some("utun4".into());
    f.gateway = None;
    f.gateway_ping = None;
    let r = run(f);
    assert_eq!(st(&r, "Gateway"), Status::Info);
    assert_eq!(st(&r, "Route"), Status::Info);
    assert!(
        detail(&r, "Route").contains("utun4")
            && detail(&r, "Route").contains("does not identify the VPN")
    );
    assert_eq!(r.worst(), Status::Ok);
}

#[test]
fn router_that_ignores_ping_but_passes_traffic_is_only_a_warning() {
    // Regression: a router that drops ICMP used to fail the gateway and skip everything above it.
    let mut f = Facts::healthy_example();
    f.gateway_ping = ping(0.0, 0.0, 100.0).map(|mut p| {
        p.received = 0;
        p.avg_ms = None;
        p.stddev_ms = None;
        p
    });
    let r = run(f);
    assert_eq!(st(&r, "Gateway"), Status::Warn);
    assert!(detail(&r, "Gateway").contains("ignores ping but traffic flows"));
    assert_eq!(st(&r, "Internet"), Status::Ok, "later steps still run");
    assert_eq!(st(&r, "DNS"), Status::Ok);
    assert!(r.verdict.contains("does not answer ping"));
    assert_eq!(r.worst(), Status::Warn, "exit status stays 0");
}

fn all_internet_down(f: &mut Facts) {
    f.internet_tcp = f
        .internet_tcp
        .iter()
        .map(|(a, _)| (a.clone(), Tcp::Fail("timed out".into())))
        .collect();
    f.internet_ping = ping(0.0, 0.0, 100.0).map(|mut p| {
        p.received = 0;
        p
    });
    f.http4 = Some(Http {
        host: "www.apple.com".into(),
        family: 4,
        error: Some("timed out".into()),
        ..Default::default()
    });
    f.http6 = None;
}

#[test]
fn router_and_internet_both_down_blames_the_router() {
    let mut f = Facts::healthy_example();
    f.gateway_ping = Some(PingStats {
        sent: 10,
        received: 0,
        loss_pct: 100.0,
        ..Default::default()
    });
    all_internet_down(&mut f);
    let r = run(f);
    assert_eq!(st(&r, "Gateway"), Status::Fail);
    assert_eq!(st(&r, "Internet"), Status::Fail);
    assert!(
        r.verdict.contains("cannot reach its router"),
        "lowest layer wins: {}",
        r.verdict
    );
    assert!(
        r.also.iter().any(|a| a.contains("internet does not")),
        "{:?}",
        r.also
    );
}

#[test]
fn weak_wifi_is_named_in_the_router_advice() {
    let mut f = Facts::healthy_example();
    f.gateway_ping = Some(PingStats {
        sent: 10,
        received: 0,
        loss_pct: 100.0,
        ..Default::default()
    });
    f.wifi.as_mut().unwrap().rssi = Some(-80);
    all_internet_down(&mut f);
    let r = run(f);
    assert!(
        r.advice[0].contains("Wi-Fi signal is weak"),
        "{:?}",
        r.advice
    );
}

#[test]
fn gateway_loss_latency_and_jitter_each_warn() {
    let cases: [(Option<PingStats>, bool, &str); 6] = [
        (ping(4.0, 1.0, 10.0), true, "packet loss to the router"),
        (ping(80.0, 2.0, 0.0), true, "high latency to the router"),
        (
            ping(30.0, 2.0, 0.0),
            true,
            "Ethernet limit is 20 ms: handled below",
        ),
        (ping(4.0, 25.0, 0.0), true, "unsteady latency"),
        (ping(49.0, 5.0, 0.0), false, ""),
        (ping(4.0, 19.0, 0.0), false, ""),
    ];
    for (i, (p, warn, text)) in cases.into_iter().enumerate() {
        let mut f = Facts::healthy_example();
        f.gateway_ping = p;
        if i == 2 {
            // 30 ms is fine on Wi-Fi (limit 50) but too slow for a cable (limit 20).
            assert_eq!(st(&run(f.clone()), "Gateway"), Status::Ok);
            f.link.is_wifi = false;
            f.wifi = None;
        }
        let r = run(f);
        assert_eq!(
            st(&r, "Gateway") == Status::Warn,
            warn,
            "case {i}: {}",
            detail(&r, "Gateway")
        );
        if warn && i != 2 {
            assert!(r.verdict.contains(text), "case {i}: {}", r.verdict);
        }
    }
}

#[test]
fn gateway_that_could_not_be_pinged_is_information() {
    let mut f = Facts::healthy_example();
    f.gateway_ping = None;
    assert_eq!(st(&run(f), "Gateway"), Status::Info);
}

// ---------- internet ----------

#[test]
fn internet_down_with_a_working_router_blames_the_isp() {
    let mut f = Facts::healthy_example();
    all_internet_down(&mut f);
    let r = run(f);
    assert_eq!(st(&r, "Internet"), Status::Fail);
    assert!(r
        .verdict
        .contains("router answers but the internet does not"));
    assert!(r.advice.iter().any(|a| a.contains("modem")));
}

#[test]
fn blocked_https_with_working_ping_is_a_warning() {
    let mut f = Facts::healthy_example();
    f.internet_tcp = f
        .internet_tcp
        .iter()
        .map(|(a, _)| (a.clone(), Tcp::Fail("refused".into())))
        .collect();
    f.http4.as_mut().unwrap().ok = false;
    let r = run(f);
    assert_eq!(st(&r, "Internet"), Status::Warn);
    assert!(detail(&r, "Internet").contains("TCP port 443 is blocked"));
}

#[test]
fn one_blocked_probe_target_does_not_blame_the_isp() {
    // Regression: a single target (1.1.1.1) used to decide "internet down" on its own.
    let mut f = Facts::healthy_example();
    f.internet_tcp[0].1 = Tcp::Fail("refused".into());
    let r = run(f);
    assert_eq!(st(&r, "Internet"), Status::Ok);
    assert!(
        detail(&r, "Internet").contains("1.1.1.1 refused ✗"),
        "{}",
        detail(&r, "Internet")
    );
}

#[test]
fn internet_latency_loss_and_jitter_warn() {
    for (label, mutate) in [
        (
            "slow",
            Box::new(|f: &mut Facts| f.internet_tcp.iter_mut().for_each(|t| t.1 = Tcp::Ok(650.0)))
                as Box<dyn Fn(&mut Facts)>,
        ),
        (
            "loss",
            Box::new(|f: &mut Facts| f.internet_ping = ping(12.0, 1.0, 5.0)),
        ),
        (
            "jitter",
            Box::new(|f: &mut Facts| f.internet_ping = ping(12.0, 45.0, 0.0)),
        ),
    ] {
        let mut f = Facts::healthy_example();
        mutate(&mut f);
        let r = run(f);
        assert_eq!(st(&r, "Internet"), Status::Warn, "{label}");
        assert!(
            r.verdict.contains("internet path"),
            "{label}: {}",
            r.verdict
        );
    }
}

#[test]
fn internet_reachable_only_by_ping_and_http_counts_as_up() {
    let mut f = Facts::healthy_example();
    f.internet_tcp = f
        .internet_tcp
        .iter()
        .map(|(a, _)| (a.clone(), Tcp::Fail("timed out".into())))
        .collect();
    f.internet_ping = None;
    // https over IPv4 succeeded, so the internet is demonstrably up.
    f.gateway_ping = Some(PingStats {
        sent: 10,
        received: 0,
        loss_pct: 100.0,
        ..Default::default()
    });
    let r = run(f);
    assert_eq!(
        st(&r, "Gateway"),
        Status::Warn,
        "traffic flows, so the router is not blamed"
    );
}

// ---------- captive portal ----------

#[test]
fn captive_portal_outranks_every_other_finding() {
    let mut f = Facts::healthy_example();
    f.captive = Some(Captive::Intercepted("HTTP 302: Sign in".into()));
    f.pmtu.as_mut().unwrap().result = PmtuResult::Measured(1400);
    f.gateway_ping = ping(80.0, 2.0, 0.0);
    let r = run(f);
    assert_eq!(st(&r, "Portal"), Status::Fail);
    assert!(r.verdict.starts_with("A sign-in page"), "{}", r.verdict);
    assert!(r.advice[0].contains("http://"));
    assert!(
        r.also.len() >= 2,
        "other problems are still listed: {:?}",
        r.also
    );
}

#[test]
fn captive_check_that_could_not_run_is_information() {
    let mut f = Facts::healthy_example();
    f.captive = Some(Captive::Unknown("timed out".into()));
    assert_eq!(st(&run(f), "Portal"), Status::Info);
    let mut f = Facts::healthy_example();
    f.captive = None;
    assert!(!has(&run(f), "Portal"));
}

// ---------- DNS ----------

#[test]
fn dns_all_configured_servers_dead_but_reference_alive() {
    let mut f = Facts::healthy_example();
    f.dns = vec![dead("192.0.2.1")];
    f.dns_system = Some(Err("example.com did not resolve".into()));
    let r = run(f);
    assert_eq!(st(&r, "DNS"), Status::Fail);
    assert!(r.verdict.contains("names do not resolve"));
    assert!(r.advice[0].contains("1.1.1.1"));
}

#[test]
fn dns_servers_dead_but_system_resolver_works_is_a_warning() {
    let mut f = Facts::healthy_example();
    f.dns = vec![dead("192.0.2.1")];
    let r = run(f);
    assert_eq!(st(&r, "DNS"), Status::Warn);
    assert!(r
        .verdict
        .contains("None of your configured DNS servers answer direct queries"));
    assert!(r.advice.iter().any(|a| a.contains("1.1.1.1 does answer")));
}

#[test]
fn dns_everything_dead_is_a_failure_with_no_reference() {
    let mut f = Facts::healthy_example();
    f.dns = vec![dead("192.0.2.1")];
    f.dns_reference = Some(dead("1.1.1.1"));
    f.dns_system = None;
    let r = run(f);
    assert_eq!(st(&r, "DNS"), Status::Fail);
    assert!(r.verdict.contains("None of your DNS servers answer"));
    assert!(!r.advice.iter().any(|a| a.contains("does answer")));
}

#[test]
fn one_dead_resolver_among_several_is_named() {
    let mut f = Facts::healthy_example();
    f.dns_servers.push("192.0.2.2".into());
    f.dns.push(dead("192.0.2.2"));
    let r = run(f);
    assert_eq!(st(&r, "DNS"), Status::Warn);
    assert!(
        r.verdict.contains("192.0.2.2 is not answering"),
        "{}",
        r.verdict
    );
}

#[test]
fn slow_dns_is_detected_directly_and_against_the_reference() {
    let mut f = Facts::healthy_example();
    f.dns = vec![probe("192.0.2.1", 30.0, 900.0)];
    let r = run(f);
    assert_eq!(st(&r, "DNS"), Status::Warn);
    assert!(
        r.verdict.contains("DNS is slow") && r.advice.iter().any(|a| a.contains("For comparison"))
    );
    // Not slow in absolute terms, but far slower than the public reference.
    let mut f = Facts::healthy_example();
    f.dns = vec![probe("192.0.2.1", 20.0, 500.0)];
    f.dns_reference = Some(probe("1.1.1.1", 10.0, 20.0));
    assert_eq!(st(&run(f), "DNS"), Status::Warn);
    // A slow lookup the reference shares is not blamed on the resolver.
    let mut f = Facts::healthy_example();
    f.dns = vec![probe("192.0.2.1", 20.0, 260.0)];
    f.dns_reference = Some(probe("1.1.1.1", 20.0, 250.0));
    assert_eq!(st(&run(f), "DNS"), Status::Ok);
}

#[test]
fn system_resolver_slow_or_failing() {
    let mut f = Facts::healthy_example();
    f.dns_system = Some(Ok(450.0));
    assert_eq!(st(&run(f), "DNS"), Status::Warn);
    let mut f = Facts::healthy_example();
    f.dns_system = Some(Err("example.com did not resolve".into()));
    assert_eq!(st(&run(f), "DNS"), Status::Fail);
}

#[test]
fn no_configured_servers_is_fine_when_the_system_resolver_works() {
    let mut f = Facts::healthy_example();
    f.dns_servers.clear();
    f.dns.clear();
    assert_eq!(st(&run(f.clone()), "DNS"), Status::Ok);
    f.dns_system = Some(Err("nope".into()));
    assert_eq!(st(&run(f), "DNS"), Status::Fail);
}

// ---------- HTTPS ----------

fn http(dns: f64, connect: f64, tls: f64, ttfb: f64, total: f64) -> Http {
    Http {
        host: "www.apple.com".into(),
        family: 4,
        ok: true,
        dns_ms: dns,
        connect_ms: connect,
        tls_ms: tls,
        ttfb_ms: ttfb,
        total_ms: total,
        code: 200,
        ..Default::default()
    }
}

#[test]
fn https_names_the_slow_stage() {
    for (h, stage, tip) in [
        (
            http(400.0, 420.0, 440.0, 460.0, 500.0),
            "DNS lookup",
            "DNS server",
        ),
        (
            http(10.0, 520.0, 540.0, 560.0, 600.0),
            "TCP connect",
            "slow or lossy",
        ),
        (
            http(10.0, 20.0, 720.0, 740.0, 800.0),
            "TLS handshake",
            "inspecting HTTPS",
        ),
        (
            http(10.0, 20.0, 30.0, 1500.0, 1600.0),
            "server response",
            "site itself",
        ),
    ] {
        let mut f = Facts::healthy_example();
        f.http4 = Some(h);
        let r = run(f);
        assert_eq!(st(&r, "HTTPS"), Status::Warn, "{stage}");
        assert!(r.verdict.contains(stage), "{stage}: {}", r.verdict);
        assert!(r.advice[0].contains(tip), "{stage}: {:?}", r.advice);
    }
}

#[test]
fn https_failure_is_reported_with_the_reason() {
    let mut f = Facts::healthy_example();
    f.http4 = Some(Http {
        host: "www.apple.com".into(),
        family: 4,
        ok: false,
        error: Some("SSL certificate problem".into()),
        ..Default::default()
    });
    let r = run(f);
    assert_eq!(st(&r, "HTTPS"), Status::Warn);
    assert!(r.verdict.contains("SSL certificate problem"));
}

#[test]
fn https_over_ipv6_failing_is_shown_next_to_the_ipv4_result() {
    let mut f = Facts::healthy_example();
    f.http6 = Some(Http {
        host: "www.apple.com".into(),
        family: 6,
        error: Some("timed out".into()),
        ..Default::default()
    });
    f.ipv6.tcp = Some(Tcp::Fail("timed out".into()));
    let r = run(f);
    assert!(detail(&r, "HTTPS").contains("over IPv6 ✗ timed out"));
    assert_eq!(st(&r, "IPv6"), Status::Warn);
    assert!(r.verdict.contains("IPv6 is configured but does not work"));
}

// ---------- path MTU ----------

#[test]
fn path_mtu_below_the_link_size_warns_with_a_number_and_a_fix() {
    // Regression: the old check only pinged the router, so a 1480-byte path said "OK".
    for mtu in [1492u32, 1480, 1400] {
        let mut f = Facts::healthy_example();
        f.pmtu.as_mut().unwrap().result = PmtuResult::Measured(mtu);
        let r = run(f);
        assert_eq!(st(&r, "MTU"), Status::Warn, "{mtu}");
        assert!(
            r.verdict.contains(&format!("larger than {mtu} bytes")),
            "{}",
            r.verdict
        );
        assert!(
            r.advice[0].contains(&format!("MTU to {mtu}")),
            "{:?}",
            r.advice
        );
        assert!(detail(&r, "MTU").contains(&mtu.to_string()));
    }
    let mut f = Facts::healthy_example();
    f.pmtu.as_mut().unwrap().result = PmtuResult::Measured(1492);
    assert!(detail(&run(f), "MTU").contains("PPPoE"));
}

#[test]
fn path_mtu_equal_to_a_smaller_link_is_fine() {
    let mut f = Facts::healthy_example();
    f.pmtu = Some(Pmtu {
        target: "1.1.1.1".into(),
        link_mtu: 1400,
        result: PmtuResult::Measured(1400),
    });
    assert_eq!(st(&run(f), "MTU"), Status::Ok);
}

#[test]
fn unmeasurable_path_mtu_is_information_only() {
    let mut f = Facts::healthy_example();
    f.pmtu.as_mut().unwrap().result = PmtuResult::Unmeasurable("ICMP may be blocked".into());
    let r = run(f);
    assert_eq!(st(&r, "MTU"), Status::Info);
    assert_eq!(r.worst(), Status::Ok);
}

// ---------- Wi-Fi ----------

#[test]
fn wifi_signal_grading() {
    let cases: [(i32, i32, Status); 7] = [
        (-55, -94, Status::Ok),
        (-70, -94, Status::Ok),
        (-76, -94, Status::Warn),
        (-65, -80, Status::Warn),
        (-88, -94, Status::Fail),
        (-74, -94, Status::Ok),   // SNR 20 is the lowest that passes
        (-75, -94, Status::Warn), // SNR 19
    ];
    for (rssi, noise, want) in cases {
        let mut f = Facts::healthy_example();
        let w = f.wifi.as_mut().unwrap();
        w.rssi = Some(rssi);
        w.noise = Some(noise);
        let r = run(f);
        assert_eq!(
            st(&r, "Wi-Fi"),
            want,
            "rssi {rssi} noise {noise}: {}",
            detail(&r, "Wi-Fi")
        );
    }
}

#[test]
fn wifi_detail_reads_like_a_sentence() {
    let r = run(Facts::healthy_example());
    let d = detail(&r, "Wi-Fi");
    for part in [
        "5GHz ch 40 (80MHz)",
        "802.11ax",
        "signal -55 dBm",
        "noise -94 dBm",
        "SNR 39 dB",
        "866 Mbps",
    ] {
        assert!(d.contains(part), "{part} missing from {d}");
    }
}

#[test]
fn wifi_24ghz_is_called_out_but_not_a_problem() {
    let mut f = Facts::healthy_example();
    f.wifi.as_mut().unwrap().band = Some("2.4GHz".into());
    let r = run(f);
    assert_eq!(st(&r, "Wi-Fi"), Status::Info);
    assert!(detail(&r, "Wi-Fi").contains("slower and more crowded"));
    assert_eq!(r.worst(), Status::Ok);
}

#[test]
fn wifi_unknown_not_joined_and_non_wifi_links() {
    let mut f = Facts::healthy_example();
    f.wifi = None;
    assert_eq!(st(&run(f), "Wi-Fi"), Status::Info);
    let mut f = Facts::healthy_example();
    f.wifi = Some(Wifi::default());
    assert!(detail(&run(f), "Wi-Fi").contains("not joined"));
    let mut f = Facts::healthy_example();
    f.link.is_wifi = false;
    f.link.label = "Ethernet".into();
    f.wifi = None;
    assert!(!has(&run(f), "Wi-Fi"), "no Wi-Fi step on a cable");
    // Missing signal figures degrade gracefully.
    let mut f = Facts::healthy_example();
    let w = f.wifi.as_mut().unwrap();
    w.rssi = None;
    w.noise = None;
    assert_eq!(st(&run(f), "Wi-Fi"), Status::Ok);
}

// ---------- IPv6, proxy, speed ----------

#[test]
fn ipv6_states() {
    let mut f = Facts::healthy_example();
    f.ipv6 = Ipv6Fact {
        global_address: false,
        default_route: false,
        tcp: None,
    };
    let r = run(f);
    assert_eq!(st(&r, "IPv6"), Status::Info);
    assert!(detail(&r, "IPv6").contains("IPv4-only network is fine"));
    let mut f = Facts::healthy_example();
    f.ipv6 = Ipv6Fact {
        global_address: true,
        default_route: false,
        tcp: None,
    };
    let r = run(f);
    assert_eq!(st(&r, "IPv6"), Status::Warn);
    assert!(r.verdict.contains("half-configured"));
    let mut f = Facts::healthy_example();
    f.ipv6.tcp = None;
    assert_eq!(st(&run(f), "IPv6"), Status::Info);
}

#[test]
fn proxy_is_listed_only_when_configured() {
    let mut f = Facts::healthy_example();
    f.proxy = Proxy {
        http: Some("proxy.example.test:8080".into()),
        pac: Some("http://pac.example.test/p.pac".into()),
        auto_discovery: true,
        ..Default::default()
    };
    let r = run(f);
    assert_eq!(st(&r, "Proxy"), Status::Info);
    let d = detail(&r, "Proxy");
    assert!(
        d.contains("HTTP proxy proxy.example.test:8080")
            && d.contains("auto-config script")
            && d.contains("automatic proxy discovery")
    );
    assert_eq!(r.worst(), Status::Ok);
}

#[test]
fn speed_results_are_classified() {
    let speed = |down: f64, rpm: f64| {
        Some(Ok(Speed {
            down_mbps: down,
            up_mbps: 40.0,
            rpm: Some(rpm),
            base_rtt_ms: Some(20.0),
        }))
    };
    let mut f = Facts::healthy_example();
    f.speed = speed(95.0, 228.0);
    let r = run(f);
    assert_eq!(st(&r, "Speed"), Status::Ok);
    assert!(
        detail(&r, "Speed").contains("down 95 Mbps") && detail(&r, "Speed").contains("(medium)")
    );
    let mut f = Facts::healthy_example();
    f.speed = speed(95.0, 900.0);
    assert!(detail(&run(f), "Speed").contains("(high)"));
    let mut f = Facts::healthy_example();
    f.speed = speed(95.0, 120.0);
    let r = run(f);
    assert_eq!(st(&r, "Speed"), Status::Warn);
    assert!(r.verdict.contains("bufferbloat"));
    let mut f = Facts::healthy_example();
    f.speed = speed(2.5, 500.0);
    assert!(run(f).verdict.contains("very low"));
    let mut f = Facts::healthy_example();
    f.speed = Some(Err("not supported".into()));
    let r = run(f);
    assert_eq!(st(&r, "Speed"), Status::Info);
    assert_eq!(r.worst(), Status::Ok);
}

// ---------- a specific site ----------

fn target_facts() -> TargetFacts {
    TargetFacts {
        host: "shop.example.test".into(),
        dns: vec![probe("192.0.2.1", 12.0, 0.0)],
        dns_reference: Some(probe("1.1.1.1", 14.0, 0.0)),
        http: Some(http(12.0, 22.0, 36.0, 70.0, 80.0)),
        ping: ping(9.0, 1.0, 0.0),
        pmtu: Some(Pmtu {
            target: "198.51.100.9".into(),
            link_mtu: 1500,
            result: PmtuResult::Measured(1500),
        }),
    }
}

#[test]
fn healthy_site_adds_three_steps_and_names_the_site_in_the_verdict() {
    let mut f = Facts::healthy_example();
    f.target = Some(target_facts());
    let r = run(f);
    for n in ["Site DNS", "Site HTTPS", "Site path"] {
        assert_eq!(st(&r, n), Status::Ok, "{n}");
    }
    assert!(
        r.verdict.contains("shop.example.test looks fine"),
        "{}",
        r.verdict
    );
    assert_eq!(r.target.as_deref(), Some("shop.example.test"));
}

#[test]
fn site_dns_failures() {
    let mut f = Facts::healthy_example();
    let mut t = target_facts();
    t.dns = vec![dead("192.0.2.1")];
    f.target = Some(t.clone());
    let r = run(f);
    assert_eq!(st(&r, "Site DNS"), Status::Fail);
    assert!(
        r.verdict
            .contains("cannot resolve shop.example.test but 1.1.1.1 can"),
        "{}",
        r.verdict
    );
    t.dns_reference = Some(dead("1.1.1.1"));
    let mut f = Facts::healthy_example();
    f.target = Some(t);
    assert!(run(f).verdict.contains("does not resolve anywhere"));
}

#[test]
fn site_https_and_path_problems() {
    let mut f = Facts::healthy_example();
    let mut t = target_facts();
    t.http = Some(http(10.0, 20.0, 30.0, 2500.0, 2600.0));
    f.target = Some(t);
    let r = run(f);
    assert_eq!(st(&r, "Site HTTPS"), Status::Warn);
    assert!(
        r.verdict
            .contains("shop.example.test is slow in the server response"),
        "{}",
        r.verdict
    );
    let mut f = Facts::healthy_example();
    let mut t = target_facts();
    t.http = Some(Http {
        host: t.host.clone(),
        error: Some("timed out".into()),
        ..Default::default()
    });
    f.target = Some(t);
    let r = run(f);
    assert_eq!(st(&r, "Site HTTPS"), Status::Fail);
    assert!(r.verdict.contains("Cannot open https://shop.example.test"));
    let mut f = Facts::healthy_example();
    let mut t = target_facts();
    t.ping = ping(80.0, 60.0, 10.0);
    t.pmtu = Some(Pmtu {
        target: "198.51.100.9".into(),
        link_mtu: 1500,
        result: PmtuResult::Measured(1400),
    });
    f.target = Some(t);
    let r = run(f);
    assert_eq!(st(&r, "Site path"), Status::Warn);
    assert!(detail(&r, "Site path").contains("path MTU 1400"));
}

// ---------- quick mode and verdict ranking ----------

#[test]
fn quick_mode_marks_skipped_checks_as_skipped_not_unavailable() {
    let mut f = Facts::healthy_example();
    f.quick = true;
    f.wifi = None;
    f.pmtu = None;
    let r = run(f);
    assert_eq!(st(&r, "Wi-Fi"), Status::Skip);
    assert_eq!(st(&r, "MTU"), Status::Skip);
    assert!(detail(&r, "Wi-Fi").contains("quick mode"));
    assert_eq!(r.worst(), Status::Ok, "skips are not problems");
}

#[test]
fn failures_outrank_warnings_and_lower_layers_outrank_higher_ones() {
    // A DNS failure (Fail, rank 4) beats a gateway warning (Warn, rank 2).
    let mut f = Facts::healthy_example();
    f.gateway_ping = ping(80.0, 2.0, 0.0);
    f.dns = vec![dead("192.0.2.1")];
    f.dns_system = Some(Err("example.com did not resolve".into()));
    let r = run(f);
    assert!(r.verdict.contains("names do not resolve"), "{}", r.verdict);
    assert!(r
        .also
        .iter()
        .any(|a| a.contains("high latency to the router")));
    // Two warnings: the lowest layer (router) is named first.
    let mut f = Facts::healthy_example();
    f.gateway_ping = ping(80.0, 2.0, 0.0);
    f.pmtu.as_mut().unwrap().result = PmtuResult::Measured(1400);
    let r = run(f);
    assert!(r.verdict.contains("router"), "{}", r.verdict);
    assert!(r.also.iter().any(|a| a.contains("1400")));
}

#[test]
fn advice_is_short_and_deterministic() {
    let mut f = Facts::healthy_example();
    f.link = LinkFact::default();
    let a = run(f.clone());
    let b = run(f);
    assert_eq!((a.verdict.clone(), a.advice.clone()), (b.verdict, b.advice));
    assert!(a.advice.len() <= 3);
    assert!(!a.verdict.is_empty());
}

#[test]
fn secrets_cover_the_local_network_but_not_public_probe_targets() {
    let mut f = Facts::healthy_example();
    f.dns_servers = vec!["192.0.2.1".into(), "1.1.1.1".into(), "fe80::1%en0".into()];
    let s = f.secrets();
    for want in ["192.0.2.1", "192.0.2.8", "fe80::1%en0", "fe80::1"] {
        assert!(s.iter().any(|x| x == want), "{want} missing from {s:?}");
    }
    for public in ["1.1.1.1", "8.8.8.8", "9.9.9.9"] {
        assert!(
            !s.iter().any(|x| x == public),
            "{public} must stay readable"
        );
    }
    let mut sorted = s.clone();
    sorted.sort_by_key(|x| std::cmp::Reverse(x.len()));
    assert_eq!(s, sorted, "longest first");
    assert_eq!(s.len(), {
        let mut d = s.clone();
        d.dedup();
        d.len()
    });
}

// ---------- renderers ----------

fn varied() -> Report {
    let mut f = Facts::healthy_example();
    f.pmtu.as_mut().unwrap().result = PmtuResult::Measured(1480);
    f.gateway_ping = ping(80.0, 2.0, 0.0);
    run(f)
}

#[test]
fn json_is_valid_and_keeps_the_old_keys() {
    let r = varied();
    let v = json::parse(&render::json(&r)).expect("valid JSON");
    for k in ["steps", "verdict", "advice", "also", "target"] {
        assert!(v.get(k).is_some(), "missing {k}");
    }
    let steps = v.get("steps").unwrap().as_array().unwrap();
    assert_eq!(steps.len(), r.steps.len());
    let first = &steps[0];
    for k in ["step", "status", "detail", "ms"] {
        assert!(first.get(k).is_some(), "step key {k}");
    }
    let gw = steps
        .iter()
        .find(|s| s.get("step").unwrap().as_str() == Some("Gateway"))
        .unwrap();
    assert!((gw.get("ms").unwrap().as_f64().unwrap() - 80.0).abs() < 0.01);
    assert_eq!(first.get("ms"), Some(&json::Value::Null));
    assert_eq!(v.get("target"), Some(&json::Value::Null));
}

#[test]
fn json_escapes_hostile_text() {
    let mut f = Facts::healthy_example();
    f.link.label = "Wi\"Fi\\\n<x>".into();
    let r = run(f);
    let v = json::parse(&render::json(&r)).expect("still valid JSON");
    assert!(v.get("steps").is_some());
}

#[test]
fn text_markdown_and_html_shapes() {
    let r = varied();
    let text = render::text(&r, &Theme { color: false });
    assert!(
        text.contains("VERDICT") && text.contains("WHAT TO TRY") && text.contains("ALSO NOTICED")
    );
    assert!(!text.contains('\x1b'), "no escape codes when color is off");
    assert!(render::text(&r, &Theme { color: true }).contains('\x1b'));
    let md = render::markdown(&r);
    assert!(md.starts_with("# MACORA network diagnosis"));
    assert_eq!(
        md.lines()
            .filter(|l| l.starts_with("| ") && !l.starts_with("| Result"))
            .count(),
        r.steps.len()
    );
    assert!(md.contains("**Verdict:**") && md.contains("## What to try"));
    let html = render::html(&r);
    assert!(
        html.starts_with("<!doctype html>")
            && html.contains("prefers-color-scheme")
            && html.contains("<table>")
    );
    assert_eq!(html.matches("<tr>").count(), r.steps.len());
}

#[test]
fn html_and_markdown_neutralize_hostile_text() {
    let mut f = Facts::healthy_example();
    f.link.label = "<script>alert(1)</script>|x".into();
    let r = run(f);
    let html = render::html(&r);
    assert!(!html.contains("<script"), "{html}");
    assert!(html.contains("&lt;script&gt;"));
    let md = render::markdown(&r);
    let link_row = md.lines().find(|l| l.contains("| Link |")).unwrap();
    assert!(
        link_row.contains("\\|x"),
        "a pipe in a cell must be escaped: {link_row}"
    );
}

#[test]
fn redact_masks_the_local_network_in_every_format() {
    let r = varied();
    for (name, out) in [
        ("text", render::text(&r, &Theme { color: false })),
        ("json", render::json(&r)),
        ("markdown", render::markdown(&r)),
        ("html", render::html(&r)),
    ] {
        let masked = render::redact(out.clone(), &r.secrets);
        for secret in ["192.0.2.1", "192.0.2.8", "fe80::1%en0", "fe80::1"] {
            assert!(
                out.contains(secret) || secret.starts_with("fe80"),
                "{name} should have contained {secret} before masking"
            );
            assert!(
                !masked.contains(secret),
                "{name} still contains {secret}:\n{masked}"
            );
        }
        assert!(masked.contains("<IPv4>"), "{name}");
        assert!(
            masked.contains("1.1.1.1"),
            "{name}: public targets stay readable"
        );
        assert_eq!(
            render::redact(masked.clone(), &r.secrets),
            masked,
            "{name}: idempotent"
        );
    }
    if let Ok(v) = json::parse(&render::redact(render::json(&r), &r.secrets)) {
        assert!(v.get("steps").is_some(), "redacted JSON stays valid");
    } else {
        panic!("redacted JSON must still parse");
    }
}

// ---------- the real collector against a fake runner ----------

fn fx(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/diagnose/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

struct Fake {
    mtu: u32,
    gateway_ignores_ping: bool,
    internet_up: bool,
    dead_dns: Vec<&'static str>,
    captive_body: &'static str,
    v4_route: &'static str,
    calls: AtomicU32,
}
impl Fake {
    fn healthy() -> Fake {
        Fake {
            mtu: 1500,
            gateway_ignores_ping: false,
            internet_up: true,
            dead_dns: vec![],
            captive_body:
                "<HTML><HEAD><TITLE>Success</TITLE></HEAD><BODY>Success</BODY></HTML>\n200",
            v4_route: "route-v4.txt",
            calls: AtomicU32::new(0),
        }
    }
}
fn ping_text(sent: u32, received: u32, avg: f64) -> String {
    if received == 0 {
        format!("--- x ping statistics ---\n{sent} packets transmitted, 0 packets received, 100.0% packet loss\n")
    } else {
        format!(
            "--- x ping statistics ---\n{sent} packets transmitted, {received} packets received, {:.1}% packet loss\nround-trip min/avg/max/stddev = {avg}/{avg}/{avg}/0.4 ms\n",
            100.0 * (sent - received) as f64 / sent as f64
        )
    }
}
impl Runner for Fake {
    fn run(
        &self,
        program: &str,
        args: &[&str],
        _secs: u64,
    ) -> Result<(bool, String, String), String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let ok = |s: String| Ok((true, s, String::new()));
        match program {
            "/sbin/route" => ok(fx(if args.contains(&"-inet6") {
                "route-v6.txt"
            } else {
                self.v4_route
            })),
            "/usr/sbin/scutil" => ok(fx(if args.contains(&"--dns") {
                "scutil-dns.txt"
            } else {
                "scutil-proxy-none.txt"
            })),
            "/sbin/ping" | "/sbin/ping6" => {
                let host = *args.last().unwrap();
                let count: u32 = args
                    .iter()
                    .position(|a| *a == "-c")
                    .and_then(|i| args[i + 1].parse().ok())
                    .unwrap_or(1);
                let size = args
                    .iter()
                    .position(|a| *a == "-s")
                    .and_then(|i| args[i + 1].parse::<u32>().ok());
                let is_gw = host == "192.0.2.1" || host.starts_with("fe80");
                if is_gw && self.gateway_ignores_ping {
                    return ok(ping_text(count, 0, 0.0));
                }
                if !is_gw && !self.internet_up {
                    return ok(ping_text(count, 0, 0.0));
                }
                if let Some(payload) = size {
                    if payload + 28 > self.mtu && !args.contains(&"ping6") {
                        return ok(ping_text(count, 0, 0.0));
                    }
                }
                ok(ping_text(count, count, if is_gw { 4.0 } else { 12.0 }))
            }
            "/usr/bin/dig" => {
                let server = args.iter().find_map(|a| a.strip_prefix('@')).unwrap_or("");
                if self.dead_dns.contains(&server) {
                    return ok(fx("dig-timeout.txt"));
                }
                let name = *args.last().unwrap();
                ok(fx(if name.starts_with("macora-") {
                    "dig-nxdomain.txt"
                } else {
                    "dig-noerror.txt"
                }))
            }
            "/usr/bin/curl" => {
                if args.iter().any(|a| a.contains("captive.apple.com")) {
                    return ok(self.captive_body.to_string());
                }
                ok(fx(if self.internet_up {
                    "curl-ok.txt"
                } else {
                    "curl-fail.txt"
                }))
            }
            "/usr/sbin/system_profiler" => ok(fx("wifi-5ghz.json")),
            "/usr/bin/dscacheutil" => ok("name: example.com\nip_address: 198.51.100.9\n".into()),
            "/usr/bin/networkQuality" => ok(fx("networkquality.json")),
            other => Err(format!("not faked: {other}")),
        }
    }
    fn tcp(&self, _addr: &str, _timeout_ms: u64) -> Result<f64, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.internet_up {
            Ok(10.0)
        } else {
            Err("timed out".into())
        }
    }
}

fn snapshot() -> zyvor_macora::model::Snapshot {
    let mut s = parse::ifconfig(include_str!("fixtures/macos.ifconfig")).unwrap();
    for i in &mut s.interfaces {
        if i.name == "en0" {
            i.hardware = Some("Wi-Fi".into());
        }
    }
    s
}
fn e2e(fake: &Fake, opts: &Options) -> (Facts, Report) {
    let f = collect(fake, &snapshot(), opts);
    let r = analyze(&f);
    (f, r)
}

#[test]
fn collector_healthy_end_to_end() {
    let fake = Fake::healthy();
    let (f, r) = e2e(&fake, &Options::default());
    assert_eq!(r.worst(), Status::Ok, "{:#?}", r.steps);
    assert_eq!(f.link.iface.as_deref(), Some("en0"));
    assert!(f.link.is_wifi && f.link.global_ipv6 && f.link.tunnel.is_none());
    assert_eq!(f.gateway.as_deref(), Some("192.0.2.1"));
    assert_eq!(f.dns_servers, ["fe80::1%en0", "192.0.2.1", "198.51.100.53"]);
    assert_eq!(f.dns.len(), 3, "every configured resolver is probed");
    assert_eq!(f.internet_tcp.len(), 3);
    assert_eq!(f.pmtu.as_ref().unwrap().result, PmtuResult::Measured(1500));
    assert_eq!(f.wifi.as_ref().unwrap().snr(), Some(28));
    assert!(f.http4.as_ref().unwrap().ok && f.http6.as_ref().unwrap().ok);
    assert_eq!(f.captive, Some(Captive::Clear));
    assert!(!f.proxy.any());
    assert!(
        fake.calls.load(Ordering::SeqCst) < 150,
        "bounded work: {}",
        fake.calls.load(Ordering::SeqCst)
    );
}

#[test]
fn collector_measures_the_path_mtu_through_df_pings() {
    for mtu in [1500u32, 1492, 1480, 1454, 1400, 1280] {
        let fake = Fake {
            mtu,
            ..Fake::healthy()
        };
        let (f, r) = e2e(&fake, &Options::default());
        assert_eq!(
            f.pmtu.as_ref().unwrap().result,
            PmtuResult::Measured(mtu),
            "mtu {mtu}"
        );
        assert_eq!(
            st(&r, "MTU"),
            if mtu == 1500 {
                Status::Ok
            } else {
                Status::Warn
            },
            "mtu {mtu}"
        );
    }
}

#[test]
fn collector_detects_a_captive_portal() {
    let fake = Fake {
        captive_body: "<HTML><BODY>Please log in to Hotel WiFi</BODY></HTML>\n200",
        ..Fake::healthy()
    };
    let (f, r) = e2e(&fake, &Options::default());
    assert!(
        matches!(f.captive, Some(Captive::Intercepted(ref s)) if s.contains("Please log in")),
        "{:?}",
        f.captive
    );
    assert_eq!(st(&r, "Portal"), Status::Fail);
    let fake = Fake {
        captive_body: "\n302",
        ..Fake::healthy()
    };
    assert!(
        matches!(collect(&fake, &snapshot(), &Options { quick: true, ..Default::default() }).captive, Some(Captive::Intercepted(ref s)) if s.contains("302"))
    );
    let fake = Fake {
        captive_body: "\n000",
        ..Fake::healthy()
    };
    assert!(matches!(
        collect(
            &fake,
            &snapshot(),
            &Options {
                quick: true,
                ..Default::default()
            }
        )
        .captive,
        Some(Captive::Unknown(_))
    ));
}

#[test]
fn collector_flags_dead_resolvers() {
    let fake = Fake {
        dead_dns: vec!["192.0.2.1"],
        ..Fake::healthy()
    };
    let (f, r) = e2e(&fake, &Options::default());
    assert!(
        f.dns
            .iter()
            .any(|p| p.server == "192.0.2.1" && !p.ok && p.detail == "timed out"),
        "{:?}",
        f.dns
    );
    assert_eq!(st(&r, "DNS"), Status::Warn);
    assert!(
        r.verdict.contains("192.0.2.1 is not answering"),
        "{}",
        r.verdict
    );
}

#[test]
fn collector_router_that_ignores_ping_is_not_a_failure() {
    let fake = Fake {
        gateway_ignores_ping: true,
        ..Fake::healthy()
    };
    let (_, r) = e2e(&fake, &Options::default());
    assert_eq!(st(&r, "Gateway"), Status::Warn);
    assert_eq!(st(&r, "Internet"), Status::Ok);
    assert_ne!(r.worst(), Status::Fail);
}

#[test]
fn collector_internet_down() {
    let fake = Fake {
        internet_up: false,
        ..Fake::healthy()
    };
    let (_, r) = e2e(&fake, &Options::default());
    assert_eq!(st(&r, "Internet"), Status::Fail);
    assert!(
        r.verdict
            .contains("router answers but the internet does not"),
        "{}",
        r.verdict
    );
}

#[test]
fn collector_tunnel_default_route() {
    // en0 is physical; utun0 (a tunnel in the fixture) carries the default route.
    let fake = Fake {
        v4_route: "route-tunnel.txt",
        ..Fake::healthy()
    };
    // Re-point the fixture at an interface that exists in the capture.
    struct Wrap(Fake);
    impl Runner for Wrap {
        fn run(&self, p: &str, a: &[&str], s: u64) -> Result<(bool, String, String), String> {
            self.0
                .run(p, a, s)
                .map(|(o, out, e)| (o, out.replace("utun4", "utun0"), e))
        }
        fn tcp(&self, a: &str, t: u64) -> Result<f64, String> {
            self.0.tcp(a, t)
        }
    }
    let f = collect(&Wrap(fake), &snapshot(), &Options::default());
    assert_eq!(f.link.tunnel.as_deref(), Some("utun0"));
    assert_eq!(
        f.link.iface.as_deref(),
        Some("en0"),
        "the link check looks at the physical adapter under the tunnel"
    );
    assert_eq!(f.route.mtu, Some(1380));
    let r = analyze(&f);
    assert_eq!(st(&r, "Route"), Status::Info);
    assert!(detail(&r, "Route").contains("utun0"));
}

#[test]
fn collector_ipv6_only_pings_the_v6_router() {
    let mut s = snapshot();
    for i in &mut s.interfaces {
        if i.name == "en0" {
            i.addresses.retain(|a| a.ip.contains(':'));
        }
    }
    let fake = Fake {
        v4_route: "route-none.txt",
        ..Fake::healthy()
    };
    let f = collect(&fake, &s, &Options::default());
    assert_eq!(f.gateway.as_deref(), Some("fe80::1%en0"));
    let r = analyze(&f);
    assert_eq!(st(&r, "Link"), Status::Ok);
    assert_eq!(st(&r, "Gateway"), Status::Ok, "{}", detail(&r, "Gateway"));
}

#[test]
fn collector_quick_skips_the_slow_checks() {
    let fake = Fake::healthy();
    let (f, r) = e2e(
        &fake,
        &Options {
            quick: true,
            ..Default::default()
        },
    );
    assert!(f.wifi.is_none() && f.pmtu.is_none() && f.quick);
    assert_eq!(
        (st(&r, "Wi-Fi"), st(&r, "MTU")),
        (Status::Skip, Status::Skip)
    );
}

#[test]
fn collector_site_and_speed_options() {
    let fake = Fake::healthy();
    let (f, r) = e2e(
        &fake,
        &Options {
            speed: true,
            target: Some("shop.example.test".into()),
            quick: false,
        },
    );
    let t = f.target.as_ref().expect("target facts");
    assert_eq!(t.host, "shop.example.test");
    assert_eq!(t.dns.len(), 3);
    assert!(t.http.as_ref().unwrap().ok && t.ping.is_some());
    assert!(
        matches!(t.pmtu.as_ref().unwrap().result, PmtuResult::Measured(1500)),
        "{:?}",
        t.pmtu
    );
    for n in ["Site DNS", "Site HTTPS", "Site path", "Speed"] {
        assert!(has(&r, n), "{n}");
    }
    assert!(matches!(&f.speed, Some(Ok(s)) if s.down_mbps > 90.0));
}

struct Dead;
impl Runner for Dead {
    fn run(&self, p: &str, _: &[&str], _: u64) -> Result<(bool, String, String), String> {
        Err(format!("{p}: cannot run"))
    }
    fn tcp(&self, _: &str, _: u64) -> Result<f64, String> {
        Err("no network".into())
    }
}

#[test]
fn collector_survives_every_tool_failing() {
    let f = collect(
        &Dead,
        &snapshot(),
        &Options {
            speed: true,
            target: Some("x.example.test".into()),
            quick: false,
        },
    );
    let r = analyze(&f);
    assert!(!r.steps.is_empty() && !r.verdict.is_empty());
    assert!(matches!(f.speed, Some(Err(_))));
    let _ = render::json(&r);
    let _ = render::html(&r);
}

#[test]
fn redact_uses_separate_placeholders_for_ipv4_and_ipv6() {
    let secrets = vec![
        "fe80::1%en0".to_string(),
        "192.0.2.1".to_string(),
        "fe80::1".to_string(),
    ];
    assert_eq!(
        render::redact(
            "gw fe80::1%en0, dns 192.0.2.1 and fe80::1.".into(),
            &secrets
        ),
        "gw <IPv6>, dns <IPv4> and <IPv6>."
    );
    assert_eq!(
        render::redact("nothing to hide 1.1.1.1".into(), &secrets),
        "nothing to hide 1.1.1.1"
    );
    assert_eq!(render::redact(String::new(), &secrets), "");
}

#[test]
fn collector_treats_a_self_assigned_address_as_no_address() {
    let mut s = snapshot();
    for i in &mut s.interfaces {
        if i.name == "en0" {
            i.addresses
                .retain(|a| a.ip.contains(':') && !a.ip.starts_with("2001")); // keep only link-local v6
            i.addresses.push(zyvor_macora::model::Address {
                ip: "169.254.17.9".into(),
                prefix: Some(16),
                attributes: vec![],
            });
        }
    }
    let f = collect(
        &Fake::healthy(),
        &s,
        &Options {
            quick: true,
            ..Default::default()
        },
    );
    assert!(
        f.link.self_assigned && f.link.ipv4.is_empty() && !f.link.global_ipv6,
        "{:?}",
        f.link
    );
    let r = analyze(&f);
    assert_eq!(st(&r, "Link"), Status::Fail);
    assert!(detail(&r, "Link").contains("169.254") && detail(&r, "Link").contains("DHCP"));
    assert_eq!(st(&r, "Gateway"), Status::Skip);
}
