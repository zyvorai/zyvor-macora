//! Dependency-free terminal dashboard: pure state/rendering plus a thin tty adapter.
use crate::{
    explore, insight,
    model::Snapshot,
    session,
    view::{self, Theme},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, IsTerminal, Read, Write},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Overview,
    Interfaces,
    Detail,
    Topology,
    Diagnostics,
    Changes,
}
impl Tab {
    pub fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Interfaces => "Interfaces",
            Self::Detail => "Detail",
            Self::Topology => "Topology",
            Self::Diagnostics => "Diagnostics",
            Self::Changes => "Changes",
        }
    }
}
pub struct Dashboard {
    pub session: session::Session,
    pub tab: Tab,
    pub index: usize,
    pub scroll: usize,
    pub paused: bool,
    pub help: bool,
    pub entry: Option<char>,
    pub input: String,
    pub status: String,
    pub favorites: BTreeSet<String>,
    pub sort: u8,
    pub rates: BTreeMap<String, (f64, f64)>,
    pub traffic: Vec<f64>,
    pub palette: String,
}
/// Maps the SGR codes the dashboard emits onto a named palette.
pub fn restyle(palette: &str, code: &str) -> String {
    match (palette, code) {
        // Monochrome keeps only weight: dim for secondary text, bold for emphasis.
        ("mono", "90") => "2".into(),
        ("mono", "1;96") | ("mono", "93") => "1".into(),
        ("mono", _) => "0".into(),
        // High contrast avoids grey-on-black and the dim cyan.
        ("high-contrast", "90") => "37".into(),
        ("high-contrast", "36") => "1;96".into(),
        ("high-contrast", "92") => "1;92".into(),
        ("high-contrast", "93") => "1;93".into(),
        _ => code.into(),
    }
}
impl Dashboard {
    pub fn new(s: Snapshot, source: Option<String>) -> Self {
        Self {
            session: session::Session::new(s, source),
            tab: Tab::Overview,
            index: 0,
            scroll: 0,
            paused: false,
            help: false,
            entry: None,
            input: String::new(),
            status: "Ready. ? help, : command palette, / search".into(),
            favorites: BTreeSet::new(),
            sort: 0,
            rates: BTreeMap::new(),
            traffic: vec![],
            palette: "default".into(),
        }
    }
    pub fn names(&self) -> Vec<String> {
        let q = self.session.query.to_lowercase();
        let mut items: Vec<_> = self
            .session
            .snapshot
            .interfaces
            .iter()
            .filter(|i| {
                (self.session.all || !["System", "Loopback", "Tunnels"].contains(&i.group()))
                    && (q.is_empty()
                        || format!(
                            "{} {} {} {}",
                            i.name,
                            i.label(),
                            i.group(),
                            i.addresses
                                .iter()
                                .map(|a| a.ip.clone())
                                .collect::<Vec<_>>()
                                .join(" ")
                        )
                        .to_lowercase()
                        .contains(&q))
            })
            .collect();
        items.sort_by(|a, b| {
            let pinned =
                (!self.favorites.contains(&a.name)).cmp(&(!self.favorites.contains(&b.name)));
            if pinned != std::cmp::Ordering::Equal {
                return pinned;
            }
            match self.sort {
                1 => a.name.cmp(&b.name),
                2 => b.mtu.cmp(&a.mtu).then(a.name.cmp(&b.name)),
                _ => b.active().cmp(&a.active()).then(a.name.cmp(&b.name)),
            }
        });
        items.into_iter().map(|i| i.name.clone()).collect()
    }
    pub fn selected(&self) -> Option<String> {
        self.names().get(self.index).cloned()
    }
    pub fn ingest(&mut self, s: Snapshot, seconds: f64) {
        self.rates = view::rates(&self.session.snapshot, &s, seconds);
        let values: Vec<_> = s
            .interfaces
            .iter()
            .filter(|i| i.group() == "Network")
            .filter_map(|i| self.rates.get(&i.name))
            .collect();
        if !values.is_empty() {
            self.traffic.push(values.iter().map(|(a, b)| a + b).sum());
            if self.traffic.len() > 60 {
                self.traffic.remove(0);
            }
        }
        let selected = self.selected();
        self.session.replace(s);
        self.index = selected
            .and_then(|n| self.names().iter().position(|i| i == &n))
            .unwrap_or(0);
    }
    pub fn action(&mut self, input: &str) -> Result<(), String> {
        let cmd = input.trim().to_lowercase();
        self.scroll = 0;
        match cmd.as_str(){
            "overview"|"home"=>self.tab=Tab::Overview,"interfaces"|"network"=>self.tab=Tab::Interfaces,
            "detail"=>self.tab=Tab::Detail,"topology"=>self.tab=Tab::Topology,"doctor"|"diagnostics"=>self.tab=Tab::Diagnostics,"changes"=>self.tab=Tab::Changes,
            "pause"=>self.paused= !self.paused,"baseline"=>{self.session.baseline=self.session.snapshot.clone();self.status="Baseline captured; Changes compares configuration, not traffic counters.".into();},
            "all"=>{self.session.all= !self.session.all;self.index=0;},"redact"=>self.session.redact= !self.session.redact,
            "help"=>self.help= !self.help,
            _ if cmd.starts_with("select ")=>{let name=input.trim()[7..].trim();self.session.all=true;self.index=self.names().iter().position(|n|n==name).ok_or("Interface not found")?;self.tab=Tab::Detail;},
            _=>return Err("Unknown palette action. Try overview, interfaces, topology, doctor, changes, baseline, pause, all, redact.".into()),
        }
        Ok(())
    }
    pub fn key(&mut self, b: u8) -> bool {
        if self.entry.is_some() {
            match b {
                3 | 27 => {
                    self.entry = None;
                    self.input.clear();
                }
                10 | 13 => {
                    let mode = self.entry.take();
                    let input = std::mem::take(&mut self.input);
                    if mode == Some('/') {
                        self.session.query = input;
                        self.index = 0;
                        self.scroll = 0;
                    } else if let Err(e) = self.action(&input) {
                        self.status = e;
                    }
                }
                8 | 127 => {
                    self.input.pop();
                }
                32..=126 if self.input.len() < 180 => {
                    self.input.push(b as char);
                }
                _ => {}
            }
            return true;
        }
        match b {
            b'q' | 3 | 4 => return false,
            b'1' => self.tab = Tab::Overview,
            b'2' => self.tab = Tab::Interfaces,
            b'3' => self.tab = Tab::Detail,
            b'4' => self.tab = Tab::Topology,
            b'5' => self.tab = Tab::Diagnostics,
            b'6' => self.tab = Tab::Changes,
            b'\t' => {
                self.tab = match self.tab {
                    Tab::Overview => Tab::Interfaces,
                    Tab::Interfaces => Tab::Detail,
                    Tab::Detail => Tab::Topology,
                    Tab::Topology => Tab::Diagnostics,
                    Tab::Diagnostics => Tab::Changes,
                    Tab::Changes => Tab::Overview,
                };
                self.scroll = 0;
            }
            b'j' => {
                if self.tab == Tab::Interfaces {
                    self.index = (self.index + 1).min(self.names().len().saturating_sub(1));
                } else {
                    self.scroll = self.scroll.saturating_add(1);
                }
            }
            b'k' => {
                if self.tab == Tab::Interfaces {
                    self.index = self.index.saturating_sub(1);
                } else {
                    self.scroll = self.scroll.saturating_sub(1);
                }
            }
            13 | 10 => {
                self.tab = Tab::Detail;
                self.scroll = 0;
            }
            b'a' => {
                self.session.all = !self.session.all;
                self.index = 0;
            }
            b'f' => {
                if let Some(name) = self.selected() {
                    if !self.favorites.remove(&name) {
                        self.favorites.insert(name.clone());
                    }
                    self.index = self.names().iter().position(|n| n == &name).unwrap_or(0);
                }
            }
            b's' => self.sort = (self.sort + 1) % 3,
            b'p' => self.paused = !self.paused,
            b'b' => {
                let _ = self.action("baseline");
            }
            b'd' => self.tab = Tab::Changes,
            b'?' => self.help = !self.help,
            b'/' | b':' => {
                self.entry = Some(b as char);
                self.input.clear();
            }
            _ => {}
        }
        true
    }
    pub fn render(&self, width: usize, height: usize, color: bool) -> String {
        let width = width.clamp(30, 240);
        let height = height.clamp(8, 100);
        let theme = Theme { color };
        let names = self.names();
        let mode = if self.session.source.is_some() {
            "OFFLINE"
        } else {
            "LIVE"
        };
        let mut lines: Vec<(String, String)> = vec![
            (
                "1;96".into(),
                format!(
                    "MACORA  /  {}  /  {}{}",
                    self.tab.title(),
                    mode,
                    if self.paused { " · PAUSED" } else { "" }
                ),
            ),
            (
                "90".into(),
                "1 Overview   2 Interfaces   3 Detail   4 Topology   5 Diagnostics   6 Changes"
                    .into(),
            ),
            (
                "90".into(),
                format!(
                    "{} interfaces · {} visible · filter {:?} · sort {} · {}",
                    self.session.snapshot.interfaces.len(),
                    names.len(),
                    self.session.query,
                    ["active", "name", "MTU"][self.sort as usize],
                    if self.session.redact {
                        "IP/MAC MASKED"
                    } else {
                        "unmasked"
                    }
                ),
            ),
            ("90".into(), "─".repeat(width)),
        ];
        let mut body: Vec<(String, String)> = Vec::new();
        if self.help {
            for line in ["KEYBOARD WORKSPACE","j/k or arrows: move/scroll · Enter: interface detail · Tab: next page","/: filter · : command palette · f: favorite · s: sort · a: include system links","p: pause/resume · r: refresh now · b: baseline · d: changes · ?: help · q: quit","Palette: overview, interfaces, detail, topology, doctor, changes, baseline, pause, all, redact","Terminal mode never applies settings. Use macora interactive for a reviewed mutation.","A tunnel is not proof of a VPN. Link-active is not an internet reachability test."]{body.push(("36".into(),line.into()));}
        } else {
            match self.tab {
                Tab::Overview => {
                    body.push(("1;97".into(), "Your network. Without the noise.".into()));
                    body.push((
                        "37".into(),
                        format!(
                            "{} link-active adapters · {} bridge(s) · {} tunnel(s)",
                            self.session
                                .snapshot
                                .interfaces
                                .iter()
                                .filter(|i| i.group() == "Network" && i.active())
                                .count(),
                            self.session
                                .snapshot
                                .interfaces
                                .iter()
                                .filter(|i| i.group() == "Bridges")
                                .count(),
                            self.session
                                .snapshot
                                .interfaces
                                .iter()
                                .filter(|i| i.group() == "Tunnels")
                                .count()
                        ),
                    ));
                    body.push((
                        "36".into(),
                        format!(
                            "Adapter traffic  {}",
                            sparkline(&self.traffic, width.saturating_sub(20))
                        ),
                    ));
                    if self.traffic.is_empty() {
                        body.push(("90".into(),"Awaiting two successful byte-counter samples; unavailable data is not estimated.".into()));
                    }
                    for i in self
                        .session
                        .snapshot
                        .interfaces
                        .iter()
                        .filter(|i| i.group() == "Network")
                    {
                        body.push((
                            if i.active() { "92" } else { "90" }.into(),
                            format!(
                                "{}  {}  {}",
                                i.name,
                                i.label(),
                                if i.active() {
                                    "LINK ACTIVE"
                                } else {
                                    "IDLE/DOWN"
                                }
                            ),
                        ));
                        for a in &i.addresses {
                            if a.scope() != "link-local" {
                                body.push(("37".into(), format!("  {} · {}", a.cidr(), a.scope())));
                            }
                        }
                    }
                    body.push((
                        "90".into(),
                        "b captures a baseline. Configuration drift appears on Changes.".into(),
                    ));
                }
                Tab::Interfaces => {
                    body.push((
                        "90".into(),
                        format!(
                            "  {:<12} {:<12} {:<12} {:<7} {}",
                            "INTERFACE", "GROUP", "STATE", "MTU", "LABEL"
                        ),
                    ));
                    let viewport = height.saturating_sub(9);
                    let start = self.index.saturating_sub(viewport.saturating_sub(1));
                    for (n, name) in names.iter().enumerate().skip(start).take(viewport) {
                        if let Some(i) = self
                            .session
                            .snapshot
                            .interfaces
                            .iter()
                            .find(|i| &i.name == name)
                        {
                            body.push((
                                if n == self.index { "1;96" } else { "37" }.into(),
                                format!(
                                    "{}{} {:<12} {:<12} {:<12} {:<7} {}",
                                    if n == self.index { ">" } else { " " },
                                    if self.favorites.contains(name) {
                                        "*"
                                    } else {
                                        " "
                                    },
                                    i.name,
                                    i.group(),
                                    if i.active() {
                                        "ACTIVE"
                                    } else if i.up() {
                                        "IDLE"
                                    } else {
                                        "DOWN"
                                    },
                                    i.mtu.map(|n| n.to_string()).unwrap_or("?".into()),
                                    i.label()
                                ),
                            ));
                        }
                    }
                    if names.is_empty() {
                        body.push((
                            "93".into(),
                            "No matching interfaces. / clears the filter; a expands system groups."
                                .into(),
                        ));
                    }
                }
                Tab::Detail => {
                    if let Some(i) = self.selected().and_then(|n| {
                        self.session
                            .snapshot
                            .interfaces
                            .iter()
                            .find(|i| i.name == n)
                    }) {
                        body.push(("1;96".into(), format!("{} / {}", i.name, i.label())));
                        for a in &i.addresses {
                            body.push((
                                "37".into(),
                                format!("{}  {}  {}", a.cidr(), a.scope(), a.attributes.join(", ")),
                            ));
                        }
                        for line in insight::explain(i).lines() {
                            body.push(("37".into(), line.into()));
                        }
                    } else {
                        body.push(("93".into(), "Select an interface on tab 2.".into()));
                    }
                }
                Tab::Topology => {
                    for line in explore::topology(&self.session.snapshot, false).lines() {
                        body.push(("36".into(), line.into()));
                    }
                }
                Tab::Diagnostics => {
                    for line in insight::findings(&self.session.snapshot) {
                        body.push((
                            if line.starts_with("WARN") { "93" } else { "36" }.into(),
                            line,
                        ));
                    }
                }
                Tab::Changes => {
                    let d = insight::diff(&self.session.baseline, &self.session.snapshot);
                    if d.is_empty() {
                        body.push((
                            "90".into(),
                            "No configuration changes from baseline. Counters are excluded.".into(),
                        ));
                    }
                    for line in d {
                        body.push(("33".into(), line));
                    }
                }
            }
        }
        let viewport = height.saturating_sub(7);
        let offset = if self.tab == Tab::Interfaces {
            0
        } else {
            self.scroll.min(body.len().saturating_sub(viewport))
        };
        lines.extend(body.into_iter().skip(offset).take(viewport));
        while lines.len() < height - 3 {
            lines.push(("37".into(), String::new()));
        }
        lines.push(("90".into(), "─".repeat(width)));
        lines.push(("93".into(), self.status.clone()));
        lines.push((
            "36".into(),
            self.entry
                .map(|mode| format!("{mode}{}", self.input))
                .unwrap_or(
                "/: search   : palette   j/k: move   Enter: detail   p: pause   ?: help   q: exit"
                    .into(),
            ),
        ));
        lines
            .into_iter()
            .take(height)
            .map(|(code, line)| {
                let mut line = if self.session.redact {
                    view::redact(
                        view::redact(line, &self.session.baseline),
                        &self.session.snapshot,
                    )
                } else {
                    line
                };
                line = view::safe(&line).replace('\n', " ");
                let line = line.chars().take(width).collect::<String>();
                theme.paint(&restyle(&self.palette, &code), &line)
            })
            .collect::<Vec<_>>()
            .join("\r\n")
    }
}
pub fn sparkline(values: &[f64], width: usize) -> String {
    if values.is_empty() {
        return "[waiting for samples]".into();
    }
    let max = values
        .iter()
        .copied()
        .filter(|n| n.is_finite())
        .fold(0.0, f64::max)
        .max(1.0);
    let glyphs = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    values
        .iter()
        .skip(values.len().saturating_sub(width))
        .map(|n| {
            glyphs[if n.is_finite() {
                ((n.max(0.0) / max * 7.0).round() as usize).min(7)
            } else {
                0
            }]
        })
        .collect()
}
fn stty(args: &[&str]) -> Result<String, String> {
    let program = if cfg!(target_os = "macos") {
        "/bin/stty"
    } else {
        "/usr/bin/stty"
    };
    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::inherit())
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().into())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into())
    }
}
struct Terminal {
    saved: String,
}
impl Terminal {
    fn enter() -> Result<Self, String> {
        let saved = stty(&["-g"])?;
        stty(&["raw", "-echo", "min", "0", "time", "1"])?;
        let guard = Self { saved };
        print!("\x1b[?1049h\x1b[?25l");
        io::stdout().flush().map_err(|e| e.to_string())?;
        Ok(guard)
    }
}
impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = stty(&[&self.saved]);
        print!("\x1b[0m\x1b[?25h\x1b[?1049l");
        let _ = io::stdout().flush();
    }
}
pub fn run(
    source: Option<String>,
    interval: f64,
    color: bool,
    redact: bool,
    all: bool,
    palette: &str,
) -> Result<(), String> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(
            "tui requires an interactive terminal; use interactive for piped sessions".into(),
        );
    }
    let mut app = Dashboard::new(session::load(source.as_deref())?, source);
    app.session.redact = redact;
    app.session.all = all;
    app.palette = palette.into();
    let _terminal = Terminal::enter()?;
    let running = Arc::new(AtomicBool::new(true));
    let active = running.clone();
    let (send, recv) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut stdin = io::stdin().lock();
        let mut b = [0];
        while active.load(Ordering::Relaxed) {
            match stdin.read(&mut b) {
                Ok(1) => {
                    if send.send(b[0]).is_err() {
                        break;
                    }
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
    });
    let mut last = Instant::now();
    let mut last_attempt = last;
    let mut escape = 0u8;
    let mut keep = true;
    let mut force_refresh = false;
    let result = (|| -> Result<(), String> {
        while keep {
            let size = stty(&["size"]).unwrap_or("30 110".into());
            let parts: Vec<_> = size.split_whitespace().collect();
            let height = parts.first().and_then(|n| n.parse().ok()).unwrap_or(30);
            let width = parts.get(1).and_then(|n| n.parse().ok()).unwrap_or(110);
            print!("\x1b[H\x1b[J{}", app.render(width, height, color));
            io::stdout().flush().map_err(|e| e.to_string())?;
            match recv.recv_timeout(Duration::from_millis(100)) {
                Ok(b) => {
                    // Arrow keys become the same tested j/k/Tab actions. Escape cancels text input.
                    if escape == 1 {
                        if b == b'[' {
                            escape = 2;
                            continue;
                        }
                        escape = 0;
                    }
                    if escape == 2 {
                        escape = 0;
                        keep = app.key(match b {
                            b'A' => b'k',
                            b'B' => b'j',
                            b'C' | b'D' => b'\t',
                            _ => 0,
                        });
                        continue;
                    }
                    if b == 27 {
                        if app.entry.is_some() {
                            app.key(27);
                        } else {
                            escape = 1;
                        }
                        continue;
                    }
                    if b == b'r' && app.entry.is_none() {
                        force_refresh = true;
                    }
                    keep = app.key(b);
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    escape = 0;
                }
            }
            if !keep {
                break;
            }
            if force_refresh
                || (!app.paused && last_attempt.elapsed() >= Duration::from_secs_f64(interval))
            {
                force_refresh = false;
                last_attempt = Instant::now();
                match session::load(app.session.source.as_deref()) {
                    Ok(s) => {
                        let now = Instant::now();
                        app.ingest(s, now.duration_since(last).as_secs_f64());
                        last = now;
                        app.status =
                            "Capture refreshed. Link state is not an internet health check.".into();
                    }
                    Err(e) => app.status = format!("STALE · refresh failed: {e}"),
                }
            }
        }
        Ok(())
    })();
    running.store(false, Ordering::Relaxed);
    let _ = reader.join();
    result
}
