//! The command line around `diagnose`: argument validation (no network needed), the words, the help
//! text, completions, and the real TCP prober against sockets we control.
use std::{
    net::TcpListener,
    process::Command,
    time::{Duration, Instant},
};
use zyvor_macora::{
    cli::words_to_flags,
    diagnose::{RealRunner, Runner},
    export,
};

fn macora(args: &[&str]) -> (bool, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_macora"))
        .args(args)
        .output()
        .unwrap();
    (
        o.status.success(),
        String::from_utf8_lossy(&o.stdout).into(),
        String::from_utf8_lossy(&o.stderr).into(),
    )
}
fn w(v: &[&str]) -> Vec<String> {
    words_to_flags(v.iter().map(|s| s.to_string()).collect())
}

#[test]
fn diagnose_words_translate_to_flags() {
    assert_eq!(
        w(&["diagnose", "to", "github.com"]),
        ["diagnose", "--to", "github.com"]
    );
    assert_eq!(
        w(&["diagnose", "quick", "redact", "md"]),
        ["diagnose", "--quick", "--redact", "--format", "md"]
    );
    assert_eq!(
        w(&["diagnose", "speed", "strict"]),
        ["diagnose", "--speed", "--strict"]
    );
    assert_eq!(
        w(&["diagnose", "count", "5", "interval", "30", "json"]),
        ["diagnose", "--count", "5", "--interval", "30", "--json"]
    );
    assert_eq!(
        w(&["quick", "diagnose"]),
        ["diagnose", "--quick"],
        "a command wins over leading words"
    );
    // `to` is a word only after a command, never an interface name that happens to look like one.
    assert_eq!(
        w(&["diagnose", "html", "to", "example.test"]),
        ["diagnose", "--format", "html", "--to", "example.test"]
    );
}

#[test]
fn bad_site_names_are_rejected_before_any_probe() {
    for bad in [
        "a;b",
        "a b",
        "$(id)",
        "http://x",
        "-x",
        "a|b",
        "host:443",
        "fe80::1%en0",
        "x%y",
    ] {
        let (ok, _, err) = macora(&["diagnose", "to", bad]);
        assert!(!ok, "{bad:?} must be rejected");
        assert!(
            err.contains("hostname after `to`") || err.contains("Unknown option"),
            "{bad:?}: {err}"
        );
    }
    let (ok, _, err) = macora(&["diagnose", "to"]);
    assert!(!ok && err.contains("needs a value"), "{err}");
}

#[test]
fn diagnose_words_are_rejected_on_other_commands() {
    for args in [
        ["show", "speed"],
        ["routes", "quick"],
        ["doctor", "strict"],
        ["show", "to"],
    ] {
        let mut a = args.to_vec();
        if a[1] == "to" {
            a.push("example.test");
        }
        let (ok, _, err) = macora(&a);
        assert!(!ok && err.contains("apply to diagnose"), "{a:?}: {err}");
    }
}

#[test]
fn live_checks_reject_offline_captures_and_ping_has_nothing_to_redact() {
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/macos.ifconfig");
    let (ok, _, err) = macora(&["diagnose", "from", fixture]);
    assert!(!ok && err.contains("live check"), "{err}");
    let (ok, _, err) = macora(&["ping", "127.0.0.1", "redact"]);
    assert!(!ok && err.contains("nothing to redact"), "{err}");
    // diagnose accepts redact, md and html as formats (they only fail later if not on macOS).
    for fmt in ["md", "html"] {
        let (_, _, err) = macora(&["diagnose", "to", "bad host", fmt]);
        assert!(!err.contains("not supported for diagnose"), "{fmt}: {err}");
    }
}

#[test]
fn formats_that_do_not_apply_to_diagnose_are_rejected() {
    let (ok, _, err) = macora(&["diagnose", "prometheus"]);
    assert!(!ok && err.contains("not supported for diagnose"), "{err}");
}

#[test]
fn help_and_completions_know_the_new_words() {
    let (ok, help, _) = macora(&["help"]);
    assert!(ok);
    for word in [
        "to HOST",
        "quick",
        "speed",
        "strict",
        "count N interval S",
        "path MTU",
    ] {
        assert!(help.contains(word), "help is missing {word:?}");
    }
    for shell in ["bash", "zsh", "fish"] {
        let c = export::completions(shell).unwrap();
        for word in ["speed", "quick", "strict", "to"] {
            assert!(c.contains(word), "{shell} completions lack {word}");
        }
    }
    assert!(
        !help.contains("--quick") && !help.contains("--speed"),
        "help documents words, not dashes"
    );
}

// ---------- the real TCP prober ----------

#[test]
fn tcp_prober_reports_open_ports_with_a_time() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let ms = RealRunner.tcp(&addr, 2000).expect("open port connects");
    assert!((0.0..1000.0).contains(&ms), "{ms}");
}

#[test]
fn tcp_prober_reports_a_closed_port_as_refused_quickly() {
    // Port 1 (tcpmux) is closed on any ordinary machine. An ephemeral port that was just released is
    // not reliable here: a parallel test can bind it again.
    let started = Instant::now();
    let e = RealRunner.tcp("127.0.0.1:1", 2000).unwrap_err();
    assert!(e.contains("refused"), "{e}");
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn tcp_prober_gives_up_on_an_unreachable_address_within_its_timeout() {
    // 192.0.2.0/24 is reserved for documentation: nothing answers there.
    let started = Instant::now();
    let r = RealRunner.tcp("192.0.2.254:9", 400);
    assert!(r.is_err(), "{r:?}");
    assert!(
        started.elapsed() < Duration::from_millis(2500),
        "took {:?}",
        started.elapsed()
    );
}

#[test]
fn tcp_prober_rejects_garbage_addresses() {
    for bad in ["", "not-an-address", "1.1.1.1", "[::1]"] {
        assert!(RealRunner.tcp(bad, 200).is_err(), "{bad:?}");
    }
}

#[test]
fn real_runner_runs_programs_without_a_shell_and_reports_failures() {
    let (ok, out, _) = RealRunner.run("/bin/echo", &["a;b", "$HOME"], 5).unwrap();
    assert!(
        ok && out.trim() == "a;b $HOME",
        "arguments are literal: {out:?}"
    );
    let (ok, _, _) = RealRunner.run("/usr/bin/false", &[], 5).unwrap();
    assert!(!ok);
    assert!(RealRunner.run("/nonexistent/program", &[], 5).is_err());
    let started = Instant::now();
    assert!(
        RealRunner.run("/bin/sleep", &["30"], 1).is_err(),
        "a hung tool is killed"
    );
    assert!(started.elapsed() < Duration::from_secs(4));
}

#[test]
fn exit_status_is_one_on_failure_and_on_warnings_only_when_strict() {
    use zyvor_macora::{cli::diagnose_exit, diagnose::Status::*};
    for strict in [false, true] {
        assert!(
            diagnose_exit(Fail, strict).is_err(),
            "a failing layer always exits 1"
        );
        for fine in [Ok, Info, Skip] {
            assert!(
                diagnose_exit(fine, strict).is_ok(),
                "{fine:?} strict={strict}"
            );
        }
    }
    assert!(
        diagnose_exit(Warn, false).is_ok(),
        "warnings alone do not fail a script"
    );
    let e = diagnose_exit(Warn, true).unwrap_err();
    assert!(e.contains("strict"), "{e}");
}

#[test]
fn repeated_runs_keep_the_most_serious_outcome() {
    use zyvor_macora::{cli::worse, diagnose::Status::*};
    assert_eq!(worse(Ok, Ok), Ok);
    assert_eq!(worse(Ok, Warn), Warn);
    assert_eq!(worse(Warn, Ok), Warn);
    assert_eq!(worse(Warn, Fail), Fail);
    assert_eq!(worse(Fail, Ok), Fail);
    assert_eq!(
        worse(Info, Skip),
        Ok,
        "informational steps are not problems"
    );
}

#[test]
fn completion_offers_nothing_after_to_because_a_hostname_follows() {
    let bash = export::completions("bash").unwrap();
    assert!(bash.contains("to) return;;"), "bash");
    let zsh = export::completions("zsh").unwrap();
    assert!(zsh.contains("to) return;;"), "zsh");
    let fish = export::completions("fish").unwrap();
    assert_eq!(fish.matches("not __fish_prev_arg_in to").count(), 2, "fish");
    // And the bash script really behaves that way.
    let script = format!(
        "source <({} completions bash); COMP_WORDS=(macora diagnose to ''); COMP_CWORD=3; _macora; echo \"[${{COMPREPLY[*]}}]\"",
        env!("CARGO_BIN_EXE_macora")
    );
    let out = Command::new("bash").args(["-c", &script]).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "[]");
}
