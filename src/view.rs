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
        self.paint("1;38;5;208", text)
    }
    pub fn muted(&self, text: &str) -> String {
        self.paint("38;5;245", text)
    }
}
/// 256-color accents used by the interface view (ignored when color is off).
fn group_color(group: &str) -> &'static str {
    match group {
        "Network" => "1;38;5;39",
        "Bridges" => "1;38;5;208",
        "Virtual" => "1;38;5;141",
        "Tunnels" => "1;38;5;171",
        "System" => "1;38;5;220",
        _ => "1;38;5;110",
    }
}
fn scope_color(scope: &str) -> &'static str {
    match scope {
        "global-range" => "38;5;77",
        "private" => "38;5;214",
        "link-local" => "38;5;179",
        "loopback" => "38;5;110",
        _ => "38;5;245",
    }
}
fn attribute_color(attr: &str) -> &'static str {
    match attr {
        "deprecated" => "38;5;208",
        "temporary" => "38;5;75",
        "secured" => "38;5;78",
        "duplicated" | "tentative" => "1;38;5;196",
        _ => "38;5;245",
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
/// One indented output line.
fn nl(prefix: &str, body: String) -> String {
    format!("{prefix}{body}\n")
}
pub fn interfaces(
    s: &Snapshot,
    f: &Filter,
    t: &Theme,
    rates: &BTreeMap<String, (f64, f64)>,
) -> String {
    let active = s.interfaces.iter().filter(|i| i.active()).count();
    let tunnels = s
        .interfaces
        .iter()
        .filter(|i| i.group() == "Tunnels")
        .count();
    let mut out = format!(
        "{}  {}\n{} {} {} {} {} {}\n\n",
        t.title("MACORA"),
        t.muted("by Zyvor · Your Mac’s network, made clear."),
        t.paint("1;97", &s.interfaces.len().to_string()),
        t.muted("interfaces ·"),
        t.paint("1;38;5;46", &active.to_string()),
        t.muted("link-active ·"),
        t.paint("1;38;5;171", &tunnels.to_string()),
        t.muted("tunnels (not proof of an active VPN)")
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
                t.paint(
                    group_color(group),
                    &format!("{} · {} interfaces", group, items.len())
                ),
                t.muted("collapsed · run macora all")
            );
            continue;
        }
        out += &format!(
            "{}\n",
            t.paint(group_color(group), &format!("── {} ──", group))
        );
        for i in items {
            let state = if i.active() {
                t.paint("1;38;5;46", "● ACTIVE")
            } else if i.up() {
                t.paint("1;38;5;208", "○ IDLE")
            } else {
                t.paint("38;5;196", "○ DOWN")
            };
            out += &format!(
                "  {}  {}  {}  {}\n",
                t.paint("1;97", &format!("{:<10}", i.name)),
                state,
                t.paint("38;5;51", &i.label()),
                t.muted(&format!(
                    "MTU {}",
                    i.mtu.map(|n| n.to_string()).unwrap_or("?".into())
                ))
            );
            let width = i
                .addresses
                .iter()
                .map(|a| a.cidr().chars().count())
                .max()
                .unwrap_or(0)
                .clamp(24, 52);
            for a in &i.addresses {
                let v6 = a.ip.contains(':');
                if f.family == Some(4) && v6 || f.family == Some(6) && !v6 {
                    continue;
                }
                let code = if a.attributes.iter().any(|s| s == "deprecated") {
                    "38;5;243"
                } else if v6 {
                    "38;5;177"
                } else {
                    "1;38;5;45"
                };
                let attrs = a
                    .attributes
                    .iter()
                    .map(|x| t.paint(attribute_color(x), x))
                    .collect::<Vec<_>>()
                    .join(&t.muted(" · "));
                out += &format!(
                    "    {} {} {} {}\n",
                    t.paint(
                        if v6 { "38;5;98" } else { "38;5;38" },
                        if v6 { "IPv6" } else { "IPv4" }
                    ),
                    t.paint(code, &format!("{:<width$}", a.cidr(), width = width)),
                    t.paint(scope_color(a.scope()), &format!("{:<12}", a.scope())),
                    attrs
                );
            }
            if i.addresses.is_empty() {
                out += &format!("    {}\n", t.muted("No IP address assigned"));
            }
            if !f.compact {
                if let Some(mac) = &i.mac {
                    out += &nl(
                        "    ",
                        format!("{} {}", t.muted("MAC"), t.paint("38;5;105", mac)),
                    );
                }
                if let Some(media) = &i.media {
                    out += &nl(
                        "    ",
                        format!("{} {}", t.muted("Media"), t.paint("38;5;80", media)),
                    );
                }
                if !i.members.is_empty() {
                    out += &nl(
                        "    ",
                        format!(
                            "{} {}",
                            t.muted("Bridge members:"),
                            t.paint("1;38;5;214", &i.members.join(", "))
                        ),
                    );
                }
                if f.all || f.interface.is_some() {
                    out += &nl(
                        "    ",
                        format!(
                            "{} {}",
                            t.muted("Flags"),
                            t.paint("38;5;110", &i.flags.join(" · "))
                        ),
                    );
                    if !i.options.is_empty() {
                        out += &nl(
                            "    ",
                            format!(
                                "{} {}",
                                t.muted("Options"),
                                t.paint("38;5;110", &i.options.join(" · "))
                            ),
                        );
                    }
                }
                if let Some(c) = &i.counters {
                    out += &nl(
                        "    ",
                        format!(
                            "{} {} {} {} {}",
                            t.muted("Total"),
                            t.muted("RX"),
                            t.paint("1;38;5;46", &bytes(c.rx as f64)),
                            t.muted("· TX"),
                            t.paint("1;38;5;208", &bytes(c.tx as f64))
                        ),
                    );
                }
            }
            if let Some((rx, tx)) = rates.get(&i.name) {
                let bars = "▁▂▃▄▅▆▇█";
                let level = ((rx + tx + 1.0).log10() as usize).min(7);
                out += &format!(
                    "    {} RX {}/s   {} TX {}/s\n",
                    t.paint(
                        "1;38;5;46",
                        &bars.chars().nth(level).unwrap_or('▁').to_string()
                    ),
                    t.paint("1;38;5;46", &bytes(*rx)),
                    t.paint("1;38;5;208", "↑"),
                    t.paint("1;38;5;208", &bytes(*tx))
                );
            }
            out.push('\n');
        }
    }
    for w in &s.warnings {
        out += &format!("{}\n", t.paint("1;38;5;208", &format!("Warning: {w}")));
    }
    out += &nl(
        "",
        format!(
            "{} {} {} {} {} {} {}",
            t.muted("Explore:"),
            t.paint("38;5;51", "macora explain en0"),
            t.muted("·"),
            t.paint("38;5;51", "macora diagnose"),
            t.muted("·"),
            t.paint("38;5;51", "macora routes"),
            t.muted("· macora watch")
        ),
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
/// Like [`redact`], but keeps table columns aligned: `address/prefix` is replaced by the placeholder
/// plus its prefix, padded with spaces to the original width.
pub fn redact_aligned(mut text: String, s: &Snapshot) -> String {
    let mut cidrs: Vec<(String, String)> = Vec::new();
    for i in &s.interfaces {
        for a in &i.addresses {
            if a.prefix.is_some() {
                let mask = if a.ip.contains(':') {
                    "<IPv6>"
                } else {
                    "<IPv4>"
                };
                let original = a.cidr();
                let prefix = original[a.ip.len()..].to_string();
                let replacement = format!("{mask}{prefix}");
                let pad = original
                    .chars()
                    .count()
                    .saturating_sub(replacement.chars().count());
                cidrs.push((original, format!("{replacement}{}", " ".repeat(pad))));
            }
        }
    }
    cidrs.sort_by_key(|(o, _)| std::cmp::Reverse(o.len()));
    cidrs.dedup();
    for (original, replacement) in cidrs {
        text = text.replace(&original, &replacement);
    }
    redact(text, s)
}
/// Replaces `secret` in `text` only where it is a whole address, so `10.0.0.1` does not match inside
/// `10.0.0.13` and `fe80::1` does not match inside `fe80::1a`.
pub fn replace_addr(text: &str, secret: &str, mask: &str) -> String {
    if secret.is_empty() {
        return text.to_string();
    }
    let v6 = secret.contains(':');
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for (i, _) in text.match_indices(secret) {
        let end = i + secret.len();
        let before = text[..i].chars().next_back();
        let mut after = text[end..].chars();
        let next = after.next();
        let joined_before = before.is_some_and(|c| c.is_ascii_alphanumeric() || c == ':');
        let joined_after = match next {
            Some(c) if c.is_ascii_alphanumeric() => true,
            Some(':') => v6,
            Some('.') => after.next().is_some_and(|c| c.is_ascii_digit()),
            _ => false,
        };
        if i < last || joined_before || joined_after {
            continue;
        }
        out.push_str(&text[last..i]);
        out.push_str(mask);
        last = end;
    }
    out.push_str(&text[last..]);
    out
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
        text = replace_addr(&text, &secret, replacement);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::replace_addr;

    #[test]
    fn replace_addr_respects_address_boundaries() {
        let m = "<IPv4>";
        assert_eq!(
            replace_addr("gw 10.0.0.1 up", "10.0.0.1", m),
            "gw <IPv4> up"
        );
        assert_eq!(
            replace_addr("10.0.0.13 10.0.0.1.", "10.0.0.1", m),
            "10.0.0.13 <IPv4>."
        );
        assert_eq!(replace_addr("10.0.0.1:443", "10.0.0.1", m), "<IPv4>:443");
        assert_eq!(replace_addr("110.0.0.1", "10.0.0.1", m), "110.0.0.1");
        assert_eq!(
            replace_addr("fe80::1a fe80::1%en0", "fe80::1", "<IPv6>"),
            "fe80::1a <IPv6>%en0"
        );
    }
}
