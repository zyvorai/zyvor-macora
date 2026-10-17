//! A persistent, shell-free exploration session. Inputs are parsed as arguments,
//! not shell programs. Settings only create a pending plan until explicit apply.
use crate::{
    cli, collect, explore, insight,
    model::Snapshot,
    parse,
    view::{self, Filter, Theme},
};
use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::{self, BufRead, IsTerminal, Write},
};

pub const HELP: &str = "MACORA Interactive · commands
  show / ls / 1       Grouped interfaces from the current session capture
  select NAME         Focus an interface; select all clears focus
  detail / 2          Explain the selected interface
  filter TEXT         Filter by name, label, address or group; blank clears
  all                 Toggle expanded system/tunnel groups
  family 4|6|all      Select address family
  doctor / 3          Local findings
  topology / 4        Observed bridge relationships
  refresh / r         Reload live source or the offline capture
  baseline            Remember the current capture
  changes / 5         Compare the baseline with current capture
  report              Markdown evidence from this session
  export FILE         Create private JSON from this session (never overwrite)
  redact on|off       Toggle IP/MAC masking for displayed/exported records
  plan up|down        Preview selected interface admin state
  plan mtu NUMBER     Preview selected interface MTU
  apply               Apply the pending plan, only after explicit confirmation
  cancel              Discard pending plan
  routes / dns / wifi / services / neighbors / path / sockets
  subnet CIDR / probe IP:PORT / resolve HOST
  history / !NUMBER   Recall a session command; history is memory-only
  help / ? / menu     Show this menu
  quit / q / exit     End session; EOF also exits

Changes to selection, filters or source invalidate pending plans. Offline sessions
cannot apply settings. No shell expansion, pipes or embedded shell commands run.";
#[derive(Clone, Debug, Default)]
pub struct Pending {
    pub args: Vec<String>,
}
pub struct Session {
    pub snapshot: Snapshot,
    pub baseline: Snapshot,
    pub source: Option<String>,
    pub selected: Option<String>,
    pub query: String,
    pub all: bool,
    pub family: Option<u8>,
    pub redact: bool,
    pub history: Vec<String>,
    pub pending: Option<Pending>,
}
pub enum Response {
    Text(String),
    Exit,
    External(Vec<String>),
    Apply(Vec<String>),
    Export(String, String),
}
impl Session {
    pub fn new(snapshot: Snapshot, source: Option<String>) -> Self {
        Self {
            baseline: snapshot.clone(),
            snapshot,
            source,
            selected: None,
            query: String::new(),
            all: false,
            family: None,
            redact: false,
            history: vec![],
            pending: None,
        }
    }
    pub fn filtered(&self) -> Snapshot {
        let q = self.query.to_lowercase();
        let mut s = self.snapshot.clone();
        s.interfaces.retain(|i| {
            self.selected.as_ref().map(|n| n == &i.name).unwrap_or(true)
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
        });
        s
    }
    pub fn display(&self, text: String) -> String {
        if self.redact {
            view::redact(text, &self.snapshot)
        } else {
            text
        }
    }
    pub fn execute(&mut self, input: &str, color: bool) -> Result<Response, String> {
        let mut words = split(input)?;
        if words.is_empty() {
            return Ok(Response::Text(String::new()));
        }
        if words[0].starts_with('!') {
            if words.len() != 1 {
                return Err("Use !NUMBER by itself".into());
            }
            let index: usize = words[0][1..]
                .parse()
                .map_err(|_| "Use !NUMBER from history")?;
            let old = self
                .history
                .get(index.checked_sub(1).ok_or("History begins at 1")?)
                .ok_or("No such history item")?
                .clone();
            words = split(&old)?;
            // Never let history recall authorize a mutation.
            if words.first().map(String::as_str) == Some("apply") {
                return Err(
                    "Type apply explicitly; mutation authorization cannot come from history".into(),
                );
            }
        }
        if words[0] != "history" {
            self.history.push(input.into());
            if self.history.len() > 500 {
                self.history.remove(0);
            }
        }
        let arg = |n: usize| words.get(n).map(String::as_str).ok_or("Missing argument");
        let mut output = match words[0].as_str() {
            "quit" | "q" | "exit" => return Ok(Response::Exit),
            "help" | "?" | "menu" => HELP.into(),
            "show" | "ls" | "1" => view::interfaces(
                &self.filtered(),
                &Filter {
                    all: self.all,
                    interface: self.selected.clone(),
                    family: self.family,
                    ..Default::default()
                },
                &Theme { color },
                &BTreeMap::new(),
            ),
            "select" => {
                let n = arg(1)?;
                if n == "all" {
                    self.selected = None;
                } else {
                    if !self.snapshot.interfaces.iter().any(|i| i.name == n) {
                        return Err("Unknown interface".into());
                    }
                    self.selected = Some(n.into());
                }
                self.pending = None;
                format!("Selection: {}", self.selected.as_deref().unwrap_or("all"))
            }
            "filter" => {
                self.query = words[1..].join(" ");
                self.pending = None;
                format!("Filter: {}", self.query)
            }
            "all" => {
                self.all = !self.all;
                self.pending = None;
                format!("Expand all groups: {}", self.all)
            }
            "family" => {
                self.family = match arg(1)? {
                    "4" => Some(4),
                    "6" => Some(6),
                    "all" => None,
                    _ => return Err("Choose family 4, 6, or all".into()),
                };
                self.pending = None;
                "Address filter updated".into()
            }
            "detail" | "2" => {
                let selected = self.selected.as_ref().ok_or("Use select NAME first")?;
                let i = self
                    .snapshot
                    .interfaces
                    .iter()
                    .find(|i| &i.name == selected)
                    .ok_or("Selected interface disappeared")?;
                insight::explain(i)
            }
            "doctor" | "3" => insight::findings(&self.snapshot).join("\n"),
            "topology" | "4" => explore::topology(&self.snapshot, false),
            "baseline" => {
                self.baseline = self.snapshot.clone();
                self.pending = None;
                "Baseline captured for this session.".into()
            }
            "changes" | "5" => {
                let d = insight::diff(&self.baseline, &self.snapshot);
                if d.is_empty() {
                    "No configuration changes from baseline.".into()
                } else {
                    d.join("\n")
                }
            }
            "report" => explore::report(&self.snapshot, false),
            "redact" => {
                self.redact = match arg(1)? {
                    "on" => true,
                    "off" => false,
                    _ => return Err("Use redact on|off".into()),
                };
                format!("IP/MAC masking: {}", self.redact)
            }
            "refresh" | "r" => {
                let s = load(self.source.as_deref())?;
                self.replace(s);
                "Capture refreshed; pending plan discarded.".into()
            }
            "history" => self
                .history
                .iter()
                .enumerate()
                .map(|(n, s)| format!("{}  {}", n + 1, s))
                .collect::<Vec<_>>()
                .join("\n"),
            "plan" => {
                self.pending = None;
                let selected = self.selected.as_ref().ok_or("Use select NAME first")?;
                let mut command = vec!["set".into(), selected.clone()];
                command.extend(words[1..].iter().cloned());
                let plan = cli::set_plan(&command, &self.snapshot)?;
                let preview = format!(
                    "Preview: /sbin/ifconfig {}\nThis can disconnect this Mac. {}",
                    plan.join(" "),
                    if self.source.is_some() {
                        "OFFLINE: application is disabled."
                    } else {
                        "Type apply, then confirm the exact command. Privileges must already be available."
                    }
                );
                self.pending = Some(Pending { args: plan });
                preview
            }
            "cancel" => {
                self.pending = None;
                "Pending plan discarded.".into()
            }
            "apply" => {
                if words.len() != 1 {
                    return Err("Type apply by itself".into());
                }
                if self.source.is_some() {
                    return Err("Offline captures cannot authorize settings changes".into());
                }
                if !cfg!(target_os = "macos") {
                    return Err("Applying settings requires macOS".into());
                }
                let plan = self
                    .pending
                    .take()
                    .ok_or("No pending plan; use plan first")?;
                return Ok(Response::Apply(plan.args));
            }
            "export" => {
                if words.len() != 2 {
                    return Err("Use export FILE (quote paths containing spaces)".into());
                }
                return Ok(Response::Export(
                    words[1].clone(),
                    self.display(self.snapshot.json()),
                ));
            }
            "routes" | "dns" | "wifi" | "services" | "neighbors" | "path" | "sockets" => {
                if self.source.is_some() {
                    return Err("Live system inspectors are disabled in an offline session".into());
                }
                return Ok(Response::External(words));
            }
            "subnet" | "probe" | "resolve" => {
                if words[0] != "subnet" && self.source.is_some() {
                    return Err("Network operations are disabled in an offline session".into());
                }
                return Ok(Response::External(words));
            }
            _ => {
                return Err(
                    "Unknown command; type help or menu. Shell syntax is not supported.".into(),
                )
            }
        };
        if self.redact {
            output = view::redact(output, &self.baseline);
            output = self.display(output);
        }
        Ok(Response::Text(output))
    }
    pub fn replace(&mut self, s: Snapshot) {
        self.snapshot = s;
        self.pending = None;
        if self
            .selected
            .as_ref()
            .map(|n| !self.snapshot.interfaces.iter().any(|i| &i.name == n))
            .unwrap_or(false)
        {
            self.selected = None;
        }
    }
}
pub fn split(input: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escape = false;
    let mut started = false;
    for c in input.chars() {
        if escape {
            current.push(c);
            escape = false;
            started = true;
            continue;
        }
        if c == '\\' && quote != Some('\'') {
            escape = true;
            started = true;
            continue;
        }
        if let Some(q) = quote {
            if c == q {
                quote = None;
            } else {
                current.push(c);
            }
            continue;
        }
        match c {
            '\'' | '"' => {
                quote = Some(c);
                started = true;
            }
            c if c.is_whitespace() => {
                if started {
                    out.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            '|' | ';' | '`' | '>' => {
                return Err("Shell operators are not supported".into());
            }
            _ => {
                current.push(c);
                started = true;
            }
        }
    }
    if quote.is_some() || escape {
        return Err("Unclosed quote or escape".into());
    }
    if started {
        out.push(current);
    }
    Ok(out)
}
pub fn load(source: Option<&str>) -> Result<Snapshot, String> {
    match source {
        Some(p) => {
            let text = std::fs::read_to_string(p).map_err(|e| e.to_string())?;
            if text.len() > 8 * 1024 * 1024 {
                return Err("Capture exceeds 8 MiB".into());
            }
            parse::ifconfig(&text)
        }
        None => collect::snapshot(),
    }
}
pub fn run(source: Option<String>, color: bool, redact: bool, all: bool) -> Result<(), String> {
    let mut session = Session::new(load(source.as_deref())?, source);
    session.redact = redact;
    session.all = all;
    let tty = io::stdin().is_terminal();
    let theme = Theme { color };
    println!(
        "{}\n{}\n{}",
        theme.title("MACORA · Interactive network workspace"),
        if session.source.is_some() {
            "OFFLINE CAPTURE · network operations and apply disabled"
        } else {
            "LIVE SOURCE · read-only until an explicit plan is confirmed"
        },
        HELP
    );
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut line = String::new();
    loop {
        if tty {
            print!(
                "{} ",
                theme.paint(
                    "1;36",
                    &format!("macora[{}] ›", session.selected.as_deref().unwrap_or("all"))
                )
            );
            io::stdout().flush().map_err(|e| e.to_string())?;
        }
        line.clear();
        if input.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
            break;
        }
        match session.execute(line.trim_end(), color) {
            Ok(Response::Exit) => break,
            Ok(Response::Text(s)) => {
                if !s.is_empty() {
                    println!("{s}");
                }
            }
            Ok(Response::External(args)) => {
                if let Err(e) = cli::run(args) {
                    eprintln!("{}", theme.paint("93", &e));
                }
            }
            Ok(Response::Export(path, text)) => {
                let mut options = OpenOptions::new();
                options.write(true).create_new(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.mode(0o600);
                }
                match options
                    .open(&path)
                    .and_then(|mut f| f.write_all(text.as_bytes()))
                {
                    Ok(_) => println!("Saved {}", view::safe(&path)),
                    Err(e) => eprintln!("Export failed: {e}"),
                }
            }
            Ok(Response::Apply(args)) => {
                if !tty {
                    eprintln!("Interactive application requires a terminal; plan discarded.");
                    continue;
                }
                let exact = format!("APPLY /sbin/ifconfig {}", args.join(" "));
                println!("Type exactly: {exact}");
                let mut confirmation = String::new();
                input
                    .read_line(&mut confirmation)
                    .map_err(|e| e.to_string())?;
                if confirmation.trim() == exact {
                    // Revalidate plan against fresh live inventory immediately before execution.
                    let mut words = vec!["set".to_string()];
                    words.extend(args);
                    words.push("--apply".into());
                    if let Err(e) = cli::run(words) {
                        eprintln!("Apply failed: {}", view::safe(&e));
                    }
                    match load(None) {
                        Ok(s) => session.replace(s),
                        Err(e) => eprintln!("Refresh failed: {}", view::safe(&e)),
                    }
                } else {
                    println!("Cancelled; no command executed.");
                }
            }
            Err(e) => eprintln!("{}", theme.paint("93", &e)),
        }
    }
    Ok(())
}
