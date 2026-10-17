//! Parsers, the JSON reader and the path-MTU search: pure code, tested against recorded tool output
//! (sanitized, documentation-range addresses) and by brute force.
use std::sync::atomic::{AtomicU32, Ordering};
use zyvor_macora::{
    diagnose::{parse, pmtu},
    json,
    probe::parse_ping,
};

fn fx(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/diagnose/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

// ---------- JSON reader ----------

#[test]
fn json_reads_every_value_type() {
    let v = json::parse(r#" {"a":[1,-2.5e1,true,false,null,"x"],"b":{"c":"d"},"e":""} "#).unwrap();
    let a = v.get("a").unwrap().as_array().unwrap();
    assert_eq!(a[0].as_f64(), Some(1.0));
    assert_eq!(a[1].as_f64(), Some(-25.0));
    assert_eq!(a[2], json::Value::Bool(true));
    assert_eq!(a[4], json::Value::Null);
    assert_eq!(a[5].as_str(), Some("x"));
    assert_eq!(v.get("b").unwrap().get("c").unwrap().as_str(), Some("d"));
    assert_eq!(v.get("e").unwrap().as_str(), Some(""));
    assert!(v.get("missing").is_none() && v.idx(0).is_none());
}

#[test]
fn json_decodes_escapes_and_surrogate_pairs() {
    let v = json::parse(r#""q\"b\\s\/n\n\té😀""#).unwrap();
    assert_eq!(v.as_str(), Some("q\"b\\s/n\n\té😀"));
    assert!(json::parse(r#""\ud83d""#).is_err(), "lone surrogate");
    assert!(json::parse(r#""\ud83dA""#).is_err(), "bad low surrogate");
    assert!(json::parse(r#""\x""#).is_err(), "unknown escape");
    assert!(json::parse(r#""\u12""#).is_err(), "short \\u");
}

#[test]
fn json_rejects_malformed_input() {
    for bad in [
        "",
        "{",
        "[1,",
        r#"{"a"}"#,
        r#"{"a":}"#,
        "[1 2]",
        "tru",
        "nul",
        "01x",
        "--1",
        "1.2.3",
        r#"{"a":1} x"#,
        "\"abc",
        "{\"a\":1,}",
        "[,1]",
        "NaN",
    ] {
        assert!(json::parse(bad).is_err(), "{bad:?} must be rejected");
    }
}

#[test]
fn json_limits_depth_and_size() {
    let deep = "[".repeat(200) + &"]".repeat(200);
    assert!(json::parse(&deep).unwrap_err().contains("deeper"));
    let ok = "[".repeat(60) + &"]".repeat(60);
    assert!(json::parse(&ok).is_ok());
    let huge = format!("[{}]", "1,".repeat(5 * 1024 * 1024));
    assert!(json::parse(&huge).unwrap_err().contains("8 MiB"));
}

#[test]
fn json_never_panics_on_truncated_or_mutated_documents() {
    for name in ["wifi-5ghz.json", "wifi-24ghz.json", "networkquality.json"] {
        let text = fx(name);
        let bytes = text.as_bytes();
        // Every prefix of a valid document, and single-byte corruptions of it.
        for cut in 0..bytes.len() {
            if let Ok(s) = std::str::from_utf8(&bytes[..cut]) {
                let _ = json::parse(s);
            }
        }
        for i in (0..bytes.len()).step_by(7) {
            for repl in [b'{', b'"', b'\\', b'9', 0u8, b' '] {
                let mut m = bytes.to_vec();
                m[i] = repl;
                if let Ok(s) = String::from_utf8(m) {
                    let _ = json::parse(&s);
                    let _ = parse::wifi(&s, "en0");
                    let _ = parse::network_quality(&s);
                }
            }
        }
    }
}

// ---------- route / dns / proxy ----------

#[test]
fn route_reads_gateway_interface_and_mtu() {
    let r = parse::route(&fx("route-v4.txt"));
    assert_eq!(r.gateway.as_deref(), Some("192.0.2.1"));
    assert_eq!(r.interface.as_deref(), Some("en0"));
    assert_eq!(r.mtu, Some(1500));
    let r6 = parse::route(&fx("route-v6.txt"));
    assert_eq!(r6.gateway.as_deref(), Some("fe80::1%en0"));
}

#[test]
fn route_through_a_tunnel_has_no_gateway() {
    let r = parse::route(&fx("route-tunnel.txt"));
    assert_eq!(r.gateway, None);
    assert_eq!(r.interface.as_deref(), Some("utun4"));
    assert_eq!(r.mtu, Some(1380));
}

#[test]
fn route_with_no_default_is_empty_and_garbage_is_harmless() {
    assert_eq!(parse::route(&fx("route-none.txt")), Default::default());
    assert_eq!(parse::route(""), Default::default());
    assert_eq!(
        parse::route("gateway:\n interface:   \n"),
        Default::default()
    );
    // A header with no values line must not panic.
    assert_eq!(parse::route(" recvpipe sendpipe mtu").mtu, None);
}

#[test]
fn dns_servers_keep_order_and_drop_duplicates_and_mdns() {
    assert_eq!(
        parse::dns_servers(&fx("scutil-dns.txt")),
        ["fe80::1%en0", "192.0.2.1", "198.51.100.53"]
    );
    assert!(parse::dns_servers("").is_empty());
    assert!(parse::dns_servers("nameserver[0] :\n").is_empty());
}

#[test]
fn proxy_settings_are_detected_only_when_enabled() {
    assert!(!parse::proxy(&fx("scutil-proxy-none.txt")).any());
    let p = parse::proxy(&fx("scutil-proxy-set.txt"));
    assert_eq!(p.http.as_deref(), Some("proxy.example.test:8080"));
    assert_eq!(p.https.as_deref(), Some("proxy.example.test:8443"));
    assert_eq!(p.socks, None, "SOCKSEnable is 0");
    assert_eq!(p.pac.as_deref(), Some("http://pac.example.test/proxy.pac"));
    assert!(p.auto_discovery && p.any());
}

// ---------- dig / curl / ping ----------

#[test]
fn dig_distinguishes_answers_failures_and_timeouts() {
    let ok = parse::dig(&fx("dig-noerror.txt"));
    assert_eq!(
        (ok.status.as_deref(), ok.ms, ok.answered()),
        (Some("NOERROR"), Some(18.0), true)
    );
    let nx = parse::dig(&fx("dig-nxdomain.txt"));
    assert!(nx.answered(), "NXDOMAIN proves the resolver is alive");
    assert_eq!(nx.ms, Some(31.0));
    let sf = parse::dig(&fx("dig-servfail.txt"));
    assert!(!sf.answered());
    assert_eq!(
        (sf.status.as_deref(), sf.ms),
        (Some("SERVFAIL"), Some(2004.0))
    );
    let to = parse::dig(&fx("dig-timeout.txt"));
    assert!(to.timed_out && !to.answered() && to.status.is_none() && to.ms.is_none());
    assert!(!parse::dig("").answered());
}

#[test]
fn curl_timing_splits_into_stages() {
    let h = parse::curl_timing(&fx("curl-ok.txt"), "www.example.test", 4).unwrap();
    assert!(h.ok && h.code == 200 && h.family == 4);
    assert_eq!(h.ip.as_deref(), Some("198.51.100.7"));
    let close = |a: f64, b: f64| (a - b).abs() < 0.01;
    assert!(close(h.dns_ms, 14.08) && close(h.total_ms, 94.755));
    assert!(close(h.tcp_stage(), 25.237 - 14.080));
    assert!(close(h.tls_stage(), 38.278 - 25.237));
    assert!(close(h.wait_stage(), 61.110 - 38.278));
}

#[test]
fn curl_failure_and_garbage() {
    let h = parse::curl_timing(&fx("curl-fail.txt"), "x", 6).unwrap();
    assert!(!h.ok && h.code == 0 && h.error.is_some() && h.ip.is_none());
    assert!(parse::curl_timing("", "x", 4).is_none());
    assert!(parse::curl_timing("curl: (6) Could not resolve host", "x", 4).is_none());
    // Plain HTTP has no TLS stage.
    let h = parse::curl_timing(
        "dns=0.01 connect=0.02 tls=0.000000 ttfb=0.05 total=0.06 ip=198.51.100.1 http=200",
        "x",
        4,
    )
    .unwrap();
    assert_eq!(h.tls_stage(), 0.0);
    assert!((h.wait_stage() - 30.0).abs() < 0.01);
}

#[test]
fn ping_reads_jitter_from_both_spellings_and_drops_nan() {
    let v4 = parse_ping(&fx("ping-v4.txt")).unwrap();
    assert_eq!((v4.sent, v4.received, v4.stddev_ms), (10, 10, Some(0.611)));
    let v6 = parse_ping(&fx("ping-v6.txt")).unwrap();
    assert_eq!(v6.stddev_ms, Some(0.511));
    assert_eq!(v6.avg_ms, Some(9.053));
    let one = parse_ping(&fx("ping-one-packet.txt")).unwrap();
    assert_eq!(
        (one.avg_ms, one.stddev_ms),
        (Some(11.868), None),
        "nan jitter is dropped"
    );
    let linux = parse_ping("3 packets transmitted, 3 received, 0% packet loss, time 2003ms\nrtt min/avg/max/mdev = 8.1/9.2/10.3/0.9 ms").unwrap();
    assert_eq!(linux.stddev_ms, Some(0.9));
}

// ---------- Wi-Fi / networkQuality ----------

#[test]
fn wifi_5ghz_fields() {
    let w = parse::wifi(&fx("wifi-5ghz.json"), "en0").unwrap();
    assert!(w.associated);
    assert_eq!((w.rssi, w.noise, w.snr()), (Some(-66), Some(-94), Some(28)));
    assert_eq!(
        (w.rate_mbps, w.channel, w.mcs),
        (Some(360), Some(40), Some(7))
    );
    assert_eq!(w.band.as_deref(), Some("5GHz"));
    assert_eq!(w.width.as_deref(), Some("80MHz"));
    assert_eq!(w.phy.as_deref(), Some("802.11ax"));
}

#[test]
fn wifi_24ghz_is_normalized() {
    let w = parse::wifi(&fx("wifi-24ghz.json"), "en0").unwrap();
    assert_eq!(w.band.as_deref(), Some("2.4GHz"), "macOS prints 2GHz");
    assert_eq!((w.rssi, w.snr()), (Some(-82), Some(8)));
    assert_eq!(w.width.as_deref(), Some("20MHz"));
}

#[test]
fn wifi_off_missing_fields_and_wrong_interface() {
    let off = parse::wifi(&fx("wifi-off.json"), "en0").unwrap();
    assert!(!off.associated && off.rssi.is_none());
    let partial = parse::wifi(&fx("wifi-missing-fields.json"), "en0").unwrap();
    assert!(
        partial.associated
            && partial.rssi.is_none()
            && partial.snr().is_none()
            && partial.band.is_none()
    );
    assert!(
        parse::wifi(&fx("wifi-5ghz.json"), "en9").is_none(),
        "unknown interface"
    );
    assert!(parse::wifi("not json", "en0").is_none());
    assert!(parse::wifi("{}", "en0").is_none());
    assert!(parse::wifi(r#"{"SPAirPortDataType":[]}"#, "en0").is_none());
}

#[test]
fn network_quality_converts_units() {
    let s = parse::network_quality(&fx("networkquality.json")).unwrap();
    assert!((s.down_mbps - 94.91508).abs() < 1e-4 && (s.up_mbps - 107.178248).abs() < 1e-4);
    assert!(
        (s.rpm.unwrap() - 228.2865).abs() < 1e-3 && (s.base_rtt_ms.unwrap() - 67.7775).abs() < 1e-3
    );
    assert!(parse::network_quality("{}").is_err());
    assert!(parse::network_quality(r#"{"dl_throughput":1}"#).is_err());
    assert!(parse::network_quality("garbage").is_err());
}

// ---------- path MTU search ----------

#[test]
fn pmtu_finds_every_possible_mtu_exactly() {
    for mtu in 576..=1500u32 {
        let o = pmtu::search(1500, &|size| size <= mtu);
        assert_eq!(o.mtu, Some(mtu), "true MTU {mtu}");
        assert!(o.rounds <= 4, "MTU {mtu} took {} rounds", o.rounds);
    }
}

#[test]
fn pmtu_full_size_path_needs_one_round() {
    let calls = AtomicU32::new(0);
    let o = pmtu::search(1500, &|_| {
        calls.fetch_add(1, Ordering::SeqCst);
        true
    });
    assert_eq!((o.mtu, o.rounds), (Some(1500), 1));
    assert_eq!(o.probes, calls.load(Ordering::SeqCst));
}

#[test]
fn pmtu_below_the_floor_is_unmeasurable() {
    for mtu in [0, 100, 575] {
        assert_eq!(pmtu::search(1500, &|s| s <= mtu).mtu, None, "{mtu}");
    }
    assert_eq!(pmtu::search(1500, &|_| false).mtu, None);
}

#[test]
fn pmtu_other_link_sizes() {
    assert_eq!(pmtu::search(1400, &|s| s <= 1380).mtu, Some(1380));
    assert_eq!(pmtu::search(9000, &|s| s <= 1500).mtu, Some(1500));
    assert_eq!(pmtu::search(9000, &|_| true).mtu, Some(9000));
    assert_eq!(
        pmtu::search(100, &|s| s <= 600).mtu,
        Some(576),
        "link below the floor is clamped"
    );
}

#[test]
fn pmtu_survives_a_flaky_oracle() {
    // Every 37th size spuriously fails: the answer may be a little low, but never above the truth,
    // never below the floor, and never a panic.
    for mtu in [1500u32, 1492, 1480, 1400, 1280] {
        let o = pmtu::search(1500, &|s| s <= mtu && s % 37 != 0);
        let got = o.mtu.expect("something works");
        assert!((pmtu::FLOOR..=mtu).contains(&got), "flaky {mtu} -> {got}");
        assert!(o.rounds <= 5);
    }
}

#[test]
fn pmtu_hints_name_the_usual_causes() {
    assert!(pmtu::cause_hint(1492).contains("PPPoE"));
    assert!(pmtu::cause_hint(1480).contains("6in4"));
    assert!(pmtu::cause_hint(1420).contains("VPN"));
    assert!(!pmtu::cause_hint(1300).is_empty() && !pmtu::cause_hint(1497).is_empty());
}

#[test]
fn pmtu_never_probes_the_same_size_twice() {
    for mtu in 576..=1500u32 {
        let seen = std::sync::Mutex::new(Vec::new());
        let o = pmtu::search(1500, &|s| {
            seen.lock().unwrap().push(s);
            s <= mtu
        });
        let mut sizes = seen.into_inner().unwrap();
        let total = sizes.len();
        sizes.sort_unstable();
        sizes.dedup();
        assert_eq!(
            sizes.len(),
            total,
            "true MTU {mtu}: a size was probed twice"
        );
        assert_eq!(
            o.probes as usize, total,
            "probe counter matches the real number of probes"
        );
        assert!(total <= 15 + 24 * 3, "MTU {mtu} used {total} probes");
    }
}
