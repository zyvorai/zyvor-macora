use std::{collections::BTreeMap, process::Command};
use zyvor_macora::{
    export, parse,
    probe::{self, Status, Step},
    tui,
};
const MAC: &str = include_str!("fixtures/macos.ifconfig");

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_macora"))
        .args(args)
        .output()
        .unwrap()
}
fn fixture() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/macos.ifconfig")
}
fn step(name: &'static str, status: Status) -> Step {
    Step {
        name,
        status,
        detail: String::new(),
    }
}

#[test]
fn ping_parses_macos_linux_and_total_loss() {
    let m = probe::parse_ping(include_str!("fixtures/ping-macos.txt")).unwrap();
    assert_eq!((m.sent, m.received), (5, 4));
    assert_eq!(m.loss_pct, 20.0);
    assert_eq!(m.avg_ms, Some(3.456));
    let l = probe::parse_ping(include_str!("fixtures/ping-linux.txt")).unwrap();
    assert_eq!((l.sent, l.received, l.loss_pct), (3, 3, 0.0));
    assert_eq!(l.max_ms, Some(10.3));
    let t = probe::parse_ping(include_str!("fixtures/ping-timeout.txt")).unwrap();
    assert_eq!(t.received, 0);
    assert_eq!(t.loss_pct, 100.0);
    assert_eq!(t.avg_ms, None);
    assert!(probe::parse_ping("garbage").is_err());
}

#[test]
fn hosts_reject_shell_and_option_injection() {
    for ok in ["example.com", "192.168.1.1", "::1", "my-host_1.local"] {
        assert!(probe::valid_host(ok), "{ok}");
    }
    for bad in ["", "-c1", "a;b", "a b", "$(id)", "http://x", "a|b", "`x`"] {
        assert!(!probe::valid_host(bad), "{bad}");
    }
    assert!(!run(&["ping", "a;b"]).status.success());
    assert!(!run(&["trace", "-x"]).status.success());
}

#[test]
fn verdict_blames_the_lowest_failing_layer() {
    let v = |s: &[Step]| probe::verdict(s);
    assert!(
        v(&[step("Link", Status::Fail), step("Gateway", Status::Skip)])
            .contains("local connection")
    );
    assert!(v(&[step("Link", Status::Ok), step("Gateway", Status::Fail)]).contains("router"));
    assert!(
        v(&[step("Gateway", Status::Ok), step("Internet", Status::Fail)])
            .contains("internet does not")
    );
    assert!(
        v(&[step("Internet", Status::Ok), step("DNS", Status::Fail)])
            .contains("names do not resolve")
    );
    assert!(v(&[step("Gateway", Status::Warn)]).contains("packet loss"));
    assert!(v(&[step("MTU", Status::Warn)]).contains("MTU"));
    assert!(v(&[step("Link", Status::Ok), step("DNS", Status::Ok)]).contains("No problem found"));
    // The lowest layer wins even when higher layers also fail.
    assert!(v(&[step("DNS", Status::Fail), step("Gateway", Status::Fail)]).contains("router"));
}

#[test]
fn diagnose_json_is_structured() {
    let j = probe::steps_json(&[step("Link", Status::Ok)]);
    assert!(j.starts_with("{\"steps\":[{\"step\":\"Link\",\"status\":\"OK\""));
    assert!(j.contains("\"verdict\":"));
}

#[test]
fn html_report_escapes_and_has_no_script() {
    let mut s = parse::ifconfig(MAC).unwrap();
    s.interfaces[0].name = "<script>alert(1)</script>".into();
    let html = export::html_report(&s);
    assert!(!html.contains("<script"));
    assert!(html.contains("&lt;script&gt;"));
    assert!(html.contains("prefers-color-scheme"));
    assert!(html.starts_with("<!doctype html>"));
    assert_eq!(
        export::escape_html("a&b\"'<>"),
        "a&amp;b&quot;&#39;&lt;&gt;"
    );
}

#[test]
fn prometheus_output_is_well_formed() {
    let s = parse::ifconfig(MAC).unwrap();
    let mut rates = BTreeMap::new();
    rates.insert("en0".to_string(), (1500.0, 250.5));
    let text = export::prometheus(&s, &rates);
    assert!(text.contains("# TYPE macora_interface_up gauge"));
    assert!(text.contains("macora_interface_up{interface=\"en0\"} 1"));
    assert!(text.contains("macora_interface_receive_bytes_per_second{interface=\"en0\"} 1500.00"));
    for line in text.lines().filter(|l| !l.starts_with('#')) {
        let (series, value) = line.rsplit_once(' ').unwrap();
        assert!(series.starts_with("macora_"), "{line}");
        assert!(value.parse::<f64>().is_ok(), "{line}");
    }
    let mut weird = parse::ifconfig(MAC).unwrap();
    weird.interfaces[0].name = "a\"b\\c".into();
    assert!(export::prometheus(&weird, &BTreeMap::new()).contains("a\\\"b\\\\c"));
}

#[test]
fn completions_cover_every_command_and_flag() {
    for shell in ["bash", "zsh", "fish"] {
        let text = export::completions(shell).unwrap();
        for c in ["diagnose", "ping", "trace", "completions"] {
            assert!(text.contains(c), "{shell} missing {c}");
        }
        assert!(
            text.contains("--format") || text.contains("format"),
            "{shell}"
        );
        assert!(text.contains("prometheus"), "{shell}");
    }
    assert!(export::completions("powershell").is_err());
    let out = run(&["completions", "zsh"]);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("#compdef macora"));
}

#[test]
fn every_help_command_is_in_the_completion_table() {
    for line in zyvor_macora::cli::HELP.lines().skip(3) {
        if line.trim().is_empty() {
            break;
        }
        let name = line.split_whitespace().next().unwrap();
        for part in name.split('/') {
            assert!(
                export::COMMANDS.contains(&part),
                "{part} missing from completions"
            );
        }
    }
}

#[test]
fn format_flag_is_validated_per_command() {
    assert!(run(&["--from", fixture(), "--format", "json"])
        .status
        .success());
    let json = run(&["--from", fixture(), "--format", "json"]);
    assert!(String::from_utf8_lossy(&json.stdout)
        .trim_start()
        .starts_with('{'));
    let html = run(&["report", "--from", fixture(), "--format", "html"]);
    assert!(html.status.success());
    assert!(String::from_utf8_lossy(&html.stdout).contains("<!doctype html>"));
    let prom = run(&["show", "--from", fixture(), "--format", "prometheus"]);
    assert!(String::from_utf8_lossy(&prom.stdout).contains("macora_interface_up"));
    for bad in [
        vec!["--format", "yaml"],
        vec!["show", "--format", "md"],
        vec!["doctor", "--format", "html"],
        vec!["routes", "--format", "prometheus"],
        vec!["show", "--theme", "mono"],
        vec!["tui", "--theme", "neon"],
        vec!["diagnose", "--from", fixture()],
        vec!["ping", "localhost", "--redact"],
        vec!["completions"],
    ] {
        assert!(!run(&bad).status.success(), "{bad:?} should fail");
    }
}

#[test]
fn redaction_still_applies_to_html_report() {
    let out = run(&[
        "report",
        "--from",
        fixture(),
        "--format",
        "html",
        "--redact",
    ]);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("&lt;IPv4&gt;") || text.contains("<IPv4>") || !text.contains("192.168."));
}

#[test]
fn tui_palettes_remap_codes() {
    assert_eq!(tui::restyle("default", "90"), "90");
    assert_eq!(tui::restyle("mono", "92"), "0");
    assert_eq!(tui::restyle("mono", "90"), "2");
    assert_eq!(tui::restyle("high-contrast", "90"), "37");
}
