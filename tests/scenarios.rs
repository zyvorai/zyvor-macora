use std::{collections::BTreeMap, fs, process::Command};
use zyvor_macora::{
    cli, insight,
    model::{json_string, Counters},
    parse,
    view::{self, Filter, Theme},
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
#[test]
fn mac_grouping_and_scope_preserved() {
    let s = parse::ifconfig(MAC).unwrap();
    assert_eq!(s.interfaces.len(), 17);
    let en0 = s.interfaces.iter().find(|i| i.name == "en0").unwrap();
    assert!(en0.active());
    assert!(en0.up());
    assert_eq!(en0.group(), "Network");
    assert_eq!(en0.addresses.len(), 5);
    assert!(en0
        .addresses
        .iter()
        .any(|a| a.ip.ends_with("%en0") && a.scope() == "link-local"));
    assert!(en0
        .addresses
        .iter()
        .any(|a| a.ip == "192.168.1.8" && a.prefix == Some(24) && a.scope() == "private"));
    assert!(en0
        .addresses
        .iter()
        .any(|a| a.attributes.contains(&"deprecated".into())));
    assert!(en0.options.contains(&"ZEROINVERT_CSUM".into()));
    let bridge = s.interfaces.iter().find(|i| i.name == "bridge0").unwrap();
    assert_eq!(bridge.members, ["en1", "en2"]);
    assert!(!bridge.active());
    assert_eq!(
        s.interfaces
            .iter()
            .filter(|i| i.group() == "Tunnels")
            .count(),
        6
    );
}
#[test]
fn inactive_status_overrides_running_flag() {
    let s = parse::ifconfig(MAC).unwrap();
    assert!(!s
        .interfaces
        .iter()
        .find(|i| i.name == "en1")
        .unwrap()
        .active());
}
#[test]
fn default_collapses_but_expanded_exposes_everything() {
    let s = parse::ifconfig(MAC).unwrap();
    let t = Theme { color: false };
    let text = view::interfaces(&s, &Filter::default(), &t, &BTreeMap::new());
    assert!(text.contains("collapsed"));
    assert!(!text.contains("fe80::cb1c"));
    assert!(!text.contains('\x1b'));
    let all = view::interfaces(
        &s,
        &Filter {
            all: true,
            ..Default::default()
        },
        &t,
        &BTreeMap::new(),
    );
    for i in &s.interfaces {
        assert!(all.contains(&i.name));
    }
}
#[test]
fn diagnostics_explain_normal_apple_conditions() {
    let s = parse::ifconfig(MAC).unwrap();
    let f = insight::findings(&s).join("\n");
    assert!(f.contains("rotation"));
    assert!(f.contains("1280"));
    assert!(f.contains("Thunderbolt"));
    assert!(!f.contains("VPN is active"));
}
#[test]
fn self_assigned_and_duplicate_are_actionable() {
    let text = "en0: flags=1<UP,RUNNING> mtu 1500\n inet 169.254.1.2 netmask 0xffff0000\n inet6 fe80::1%en0 prefixlen 64 duplicated\n status: active";
    let s = parse::ifconfig(text).unwrap();
    let f = insight::findings(&s).join("\n");
    assert!(f.contains("DHCP"));
    assert!(f.contains("duplicate"));
}
#[test]
fn rates_handle_reset_missing_and_elapsed_time() {
    let mut a = parse::ifconfig(MAC).unwrap();
    a.interfaces[0].counters = Some(Counters { rx: 100, tx: 100 });
    let mut b = a.clone();
    b.interfaces[0].counters = Some(Counters { rx: 300, tx: 50 });
    assert_eq!(
        view::rates(&a, &b, 2.0)[&a.interfaces[0].name],
        (100.0, 0.0)
    );
    assert!(view::rates(&a, &b, 0.0).is_empty());
    assert!(view::rates(&a, &b, f64::NAN).is_empty());
}
#[test]
fn snapshots_diff_captures_state_address_mtu_bridge_changes() {
    let a = parse::ifconfig(MAC).unwrap();
    let mut b = a.clone();
    b.interfaces.retain(|i| i.name != "nan0");
    let en0 = b.interfaces.iter_mut().find(|i| i.name == "en0").unwrap();
    en0.mtu = Some(1400);
    en0.status = Some("inactive".into());
    en0.addresses.clear();
    let bridge = b
        .interfaces
        .iter_mut()
        .find(|i| i.name == "bridge0")
        .unwrap();
    bridge.members.clear();
    let d = insight::diff(&a, &b).join("\n");
    for word in [
        "MTU en0",
        "STATE en0",
        "ADDRESS- en0",
        "REMOVED nan0",
        "MEMBERS bridge0",
    ] {
        assert!(d.contains(word));
    }
    assert!(insight::diff(&a, &a).is_empty());
}
#[test]
fn subnet_boundaries_and_validation() {
    let s = insight::subnet("192.168.1.8/24", false).unwrap();
    assert!(s.contains("192.168.1.0/24"));
    assert!(s.contains("192.168.1.254"));
    assert!(s.contains("254"));
    assert!(insight::subnet("10.0.0.0/31", true)
        .unwrap()
        .contains("\"usable_hosts\":2"));
    assert!(insight::subnet("10.0.0.1/32", true)
        .unwrap()
        .contains("\"usable_hosts\":1"));
    assert!(insight::subnet("0.0.0.0/0", true)
        .unwrap()
        .contains("4294967294"));
    assert!(insight::subnet("1.2.3.4/33", false).is_err());
    assert!(insight::subnet("bad/24", false).is_err());
}
#[test]
fn invalid_flags_and_arguments_fail() {
    for args in [
        vec!["--interval", "NaN"],
        vec!["--count", "0"],
        vec!["--color", "banana"],
        vec!["--ipv4", "--ipv6"],
        vec!["--unknown"],
        vec!["--group", "bad"],
        vec!["--from"],
    ] {
        assert!(cli::arguments(args.iter().map(|s| s.to_string()).collect()).is_err());
    }
    assert!(!run(&["show", "extra"]).status.success());
    assert!(!run(&["show", "--apply"]).status.success());
}
#[test]
fn json_escaping_and_terminal_safety() {
    assert_eq!(json_string("a\"\\\n\x1b"), "\"a\\\"\\\\\\n\\u001b\"");
    let t = Theme { color: false };
    assert!(!t.paint("36", "untrusted\x1b[31m\rtext").contains('\x1b'));
}
#[test]
fn redaction_removes_scoped_addresses_and_macs() {
    let s = parse::ifconfig(MAC).unwrap();
    let redacted = view::redact(s.json(), &s);
    for i in &s.interfaces {
        if let Some(mac) = &i.mac {
            assert!(!redacted.contains(mac));
        }
        for a in &i.addresses {
            assert!(!redacted.contains(&json_string(&a.ip)));
        }
    }
    assert!(redacted.contains("<IPv4>"));
}
#[test]
fn hardware_mapping_is_not_assumed_from_en0() {
    let h = parse::hardware_ports("Hardware Port: Wi-Fi\nDevice: en2\nEthernet Address: aa:bb\n\nHardware Port: Thunderbolt 1\nDevice: en1\n");
    assert_eq!(h["en2"], "Wi-Fi");
    assert_eq!(h["en1"], "Thunderbolt 1");
}
#[test]
fn mutation_plans_validate_device_and_mtu() {
    let s = parse::ifconfig(MAC).unwrap();
    let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(
        cli::set_plan(&args(&["set", "en0", "mtu", "1400"]), &s).unwrap(),
        ["en0", "mtu", "1400"]
    );
    assert!(cli::set_plan(&args(&["set", "en0;id", "up"]), &s).is_err());
    assert!(cli::set_plan(&args(&["set", "en0", "mtu", "-1"]), &s).is_err());
    assert!(cli::set_plan(&args(&["set", "missing", "down"]), &s).is_err());
    assert!(cli::set_plan(&args(&["set", "en0", "up", "extra"]), &s).is_err());
}
#[test]
fn cli_offline_show_and_explain() {
    let output = run(&["show", "--from", fixture(), "--all", "--no-color"]);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("OFFLINE CAPTURE"));
    assert!(text.contains("192.168.1.8/24"));
    let output = run(&["explain", "utun0", "--from", fixture()]);
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("does not identify its owner"));
}
#[test]
fn cli_family_selection_and_redacted_json() {
    let o = run(&[
        "show",
        "--from",
        fixture(),
        "--interface",
        "en0",
        "--ipv4",
        "--json",
        "--redact",
    ]);
    assert!(o.status.success());
    let text = String::from_utf8(o.stdout).unwrap();
    assert!(text.contains("<IPv4>"));
    assert!(!text.contains("192.168.1.8"));
    assert!(!text.contains("fe80::"));
    assert!(!text.contains('\x1b'));
    assert!(!text.contains("\"name\":\"utun0\""));
}
#[test]
fn snapshot_private_no_overwrite_and_apply_offline_rejected() {
    let path = std::env::temp_dir().join(format!("macora-{}-capture", std::process::id()));
    let _ = fs::remove_file(&path);
    let name = path.to_str().unwrap();
    assert!(run(&["snapshot", name, "--from", fixture()])
        .status
        .success());
    assert!(!run(&["snapshot", name, "--from", fixture()])
        .status
        .success());
    assert_eq!(fs::read_to_string(&path).unwrap(), MAC);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    assert!(
        !run(&["set", "en0", "down", "--from", fixture(), "--apply"])
            .status
            .success()
    );
    fs::remove_file(path).unwrap();
}
#[test]
fn tcp_probe_against_local_listener() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let output = run(&["probe", &addr, "--json", "--timeout-ms", "1000"]);
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("\"reachable\":true"));
    drop(listener);
    assert!(!run(&["probe", &addr, "--timeout-ms", "100"])
        .status
        .success());
}
#[test]
fn command_failure_reports_error_not_empty_success() {
    assert!(zyvor_macora::collect::command("/definitely-not-a-program", &[]).is_err());
}
