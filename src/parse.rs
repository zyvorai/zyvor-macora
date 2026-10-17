use crate::model::{Address, Counters, Interface, Snapshot};
use std::collections::BTreeMap;
use std::net::Ipv4Addr;

pub fn mask_prefix(mask: &str) -> Option<u8> {
    let bits = if let Some(hex) = mask.strip_prefix("0x") {
        u32::from_str_radix(hex, 16).ok()?
    } else {
        u32::from(mask.parse::<Ipv4Addr>().ok()?)
    };
    let n = bits.leading_ones();
    if bits != if n == 0 { 0 } else { u32::MAX << (32 - n) } {
        return None;
    }
    Some(n as u8)
}
fn after<'a>(words: &[&'a str], key: &str) -> Option<&'a str> {
    words
        .iter()
        .position(|s| *s == key)
        .and_then(|n| words.get(n + 1))
        .copied()
}
fn angle(s: &str) -> Vec<String> {
    s.split_once('<')
        .and_then(|(_, tail)| tail.split_once('>'))
        .map(|(flags, _)| flags.split(',').map(str::to_string).collect())
        .unwrap_or_default()
}
pub fn ifconfig(text: &str) -> Result<Snapshot, String> {
    let mut snapshot = Snapshot::default();
    let mut current: Option<Interface> = None;
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if !line.starts_with(char::is_whitespace) && line.contains(": flags=") {
            if let Some(i) = current.take() {
                snapshot.interfaces.push(i);
            }
            let name = line.split(':').next().unwrap_or("").to_string();
            if name.is_empty() || name.chars().any(char::is_control) {
                return Err("Invalid interface header".into());
            }
            let words: Vec<_> = line.split_whitespace().collect();
            current = Some(Interface {
                name,
                flags: angle(line),
                mtu: after(&words, "mtu").and_then(|n| n.parse().ok()),
                raw: vec![line.into()],
                ..Default::default()
            });
            continue;
        }
        let Some(i) = current.as_mut() else {
            continue;
        };
        i.raw.push(line.into());
        let words: Vec<_> = line.split_whitespace().collect();
        match words.first().copied().unwrap_or("") {
            "inet" | "inet6" => {
                if let Some(ip) = words.get(1) {
                    let mut attributes = Vec::new();
                    for a in [
                        "secured",
                        "autoconf",
                        "temporary",
                        "deprecated",
                        "tentative",
                        "duplicated",
                        "detached",
                    ] {
                        if words.contains(&a) {
                            attributes.push(a.into());
                        }
                    }
                    let prefix = if words[0] == "inet" {
                        after(&words, "netmask").and_then(mask_prefix)
                    } else {
                        after(&words, "prefixlen")
                            .and_then(|n| n.parse::<u8>().ok())
                            .filter(|n| *n <= 128)
                    };
                    if ip
                        .split('%')
                        .next()
                        .unwrap_or("")
                        .parse::<std::net::IpAddr>()
                        .is_ok()
                    {
                        i.addresses.push(Address {
                            ip: ip.to_string(),
                            prefix,
                            attributes,
                        });
                    } else {
                        snapshot
                            .warnings
                            .push(format!("Ignored invalid address on {}", i.name));
                    }
                }
            }
            "ether" | "lladdr" => i.mac = words.get(1).map(|s| s.to_string()),
            "status:" => i.status = words.get(1).map(|s| s.to_string()),
            "media:" => i.media = Some(words[1..].join(" ")),
            "member:" => {
                if let Some(name) = words.get(1) {
                    i.members.push(name.to_string());
                }
            }
            s if s.starts_with("options=") || s.starts_with("nd6") => i.options.extend(angle(line)),
            _ => {}
        }
    }
    if let Some(i) = current {
        snapshot.interfaces.push(i);
    }
    if snapshot.interfaces.is_empty() {
        return Err("No interface records found; expected macOS `ifconfig -a` output".into());
    }
    snapshot.interfaces.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(snapshot)
}
/// Parse by column names: macOS may insert Address, Ierrs, Idrop, and other columns.
/// Keep the link-layer row only; IP rows repeat the same interface's counters.
pub fn netstat_counters(text: &str) -> BTreeMap<String, Counters> {
    let mut result = BTreeMap::new();
    let mut header: Vec<&str> = Vec::new();
    for line in text.lines() {
        let w: Vec<_> = line.split_whitespace().collect();
        if w.first() == Some(&"Name") {
            header = w;
            continue;
        }
        if header.is_empty() || !w.iter().any(|s| s.starts_with("<Link#")) {
            continue;
        }
        let rx = header.iter().position(|s| *s == "Ibytes");
        let tx = header.iter().position(|s| *s == "Obytes");
        // Link rows with no MAC omit the Address field. Adjust columns after Address.
        let missing_address = header.contains(&"Address") && w.len() + 1 == header.len();
        let get = |p: Option<usize>| {
            p.and_then(|n| {
                let n = if missing_address
                    && n > header
                        .iter()
                        .position(|s| *s == "Address")
                        .unwrap_or(usize::MAX)
                {
                    n - 1
                } else {
                    n
                };
                w.get(n)?.parse::<u64>().ok()
            })
        };
        if let (Some(name), Some(rx), Some(tx)) = (w.first(), get(rx), get(tx)) {
            result.insert(name.trim_end_matches('*').into(), Counters { rx, tx });
        }
    }
    result
}
pub fn hardware_ports(text: &str) -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();
    let mut label = "";
    for line in text.lines() {
        if let Some(s) = line.strip_prefix("Hardware Port: ") {
            label = s;
        }
        if let Some(s) = line.strip_prefix("Device: ") {
            if !label.is_empty() {
                result.insert(s.into(), label.into());
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn masks_validate_contiguity() {
        assert_eq!(mask_prefix("0xffffff00"), Some(24));
        assert_eq!(mask_prefix("255.255.255.0"), Some(24));
        assert_eq!(mask_prefix("0x0"), Some(0));
        assert_eq!(mask_prefix("255.255.255.255"), Some(32));
        assert_eq!(mask_prefix("255.0.255.0"), None);
    }
    #[test]
    fn reject_garbage() {
        assert!(ifconfig("not an interface").is_err());
    }
    #[test]
    fn counters_link_rows_and_missing_mac() {
        let s = "Name Mtu Network Address Ipkts Ierrs Ibytes Opkts Oerrs Obytes Coll\nen0 1500 <Link#1> aa:bb:cc:dd:ee:ff 2 0 300 4 0 500 0\nlo0 16384 <Link#2> 2 0 700 3 0 800 0\nen0 1500 10.0.0 10.0.0.2 2 0 300 4 0 500 0";
        let c = netstat_counters(s);
        assert_eq!(c["en0"].rx, 300);
        assert_eq!(c["lo0"].tx, 800);
    }
}
