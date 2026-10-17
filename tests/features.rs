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
    let mut in_commands = false;
    let mut seen = 0;
    for line in zyvor_macora::cli::HELP.lines() {
        if line.starts_with("COMMANDS") {
            in_commands = true;
            continue;
        }
        if in_commands {
            if line.trim().is_empty() {
                break;
            }
            for part in line.split_whitespace().next().unwrap().split('/') {
                assert!(
                    export::COMMANDS.contains(&part),
                    "{part} missing from completions"
                );
                seen += 1;
            }
        }
    }
    assert!(seen >= 20, "parsed {seen} help commands");
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

fn fixture_order() -> Vec<String> {
    MAC.lines()
        .filter(|l| !l.starts_with(char::is_whitespace) && l.contains(": flags="))
        .map(|l| l.split(':').next().unwrap().to_string())
        .collect()
}
fn stdout(args: &[&str]) -> String {
    String::from_utf8_lossy(&run(args).stdout).to_string()
}

#[test]
fn list_keeps_system_order_and_filters_by_state() {
    let order = fixture_order();
    let listed = stdout(&["list", "from", fixture()]);
    assert_eq!(
        listed.trim().split(' ').collect::<Vec<_>>(),
        order,
        "list must keep ifconfig order"
    );
    let up = stdout(&["list", "up", "from", fixture()]);
    let down = stdout(&["list", "down", "from", fixture()]);
    let (up, down): (Vec<_>, Vec<_>) = (
        up.split_whitespace().collect(),
        down.split_whitespace().collect(),
    );
    assert!(
        up.iter().all(|n| !down.contains(n)),
        "up and down are disjoint"
    );
    assert_eq!(up.len() + down.len(), order.len());
    assert!(
        !run(&["list", "up", "down", "from", fixture()])
            .status
            .success(),
        "up with down is rejected"
    );
}

#[test]
fn interface_name_works_like_ifconfig() {
    let one = stdout(&["en0", "from", fixture()]);
    assert!(
        one.contains("en0") && !one.contains("bridge0"),
        "only the named interface"
    );
    let missing = run(&["nosuch0", "from", fixture()]);
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("not found"));
    // A real command name is never mistaken for an interface.
    assert!(run(&["subnet", "10.0.0.0/8"]).status.success());
}

#[test]
fn raw_is_the_original_ifconfig_text() {
    let raw = stdout(&["en0", "raw", "from", fixture()]);
    let start = MAC.lines().position(|l| l.starts_with("en0:")).unwrap();
    let expected: Vec<&str> = std::iter::once(MAC.lines().nth(start).unwrap())
        .chain(
            MAC.lines()
                .skip(start + 1)
                .take_while(|l| l.starts_with(char::is_whitespace)),
        )
        .collect();
    assert_eq!(raw.trim_end(), expected.join("\n"));
    assert!(
        !run(&["routes", "raw"]).status.success(),
        "raw only applies to the interface view"
    );
}

#[test]
fn changes_by_word_are_previews_until_apply() {
    let out = run(&["en0", "mtu", "1400", "from", fixture()]);
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("Preview") && text.contains("/sbin/ifconfig en0 mtu 1400"),
        "{text}"
    );
    let down = stdout(&["en0", "down", "from", fixture()]);
    assert!(down.contains("/sbin/ifconfig en0 down"), "{down}");
    assert!(
        !run(&["en0", "mtu", "1400", "apply", "from", fixture()])
            .status
            .success(),
        "offline apply is refused"
    );
    assert!(!run(&["nosuch0", "up", "from", fixture()]).status.success());
}

#[test]
fn option_words_match_the_dash_flags() {
    let a = stdout(&["show", "all", "compact", "from", fixture()]);
    let b = stdout(&["show", "--all", "--compact", "--from", fixture()]);
    assert_eq!(
        a, b,
        "words and the hidden dash flags produce the same output"
    );
    assert_eq!(
        stdout(&["json", "from", fixture()]),
        stdout(&["--json", "--from", fixture()])
    );
    assert_eq!(
        stdout(&["report", "md", "from", fixture()]),
        stdout(&["report", "--from", fixture()])
    );
    let html = stdout(&["report", "html", "redact", "from", fixture()]);
    assert!(html.starts_with("<!doctype html>") && !html.contains("192.168."));
    // Positional arguments are never mistaken for words.
    assert!(stdout(&["subnet", "10.0.0.0/8", "json"]).starts_with('{'));
    assert!(run(&["resolve", "up"]).status.code().is_some());
}

#[test]
fn words_translate_to_flags() {
    use zyvor_macora::cli::words_to_flags;
    let w = |v: &[&str]| words_to_flags(v.iter().map(|s| s.to_string()).collect());
    assert_eq!(w(&["en0"]), ["show", "--interface", "en0", "--all"]);
    assert_eq!(w(&["en0", "up"]), ["set", "en0", "up"]);
    assert_eq!(
        w(&["en0", "mtu", "1400", "apply"]),
        ["set", "en0", "mtu", "1400", "--apply"]
    );
    assert_eq!(w(&["list", "up"]), ["show", "--list", "--up"]);
    assert_eq!(
        w(&["watch", "count", "3", "json"]),
        ["watch", "--count", "3", "--json"]
    );
    assert_eq!(
        w(&["diff", "a", "b", "redact"]),
        ["diff", "a", "b", "--redact"]
    );
    // A value word may lead: `macora color always`, `macora from FILE`.
    assert_eq!(w(&["color", "always"]), ["show", "--color", "always"]);
    assert_eq!(
        w(&["from", "x.cap", "list"]),
        ["show", "--from", "x.cap", "--list"]
    );
    assert_eq!(
        w(&["all", "en0"]),
        ["show", "--all", "--interface", "en0", "--all"]
    );
    assert_eq!(w(&["show", "en0"]), ["show", "--interface", "en0", "--all"]);
    // Existing dash flags and their values pass through untouched.
    assert_eq!(
        w(&["--format", "html", "report"]),
        ["--format", "html", "report"]
    );
}
