//! Output formats beyond text/JSON: standalone HTML report, Prometheus text, shell completions.
use crate::{insight, model::Snapshot, view};
use std::collections::BTreeMap;

pub const COMMANDS: &[&str] = &[
    "interactive",
    "repl",
    "tui",
    "topology",
    "report",
    "resolve",
    "show",
    "explain",
    "routes",
    "path",
    "dns",
    "services",
    "wifi",
    "neighbors",
    "sockets",
    "doctor",
    "diagnose",
    "ping",
    "trace",
    "watch",
    "snapshot",
    "diff",
    "subnet",
    "probe",
    "set",
    "scan",
    "apps",
    "lan",
    "mtr",
    "lookup",
    "firewall",
    "throughput",
    "completions",
    "init",
];
/// One-line descriptions shown next to each command by the zsh completion (same order as `COMMANDS`).
const COMMAND_HELP: &[&str] = &[
    "persistent exploration with history and plan previews",
    "persistent exploration with history and plan previews",
    "full-screen terminal dashboard",
    "bridge membership and interface roles",
    "local-evidence report (json, html also work)",
    "DNS lookup via dscacheutil",
    "grouped interfaces",
    "explain flags, IPv6 and roles of an interface",
    "IPv4 and IPv6 routing tables",
    "default route and reachability state",
    "all scoped DNS resolvers",
    "network service order and hardware ports",
    "Wi-Fi power, association and preferred networks",
    "ARP and IPv6 neighbor tables",
    "TCP and UDP sockets visible to you",
    "local interface findings",
    "why is my network slow?",
    "packet loss and latency",
    "hop-by-hop path via traceroute",
    "live counters and traffic rates",
    "save a private ifconfig capture",
    "compare two ifconfig captures",
    "network, mask and host range of IPv4/PREFIX",
    "TCP reachability check of IP:PORT",
    "preview an interface state or MTU change",
    "nearby Wi-Fi networks and channel crowding",
    "traffic by process",
    "devices on your local network",
    "per-hop loss and latency",
    "compare DNS answers across resolvers",
    "application firewall and pf status",
    "LAN speed against an iperf3 server",
    "print bash, zsh or fish completions",
    "shell setup: completions and suggestions, for eval",
];
/// Option words accepted after a command or interface (see `cli::words_to_flags`).
pub const WORDS: &[&str] = &[
    "all",
    "up",
    "down",
    "list",
    "compact",
    "ipv4",
    "ipv6",
    "raw",
    "json",
    "redact",
    "html",
    "md",
    "prometheus",
    "apply",
    "from",
    "interface",
    "group",
    "count",
    "interval",
    "timeout",
    "format",
    "theme",
    "color",
    "speed",
    "quick",
    "strict",
    "to",
    "type",
];
pub const FORMATS: &[&str] = &["table", "json", "ndjson", "md", "html", "prometheus"];
pub const THEMES: &[&str] = &["default", "mono", "high-contrast"];

pub fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c if c.is_control() && c != '\n' => out.push('\u{fffd}'),
            c => out.push(c),
        }
    }
    out
}

/// A self-contained, script-free HTML evidence report (inline CSS, light/dark aware).
pub fn html_report(s: &Snapshot) -> String {
    let mut rows = String::new();
    for i in &s.interfaces {
        let addresses = if i.addresses.is_empty() {
            "—".to_string()
        } else {
            i.addresses
                .iter()
                .map(|a| format!("{} <small>({})</small>", escape_html(&a.cidr()), a.scope()))
                .collect::<Vec<_>>()
                .join("<br>")
        };
        rows +=
            &format!(
            "<tr><td><b>{}</b><br><small>{}</small></td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
            escape_html(&i.name),
            escape_html(&i.label()),
            if i.active() { "link-active" } else { "idle/down" },
            i.mtu.map_or("—".to_string(), |m| m.to_string()),
            addresses
        );
    }
    let findings: String = insight::findings(s)
        .iter()
        .map(|f| {
            let class = if f.starts_with("WARN") {
                "warn"
            } else {
                "info"
            };
            format!("<li class=\"{class}\">{}</li>\n", escape_html(f))
        })
        .collect();
    format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\n<title>MACORA network evidence</title>\n<style>\n:root{{--bg:#fff;--fg:#1d1d1f;--mut:#6e6e73;--line:#d2d2d7;--acc:#0071e3;--warn:#b25000}}\n@media(prefers-color-scheme:dark){{:root{{--bg:#111;--fg:#f5f5f7;--mut:#a1a1a6;--line:#38383a;--acc:#2997ff;--warn:#ffb340}}}}\nbody{{font:15px/1.5 -apple-system,system-ui,sans-serif;background:var(--bg);color:var(--fg);max-width:60rem;margin:2rem auto;padding:0 1rem}}\nh1{{margin-bottom:0}}p.sub{{color:var(--mut);margin-top:.25rem}}\ntable{{border-collapse:collapse;width:100%}}td,th{{border-bottom:1px solid var(--line);padding:.5rem;text-align:left;vertical-align:top}}\nsmall{{color:var(--mut)}}li.warn{{color:var(--warn)}}\n</style></head><body>\n<h1>MACORA network evidence</h1>\n<p class=\"sub\">Local interface capture. This is not an internet reachability test or a security certification.</p>\n<h2>Interfaces</h2>\n<table><tr><th>Interface</th><th>State</th><th>MTU</th><th>Addresses</th></tr>\n{rows}</table>\n<h2>Findings</h2>\n<ul>\n{findings}</ul>\n<p class=\"sub\">Generated by MACORA. IP/MAC redaction does not anonymize labels or arbitrary text.</p>\n</body></html>"
    )
}

fn label_value(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

/// Prometheus text exposition format 0.0.4.
pub fn prometheus(s: &Snapshot, rates: &BTreeMap<String, (f64, f64)>) -> String {
    let mut out = String::new();
    out += "# HELP macora_interface_up 1 if the interface is administratively up.\n# TYPE macora_interface_up gauge\n";
    for i in &s.interfaces {
        out += &format!(
            "macora_interface_up{{interface=\"{}\"}} {}\n",
            label_value(&i.name),
            i.up() as u8
        );
    }
    out += "# HELP macora_interface_link_active 1 if the link is active.\n# TYPE macora_interface_link_active gauge\n";
    for i in &s.interfaces {
        out += &format!(
            "macora_interface_link_active{{interface=\"{}\"}} {}\n",
            label_value(&i.name),
            i.active() as u8
        );
    }
    out += "# HELP macora_interface_receive_bytes_total Bytes received.\n# TYPE macora_interface_receive_bytes_total counter\n";
    for i in &s.interfaces {
        if let Some(c) = &i.counters {
            out += &format!(
                "macora_interface_receive_bytes_total{{interface=\"{}\"}} {}\n",
                label_value(&i.name),
                c.rx
            );
        }
    }
    out += "# HELP macora_interface_transmit_bytes_total Bytes transmitted.\n# TYPE macora_interface_transmit_bytes_total counter\n";
    for i in &s.interfaces {
        if let Some(c) = &i.counters {
            out += &format!(
                "macora_interface_transmit_bytes_total{{interface=\"{}\"}} {}\n",
                label_value(&i.name),
                c.tx
            );
        }
    }
    if !rates.is_empty() {
        out += "# HELP macora_interface_receive_bytes_per_second Receive rate since the previous sample.\n# TYPE macora_interface_receive_bytes_per_second gauge\n";
        for (name, (rx, _)) in rates {
            out += &format!(
                "macora_interface_receive_bytes_per_second{{interface=\"{}\"}} {rx:.2}\n",
                label_value(name)
            );
        }
        out += "# HELP macora_interface_transmit_bytes_per_second Transmit rate since the previous sample.\n# TYPE macora_interface_transmit_bytes_per_second gauge\n";
        for (name, (_, tx)) in rates {
            out += &format!(
                "macora_interface_transmit_bytes_per_second{{interface=\"{}\"}} {tx:.2}\n",
                label_value(name)
            );
        }
    }
    view::safe(&out)
}

const GROUPS: &str = "Network Bridges Virtual Tunnels System Loopback";

fn fill(template: &str) -> String {
    template
        .replace("@CMDS@", &COMMANDS.join(" "))
        .replace("@WORDS@", &WORDS.join(" "))
        .replace("@FORMATS@", &FORMATS.join(" "))
        .replace("@THEMES@", &THEMES.join(" "))
        .replace("@GROUPS@", GROUPS)
}

const BASH: &str = r#"_macora() {
  local cur="${COMP_WORDS[COMP_CWORD]}" prev="${COMP_WORDS[COMP_CWORD-1]}" cmd="${COMP_WORDS[1]}"
  case "$prev" in
    format) COMPREPLY=($(compgen -W "@FORMATS@" -- "$cur")); return;;
    theme) COMPREPLY=($(compgen -W "@THEMES@" -- "$cur")); return;;
    color) COMPREPLY=($(compgen -W "auto always never" -- "$cur")); return;;
    group) COMPREPLY=($(compgen -W "@GROUPS@" -- "$cur")); return;;
    type) COMPREPLY=($(compgen -W "A AAAA MX TXT NS CNAME SOA CAA PTR SRV" -- "$cur")); return;;
    from) COMPREPLY=($(compgen -f -- "$cur")); return;;
    to) COMPREPLY=($(compgen -A hostname -- "$cur")); return;;
    count|interval|timeout) return;;
    interface|explain|set) COMPREPLY=($(compgen -W "$(macora list 2>/dev/null)" -- "$cur")); return;;
  esac
  if [[ $COMP_CWORD -eq 2 ]]; then
    case "$cmd" in
      ping|trace|resolve|mtr|lookup|throughput) COMPREPLY=($(compgen -A hostname -- "$cur")); return;;
      snapshot|diff) COMPREPLY=($(compgen -f -- "$cur")); return;;
      completions|init) COMPREPLY=($(compgen -W "bash zsh fish" -- "$cur")); return;;
      subnet|probe) return;;
    esac
  fi
  if [[ $COMP_CWORD -eq 1 ]]; then
    COMPREPLY=($(compgen -W "@CMDS@ @WORDS@ $(macora list 2>/dev/null)" -- "$cur"))
  else
    COMPREPLY=($(compgen -W "@WORDS@ $(macora list 2>/dev/null)" -- "$cur"))
  fi
}
complete -F _macora macora
"#;

/// The zsh completion function, without the `#compdef` header or the trailing call, so it can be
/// autoloaded from `fpath` (`completions zsh`) or evaluated directly (`init zsh`).
fn zsh_function() -> String {
    let described: String = COMMANDS
        .iter()
        .zip(COMMAND_HELP)
        .map(|(c, h)| format!("    '{c}:{h}'\n"))
        .collect();
    fill(
        r#"_macora() {
  local -a cmds opts ifaces
  cmds=(
@DESCRIBED@  )
  opts=(@WORDS@)
  ifaces=(${(f)"$(macora list 2>/dev/null)"})
  case $words[CURRENT-1] in
    format) compadd -- @FORMATS@; return;;
    theme) compadd -- @THEMES@; return;;
    color) compadd -- auto always never; return;;
    group) compadd -- @GROUPS@; return;;
    type) compadd -- A AAAA MX TXT NS CNAME SOA CAA PTR SRV; return;;
    from) _files; return;;
    to) _hosts; return;;
    count|interval|timeout) _message -e numbers 'number'; return;;
    interface) compadd -- $ifaces; return;;
  esac
  case $words[2] in
    ping|trace|resolve|mtr|lookup|throughput) (( CURRENT == 3 )) && { _hosts; return };;
    snapshot) (( CURRENT == 3 )) && { _files; return };;
    diff) (( CURRENT <= 4 )) && { _files; return };;
    subnet) (( CURRENT == 3 )) && { _message 'IPv4/PREFIX, for example 192.168.1.0/24'; return };;
    probe) (( CURRENT == 3 )) && { _message 'IP:PORT, for example 1.1.1.1:443'; return };;
    completions|init) (( CURRENT == 3 )) && { compadd -- bash zsh fish; return };;
    explain) (( CURRENT == 3 )) && { compadd -- $ifaces; return };;
    set)
      if (( CURRENT == 3 )); then compadd -- $ifaces; return
      elif (( CURRENT == 4 )); then compadd -- up down mtu; return
      elif [[ $words[CURRENT-1] == mtu ]]; then _message -e numbers 'MTU'; return
      fi
      compadd -- apply; return;;
  esac
  if (( CURRENT == 2 )); then
    _describe -t commands 'command' cmds
    _describe -t interfaces 'interface' ifaces
    compadd -X 'word' -- $opts
  else
    compadd -X 'word' -- $opts
    compadd -X 'interface' -- $ifaces
  fi
}
"#,
    )
    .replace("@DESCRIBED@", &described)
}

/// Inline, history-free suggestions in the style of zsh-autosuggestions, limited to macora's own
/// grammar. Skipped when zsh-autosuggestions is loaded, which already owns POSTDISPLAY.
const ZSH_SUGGEST: &str = r#"
typeset -ga __macora_vocab
__macora_vocab=(@CMDS@ @WORDS@)
# Prints the part of the last word still to be typed, or nothing when there is no unique-first match.
__macora_suggestion() {
  [[ $1 == macora\ * && $1 != *\  ]] || return 1
  local last=${1##* } w
  [[ -n $last ]] || return 1
  for w in $__macora_vocab; do
    [[ $w == ${last}?* ]] && { print -r -- ${w#$last}; return 0 }
  done
  return 1
}
if (( ! $+functions[_zsh_autosuggest_start] )) && autoload -Uz add-zle-hook-widget 2>/dev/null; then
  __macora_redraw() {
    POSTDISPLAY=
    (( CURSOR == $#BUFFER )) || return 0
    local rest
    rest=$(__macora_suggestion "$BUFFER") || return 0
    POSTDISPLAY=$rest
    region_highlight+=("$#BUFFER $(( $#BUFFER + $#rest )) fg=8")
  }
  __macora_accept() {
    if [[ -n $POSTDISPLAY ]]; then BUFFER+=$POSTDISPLAY; CURSOR=$#BUFFER; POSTDISPLAY=
    else zle forward-char; fi
  }
  zle -N __macora_accept
  add-zle-hook-widget line-pre-redraw __macora_redraw
  bindkey '^[[C' __macora_accept 2>/dev/null
  bindkey '^[OC' __macora_accept 2>/dev/null
fi
"#;

fn zsh_init() -> String {
    let mut out = zsh_function();
    out += r#"if (( $+functions[compdef] )); then
  compdef _macora macora
else
  # compinit has not run yet: register on the first prompt, then remove this hook.
  __macora_late_init() {
    (( $+functions[compdef] )) && compdef _macora macora
    precmd_functions=(${precmd_functions:#__macora_late_init})
  }
  precmd_functions+=(__macora_late_init)
fi
"#;
    out + &fill(ZSH_SUGGEST)
}

fn fish_script() -> String {
    let mut out = String::from("complete -c macora -f\n");
    for (c, h) in COMMANDS.iter().zip(COMMAND_HELP) {
        out += &format!("complete -c macora -n '__fish_use_subcommand' -a '{c}' -d '{h}'\n");
    }
    out += "complete -c macora -n '__fish_use_subcommand' -a '(macora list 2>/dev/null)' -d interface\n";
    out += &fill(
        "complete -c macora -n 'not __fish_use_subcommand; and not __fish_prev_arg_in to' -a '@WORDS@' -d word
complete -c macora -n 'not __fish_use_subcommand; and not __fish_prev_arg_in to' -a '(macora list 2>/dev/null)' -d interface
complete -c macora -n '__fish_prev_arg_in to' -a '(__fish_print_hostnames)'
complete -c macora -n '__fish_seen_subcommand_from ping trace resolve mtr lookup throughput' -a '(__fish_print_hostnames)'
complete -c macora -n '__fish_seen_subcommand_from snapshot diff' -F
complete -c macora -n '__fish_seen_subcommand_from completions init' -a 'bash zsh fish'
complete -c macora -n '__fish_prev_arg_in format' -a '@FORMATS@'
complete -c macora -n '__fish_prev_arg_in theme' -a '@THEMES@'
complete -c macora -n '__fish_prev_arg_in color' -a 'auto always never'
complete -c macora -n '__fish_prev_arg_in group' -a '@GROUPS@'
complete -c macora -n '__fish_prev_arg_in type' -a 'A AAAA MX TXT NS CNAME SOA CAA PTR SRV'
complete -c macora -n '__fish_prev_arg_in from' -F
",
    );
    out
}

pub fn completions(shell: &str) -> Result<String, String> {
    match shell {
        "bash" => Ok(fill(BASH)),
        "zsh" => Ok(format!(
            "#compdef macora\n{}_macora \"$@\"\n",
            zsh_function()
        )),
        "fish" => Ok(fish_script()),
        _ => Err("Shell must be bash, zsh or fish".into()),
    }
}

/// `eval "$(macora init zsh)"` (or `macora init fish | source`): completions plus, for zsh,
/// inline suggestions. Completions are registered without compinit having to know about them.
pub fn init(shell: &str) -> Result<String, String> {
    match shell {
        "zsh" => Ok(zsh_init()),
        "bash" | "fish" => completions(shell),
        _ => Err("Shell must be bash, zsh or fish".into()),
    }
}

/// The closest candidate to `typed`: a unique prefix match, or one within two edits.
pub fn did_you_mean<'a>(
    typed: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Option<String> {
    let typed = typed.to_ascii_lowercase();
    if typed.is_empty() {
        return None;
    }
    let all: Vec<&str> = candidates.into_iter().collect();
    let prefixed: Vec<&&str> = all.iter().filter(|c| c.starts_with(&typed)).collect();
    if prefixed.len() == 1 {
        return Some(prefixed[0].to_string());
    }
    let limit = if typed.len() <= 4 { 1 } else { 2 };
    all.iter()
        .map(|c| (edit_distance(&typed, c), *c))
        .filter(|(d, _)| *d <= limit)
        .min_by_key(|(d, _)| *d)
        .map(|(_, c)| c.to_string())
}

fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cur = row[j + 1];
            row[j + 1] = (prev + usize::from(ca != *cb)).min(row[j] + 1).min(cur + 1);
            prev = cur;
        }
    }
    row[b.len()]
}
