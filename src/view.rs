use crate::model::{Interface, Snapshot};
use std::collections::BTreeMap;

pub fn safe(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_control() && c != '\n' {
                '�'
            } else {
                c
            }
        })
        .collect()
}
pub struct Theme {
    pub color: bool,
}
impl Theme {
    pub fn paint(&self, code: &str, text: &str) -> String {
        let text = safe(text);
        if self.color {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text
        }
    }
    pub fn title(&self, text: &str) -> String {
        self.paint("1;96", text)
    }
    pub fn muted(&self, text: &str) -> String {
        self.paint("90", text)
    }
}
#[derive(Default)]
pub struct Filter {
    pub all: bool,
    pub up: bool,
    pub interface: Option<String>,
    pub group: Option<String>,
    pub family: Option<u8>,
    pub compact: bool,
}
impl Filter {
    pub fn matches(&self, i: &Interface) -> bool {
        (!self.up || i.up())
            && self
                .interface
                .as_ref()
                .map(|s| *s == i.name)
                .unwrap_or(true)
            && self
                .group
                .as_ref()
                .map(|s| s.eq_ignore_ascii_case(i.group()))
                .unwrap_or(true)
    }
}
pub fn bytes(n: f64) -> String {
    if n >= 1024.0 * 1024.0 * 1024.0 {
        format!("{:.1} GiB", n / 1073741824.0)
    } else if n >= 1048576.0 {
        format!("{:.1} MiB", n / 1048576.0)
    } else if n >= 1024.0 {
        format!("{:.1} KiB", n / 1024.0)
    } else {
        format!("{:.0} B", n)
    }
}
pub fn interfaces(
    s: &Snapshot,
    f: &Filter,
    t: &Theme,
    rates: &BTreeMap<String, (f64, f64)>,
) -> String {
    let mut out = format!(
        "{}  {}\n{}\n\n",
        t.title("MACORA"),
        t.muted("by Zyvor · Your Mac’s network, made clear."),
        t.muted(&format!(
            "{} interfaces · {} link-active · {} tunnels (not proof of an active VPN)",
            s.interfaces.len(),
            s.interfaces.iter().filter(|i| i.active()).count(),
            s.interfaces
                .iter()
                .filter(|i| i.group() == "Tunnels")
                .count()
        ))
    );
    let groups = [
        "Network", "Bridges", "Virtual", "Tunnels", "System", "Loopback",
    ];
    for group in groups {
        let mut items: Vec<_> = s
            .interfaces
            .iter()
            .filter(|i| i.group() == group && f.matches(i))
            .collect();
        if items.is_empty() {
            continue;
        }
        items.sort_by_key(|i| (!i.active(), &i.name));
        let expand = f.all
            || f.interface.is_some()
            || f.group.is_some()
            || group == "Network"
            || group == "Bridges"
            || group == "Virtual";
        if !expand {
            out += &format!(
                "{}  {}\n",
                t.muted(&format!("{} · {} interfaces", group, items.len())),
                t.muted("collapsed · use --all")
            );
            continue;
        }
        out += &format!("{}\n", t.title(&format!("── {} ──", group)));
        for i in items {
            let state = if i.active() {
                t.paint("92", "● ACTIVE")
            } else if i.up() {
                t.paint("93", "○ IDLE")
            } else {
                t.muted("○ DOWN")
            };
            out += &format!(
                "  {}  {}  {}  {}\n",
                t.paint("1;97", &format!("{:<10}", i.name)),
                state,
                t.paint("36", &i.label()),
                t.muted(&format!(
                    "MTU {}",
                    i.mtu.map(|n| n.to_string()).unwrap_or("?".into())
                ))
            );
            for a in &i.addresses {
                let v6 = a.ip.contains(':');
                if f.family == Some(4) && v6 || f.family == Some(6) && !v6 {
                    continue;
                }
                let code = if a.attributes.iter().any(|s| s == "deprecated") {
                    "90"
                } else if v6 {
                    "95"
                } else {
                    "96"
                };
                out += &format!(
                    "    {} {:<44} {} {}\n",
                    if v6 { "IPv6" } else { "IPv4" },
                    t.paint(code, &a.cidr()),
                    t.muted(a.scope()),
                    t.muted(&a.attributes.join(" · "))
                );
            }
            if i.addresses.is_empty() {
                out += &format!("    {}\n", t.muted("No IP address assigned"));
            }
            if !f.compact {
                if let Some(mac) = &i.mac {
                    out += &format!("    {}\n", t.muted(&format!("MAC {mac}")));
                }
                if let Some(media) = &i.media {
                    out += &format!("    {}\n", t.muted(&format!("Media {media}")));
                }
                if !i.members.is_empty() {
                    out += &format!(
                        "    {}\n",
                        t.paint("33", &format!("Bridge members: {}", i.members.join(", ")))
                    );
                }
                if f.all || f.interface.is_some() {
                    out += &format!(
                        "    {}\n",
                        t.muted(&format!("Flags {}", i.flags.join(" · ")))
                    );
                    if !i.options.is_empty() {
                        out += &format!(
                            "    {}\n",
                            t.muted(&format!("Options {}", i.options.join(" · ")))
                        );
                    }
                }
                if let Some(c) = &i.counters {
                    out += &format!(
                        "    {}\n",
                        t.muted(&format!(
                            "Total RX {} · TX {}",
                            bytes(c.rx as f64),
                            bytes(c.tx as f64)
                        ))
                    );
                }
            }
            if let Some((rx, tx)) = rates.get(&i.name) {
                let bars = "▁▂▃▄▅▆▇█";
                let level = ((rx + tx + 1.0).log10() as usize).min(7);
                out += &format!(
                    "    {} RX {}/s   {} TX {}/s\n",
                    t.paint("92", &bars.chars().nth(level).unwrap_or('▁').to_string()),
                    t.paint("92", &bytes(*rx)),
                    t.paint("95", "↑"),
                    t.paint("95", &bytes(*tx))
                );
            }
            out.push('\n');
        }
    }
    for w in &s.warnings {
        out += &format!("{}\n", t.paint("93", &format!("Warning: {w}")));
    }
    out += &format!(
        "{}\n",
        t.muted("Explore: macora explain en0 · macora routes · macora doctor · macora watch")
    );
    out
}
pub fn rates(old: &Snapshot, new: &Snapshot, seconds: f64) -> BTreeMap<String, (f64, f64)> {
    let mut result = BTreeMap::new();
    if seconds <= 0.0 || !seconds.is_finite() {
        return result;
    }
    for i in &new.interfaces {
        if let (Some(a), Some(b)) = (
            old.interfaces
                .iter()
                .find(|n| n.name == i.name)
                .and_then(|i| i.counters.as_ref()),
            i.counters.as_ref(),
        ) {
            // A reset or wrap yields zero for that counter, never a giant artificial rate.
            result.insert(
                i.name.clone(),
                (
                    b.rx.saturating_sub(a.rx) as f64 / seconds,
                    b.tx.saturating_sub(a.tx) as f64 / seconds,
                ),
            );
        }
    }
    result
}
pub fn redact(mut text: String, s: &Snapshot) -> String {
    let mut secrets = Vec::new();
    for i in &s.interfaces {
        if let Some(m) = &i.mac {
            secrets.push((m.clone(), "<MAC>"));
        }
        for a in &i.addresses {
            secrets.push((
                a.ip.clone(),
                if a.ip.contains(':') {
                    "<IPv6>"
                } else {
                    "<IPv4>"
                },
            ));
        }
    }
    secrets.sort_by_key(|(s, _)| std::cmp::Reverse(s.len()));
    secrets.dedup();
    for (secret, replacement) in secrets {
        text = text.replace(&secret, replacement);
    }
    text
}
