use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
};
use zyvor_macora::{
    explore, parse,
    session::{split, Response, Session},
    tui::{sparkline, Dashboard, Tab},
};
const CAPTURE: &str = include_str!("fixtures/macos.ifconfig");
fn session() -> Session {
    Session::new(parse::ifconfig(CAPTURE).unwrap(), Some("offline".into()))
}
fn text(r: Response) -> String {
    match r {
        Response::Text(s) => s,
        _ => panic!("expected text"),
    }
}
#[test]
fn quoted_arguments_are_literal_and_unclosed_quotes_rejected() {
    assert_eq!(
        split("export '/tmp/my report.json'").unwrap(),
        ["export", "/tmp/my report.json"]
    );
    assert_eq!(
        split("filter \"Wi-Fi adapter\"").unwrap(),
        ["filter", "Wi-Fi adapter"]
    );
    assert_eq!(split("filter ''").unwrap(), ["filter", ""]);
    assert!(split("filter 'unfinished").is_err());
    assert!(split("filter foo\\").is_err());
}
#[test]
fn shell_operators_are_refused_without_evaluation() {
    for s in ["show | cat", "show; id", "show > /tmp/a", "show `id`"] {
        assert!(split(s).is_err());
    }
    assert_eq!(
        split("filter '$(id); echo hi'").unwrap()[1],
        "$(id); echo hi"
    );
}
#[test]
fn selection_detail_and_clear_focus() {
    let mut s = session();
    s.execute("select en0", false).unwrap();
    assert_eq!(s.filtered().interfaces.len(), 1);
    assert!(text(s.execute("detail", false).unwrap()).contains("en0"));
    assert!(s.execute("select absent", false).is_err());
    s.execute("select all", false).unwrap();
    assert_eq!(s.filtered().interfaces.len(), 17);
}
#[test]
fn query_is_case_insensitive_and_family_is_validated() {
    let mut s = session();
    s.execute("filter TUNNELS", false).unwrap();
    assert_eq!(s.filtered().interfaces.len(), 6);
    s.execute("filter", false).unwrap();
    assert_eq!(s.filtered().interfaces.len(), 17);
    s.execute("family 6", false).unwrap();
    assert_eq!(s.family, Some(6));
    assert!(s.execute("family 7", false).is_err());
}
#[test]
fn plan_preview_requires_selection_and_validates_values() {
    let mut s = session();
    assert!(s.execute("plan down", false).is_err());
    s.execute("select en0", false).unwrap();
    assert!(
        text(s.execute("plan mtu 1400", false).unwrap()).contains("/sbin/ifconfig en0 mtu 1400")
    );
    assert!(s.pending.is_some());
    assert!(s.execute("plan mtu 1", false).is_err());
    assert!(s.pending.is_none());
}
#[test]
fn offline_plan_cannot_apply_and_input_changes_invalidate_it() {
    let mut s = session();
    s.execute("select en0", false).unwrap();
    s.execute("plan down", false).unwrap();
    assert!(s.execute("apply", false).is_err());
    s.execute("filter network", false).unwrap();
    assert!(s.pending.is_none());
    s.execute("plan up", false).unwrap();
    s.execute("cancel", false).unwrap();
    assert!(s.pending.is_none());
}
#[test]
fn history_can_recall_inspection_but_never_authorize_apply() {
    let mut s = session();
    s.execute("select en0", false).unwrap();
    s.execute("show", false).unwrap();
    assert!(text(s.execute("!2", false).unwrap()).contains("192.168.1.8"));
    assert!(s.execute("!0", false).is_err());
    assert!(s.execute("!999", false).is_err());
    s.history.push("apply".into());
    let n = s.history.len();
    assert!(s.execute(&format!("!{n}"), false).is_err());
}
#[test]
fn refresh_replacement_drops_stale_selection_and_plan() {
    let mut s = session();
    s.execute("select en0", false).unwrap();
    s.execute("plan down", false).unwrap();
    let mut next = s.snapshot.clone();
    next.interfaces.retain(|i| i.name != "en0");
    s.replace(next);
    assert!(s.selected.is_none());
    assert!(s.pending.is_none());
    assert!(text(s.execute("changes", false).unwrap()).contains("REMOVED en0"));
}
#[test]
fn baseline_reset_and_report_use_cached_evidence() {
    let mut s = session();
    s.snapshot.interfaces[0].mtu = Some(999);
    assert!(text(s.execute("changes", false).unwrap()).contains("MTU"));
    s.execute("baseline", false).unwrap();
    assert!(text(s.execute("changes", false).unwrap()).contains("No configuration changes"));
    assert!(text(s.execute("report", false).unwrap()).contains("not an internet reachability test"));
}
#[test]
fn redaction_applies_to_detail_diff_and_export() {
    let mut s = session();
    s.execute("redact on", false).unwrap();
    s.execute("select en0", false).unwrap();
    let detail = text(s.execute("show", false).unwrap());
    assert!(!detail.contains("192.168.1.8"));
    assert!(detail.contains("<IPv4>"));
    if let Response::Export(path, json) = s.execute("export 'my file.json'", false).unwrap() {
        assert_eq!(path, "my file.json");
        assert!(!json.contains("192.168.1.8"));
    } else {
        panic!("missing export");
    }
}
#[test]
fn offline_sessions_block_live_system_and_outbound_tools() {
    let mut s = session();
    for cmd in [
        "routes",
        "dns",
        "wifi",
        "probe 1.1.1.1:443",
        "resolve example.com",
    ] {
        assert!(s.execute(cmd, false).is_err());
    }
    assert!(matches!(
        s.execute("subnet 10.0.0.1/24", false).unwrap(),
        Response::External(_)
    ));
}
#[test]
fn tui_tabs_search_pinning_and_exit_work() {
    let mut d = Dashboard::new(parse::ifconfig(CAPTURE).unwrap(), None);
    assert!(d.key(b'2'));
    assert_eq!(d.tab, Tab::Interfaces);
    assert_eq!(d.selected().as_deref(), Some("en0"));
    d.key(b'f');
    assert!(d.favorites.contains("en0"));
    d.key(b'/');
    for b in b"utun" {
        d.key(*b);
    }
    d.key(13);
    assert!(d.names().is_empty());
    d.key(b'a');
    assert_eq!(d.names().len(), 6);
    d.key(b'j');
    assert_eq!(d.index, 1);
    d.key(13);
    assert_eq!(d.tab, Tab::Detail);
    assert!(!d.key(b'q'));
}
#[test]
fn tui_palette_and_escape_cancel_input() {
    let mut d = Dashboard::new(parse::ifconfig(CAPTURE).unwrap(), None);
    d.key(b':');
    for b in b"topology" {
        d.key(*b);
    }
    d.key(13);
    assert_eq!(d.tab, Tab::Topology);
    d.key(b'/');
    d.key(b'x');
    d.key(27);
    assert!(d.entry.is_none());
    assert!(d.session.query.is_empty());
    assert!(d.action("rm -rf anything").is_err());
}
#[test]
fn tui_render_is_bounded_and_redacted() {
    let mut d = Dashboard::new(parse::ifconfig(CAPTURE).unwrap(), Some("offline".into()));
    d.session.redact = true;
    for tab in [
        Tab::Overview,
        Tab::Interfaces,
        Tab::Detail,
        Tab::Topology,
        Tab::Diagnostics,
        Tab::Changes,
    ] {
        d.tab = tab;
        let rendered = d.render(84, 26, false);
        assert!(rendered.lines().count() <= 26);
        assert!(!rendered.contains("192.168.1.8"));
        assert!(!rendered.contains('\x1b'));
        for line in rendered.lines() {
            assert!(line.chars().count() <= 84);
        }
    }
    let tiny = d.render(8, 4, false);
    assert!(!tiny.is_empty());
}
#[test]
fn tui_ingest_keeps_baseline_changes_and_clears_stale_index() {
    let mut d = Dashboard::new(parse::ifconfig(CAPTURE).unwrap(), None);
    let mut s = d.session.snapshot.clone();
    s.interfaces.retain(|i| i.name != "en0");
    d.index = 999;
    d.ingest(s, 2.0);
    assert_eq!(d.index, 0);
    d.tab = Tab::Changes;
    assert!(d.render(100, 30, false).contains("REMOVED en0"));
}
#[test]
fn sparklines_handle_empty_zero_nan_and_truncation() {
    assert!(sparkline(&[], 20).contains("waiting"));
    assert_eq!(sparkline(&[0., 0.], 20), "▁▁");
    assert_eq!(sparkline(&[1., 2., 8.], 2).chars().count(), 2);
    assert_eq!(sparkline(&[f64::NAN], 5), "▁");
}
#[test]
fn topology_does_not_infer_vpn_or_route_edges() {
    let s = parse::ifconfig(CAPTURE).unwrap();
    let json = explore::topology(&s, true);
    assert!(json.contains("\"bridge\":\"bridge0\",\"member\":\"en1\""));
    assert!(json.contains("no inferred internet"));
    assert!(explore::report(&s, true).contains("local interface evidence"));
}
#[test]
fn cli_repl_handles_piped_session_eof_and_export_permissions() {
    let binary = env!("CARGO_BIN_EXE_macora");
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/macos.ifconfig");
    let path = std::env::temp_dir().join(format!("macora-repl-{}.json", std::process::id()));
    let _ = fs::remove_file(&path);
    let mut child = Command::new(binary)
        .args(["interactive", "--from", fixture, "--no-color"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let commands = format!(
        "select en0\nredact on\nshow\nplan mtu 1400\napply\nexport '{}'\nexport '{}'\nquit\n",
        path.display(),
        path.display()
    );
    child
        .stdin
        .take()
        .unwrap()
        .write_all(commands.as_bytes())
        .unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(result.status.success());
    let stdout = String::from_utf8(result.stdout).unwrap();
    assert!(stdout.contains("OFFLINE CAPTURE"));
    assert!(stdout.contains("<IPv4>"));
    let stderr = String::from_utf8(result.stderr).unwrap();
    assert!(stderr.contains("Offline captures"));
    assert!(stderr.contains("Export failed"));
    assert!(!fs::read_to_string(&path).unwrap().contains("192.168.1.8"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    fs::remove_file(path).unwrap();
    let mut child = Command::new(binary)
        .args(["repl", "--from", fixture])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    drop(child.stdin.take());
    assert!(child.wait().unwrap().success());
}
#[test]
fn nonterminal_tui_refuses_and_new_commands_accept_offline_json() {
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/macos.ifconfig");
    for cmd in ["report", "topology"] {
        let o = Command::new(env!("CARGO_BIN_EXE_macora"))
            .args([cmd, "--from", fixture, "--json", "--redact"])
            .output()
            .unwrap();
        assert!(o.status.success());
        assert!(!String::from_utf8(o.stdout).unwrap().contains("192.168.1.8"));
    }
    let o = Command::new(env!("CARGO_BIN_EXE_macora"))
        .args(["tui", "--from", fixture])
        .output()
        .unwrap();
    assert!(!o.status.success());
}
