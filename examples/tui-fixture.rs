//! Render the real TUI against the sanitized fixture for documentation.
fn main() {
    let capture = include_str!("../tests/fixtures/macos.ifconfig");
    let snapshot = zyvor_macora::parse::ifconfig(capture).expect("valid bundled fixture");
    let mut app = zyvor_macora::tui::Dashboard::new(snapshot, Some("sanitized fixture".into()));
    app.session.all = true;
    app.tab = zyvor_macora::tui::Tab::Interfaces;
    println!("{}", app.render(115, 30, false));
}
