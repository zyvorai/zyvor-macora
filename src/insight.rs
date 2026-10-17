use crate::model::{json_list, Interface, Snapshot};
use std::net::Ipv4Addr;

pub fn findings(s: &Snapshot) -> Vec<String> {
    let mut out = Vec::new();
    let useful: Vec<_> = s
        .interfaces
        .iter()
        .filter(|i| i.group() == "Network" && i.active())
        .collect();
    if useful.is_empty() {
        out.push(
            "WARN: No link-active network adapter detected. Check Wi-Fi or your cable.".into(),
        );
    }
    for i in useful {
        if i.addresses.is_empty() {
            out.push(format!(
                "WARN: {} has an active link but no IP address.",
                i.name
            ));
        }
        if i.addresses.iter().any(|a| a.ip.starts_with("169.254.")) {
            out.push(format!(
                "WARN: {} has a self-assigned IPv4 address; DHCP may be unavailable.",
                i.name
            ));
        }
        if !i.addresses.is_empty() && i.addresses.iter().all(|a| a.scope() == "link-local") {
            out.push(format!(
                "INFO: {} only has link-local addresses; check address assignment.",
                i.name
            ));
        }
    }
    for i in &s.interfaces {
        if i.addresses.iter().any(|a| {
            a.attributes
                .iter()
                .any(|s| s == "duplicated" || s == "tentative")
        }) {
            out.push(format!(
                "WARN: {} has an IPv6 address still being checked or marked duplicate.",
                i.name
            ));
        }
        if i.addresses
            .iter()
            .any(|a| a.attributes.iter().any(|s| s == "deprecated"))
        {
            out.push(format!("INFO: {} has deprecated IPv6 addresses; these are usually part of normal address rotation.", i.name));
        }
        if i.group() == "Tunnels" && i.mtu.unwrap_or(1500) < 1280 {
            out.push(format!(
                "INFO: {} has MTU below 1280; effective tunnel behavior depends on its owner.",
                i.name
            ));
        }
        if !i.members.is_empty() && !i.active() {
            out.push(format!(
                "INFO: {} is an inactive bridge; unused Thunderbolt bridges can be normal.",
                i.name
            ));
        }
    }
    if !s.interfaces.iter().any(|i| {
        i.addresses
            .iter()
            .any(|a| a.ip.contains(':') && a.scope() == "global-range")
    }) {
        out.push(
            "INFO: No global-range IPv6 address detected. IPv4-only networks are valid.".into(),
        );
    }
    if out.is_empty() {
        out.push(
            "OK: No local interface issues detected. This does not verify internet reachability."
                .into(),
        );
    }
    out
}
pub fn explain(i: &Interface) -> String {
    let mut s = format!(
        "{} — {}\nGroup: {}. Administrative state: {}. Link state: {}.\n",
        i.name,
        i.label(),
        i.group(),
        if i.up() { "up" } else { "down" },
        if i.active() { "active" } else { "idle/down" }
    );
    for flag in &i.flags {
        let meaning = match flag.as_str() {
            "UP" => "Administratively enabled; this alone does not prove a working connection.",
            "RUNNING" => "Driver reports interface resources ready; not an internet health check.",
            "LOOPBACK" => "Traffic stays on this Mac.",
            "BROADCAST" => "Supports link broadcast traffic.",
            "MULTICAST" => "Supports one-to-many traffic such as local service discovery.",
            "POINTOPOINT" => "Point-to-point link, often used by tunnels.",
            "PROMISC" => {
                "Receives frames beyond its own MAC; bridges and capture tools may enable this."
            }
            "SIMPLEX" => "Does not hear its own transmitted frames.",
            "SMART" => "Driver manages some interface resources automatically.",
            _ => "OS/driver flag; retained as reported by ifconfig.",
        };
        s += &format!("  {flag}: {meaning}\n");
    }
    match i.group() {
        "Tunnels" => s += "Tunnel presence does not identify its owner or prove a VPN is connected. macOS services and network extensions can create utun interfaces.\n",
        "Bridges" => s += "A bridge joins member interfaces at layer 2. An idle Thunderbolt bridge is common when no peer is attached.\n",
        "System" => s += "Apple system and peer interfaces support local discovery and device features; they are separate from the primary internet adapter.\n",
        _ => {},
    }
    s += "IPv6: fe80::/10 is link-local; %en0 is the interface scope, not part of the address. Temporary addresses support privacy; deprecated means avoid selecting it for new connections, not necessarily a broken connection.\nGlobal-range labels describe address scope, not verified public internet access.\n";
    s
}
pub fn diff(a: &Snapshot, b: &Snapshot) -> Vec<String> {
    let mut changes = Vec::new();
    for old in &a.interfaces {
        if !b.interfaces.iter().any(|i| i.name == old.name) {
            changes.push(format!("REMOVED {}", old.name));
        }
    }
    for new in &b.interfaces {
        match a.interfaces.iter().find(|i| i.name == new.name) {
            None => changes.push(format!("ADDED {}", new.name)),
            Some(old) => {
                if old.flags != new.flags || old.status != new.status {
                    changes.push(format!(
                        "STATE {}: {:?} -> {:?}; flags {:?} -> {:?}",
                        new.name, old.status, new.status, old.flags, new.flags
                    ));
                }
                if old.mtu != new.mtu {
                    changes.push(format!("MTU {}: {:?} -> {:?}", new.name, old.mtu, new.mtu));
                }
                if old.mac != new.mac {
                    changes.push(format!("MAC {}: {:?} -> {:?}", new.name, old.mac, new.mac));
                }
                if old.members != new.members {
                    changes.push(format!(
                        "MEMBERS {}: {:?} -> {:?}",
                        new.name, old.members, new.members
                    ));
                }
                if old.media != new.media {
                    changes.push(format!(
                        "MEDIA {}: {:?} -> {:?}",
                        new.name, old.media, new.media
                    ));
                }
                for addr in &old.addresses {
                    if !new.addresses.contains(addr) {
                        changes.push(format!(
                            "ADDRESS- {} {} {}",
                            new.name,
                            addr.cidr(),
                            addr.attributes.join(",")
                        ));
                    }
                }
                for addr in &new.addresses {
                    if !old.addresses.contains(addr) {
                        changes.push(format!(
                            "ADDRESS+ {} {} {}",
                            new.name,
                            addr.cidr(),
                            addr.attributes.join(",")
                        ));
                    }
                }
            }
        }
    }
    changes
}
pub fn subnet(cidr: &str, json: bool) -> Result<String, String> {
    let (ip, p) = cidr
        .split_once('/')
        .ok_or("Use IPv4 CIDR, e.g. 192.168.1.8/24")?;
    let ip = ip.parse::<Ipv4Addr>().map_err(|_| "Invalid IPv4 address")?;
    let p: u8 = p.parse().map_err(|_| "Invalid prefix")?;
    if p > 32 {
        return Err("IPv4 prefix must be 0..32".into());
    }
    let mask = if p == 0 { 0 } else { u32::MAX << (32 - p) };
    let network = u32::from(ip) & mask;
    let last = network | !mask;
    let count = 1u64 << (32 - p);
    let (first_host, last_host, hosts) = if p >= 31 {
        (network, last, count)
    } else {
        (network + 1, last - 1, count - 2)
    };
    if json {
        Ok(format!("{{\"network\":\"{}/{}\",\"netmask\":\"{}\",\"last_address\":\"{}\",\"first_host\":\"{}\",\"last_host\":\"{}\",\"usable_hosts\":{}}}", Ipv4Addr::from(network), p, Ipv4Addr::from(mask), Ipv4Addr::from(last), Ipv4Addr::from(first_host), Ipv4Addr::from(last_host), hosts))
    } else {
        Ok(format!("Network       {}/{}\nNetmask       {}\nLast address  {}\nHost range    {} – {}\nUsable hosts  {}\n/31 uses point-to-point semantics; /32 is one host.\n", Ipv4Addr::from(network), p, Ipv4Addr::from(mask), Ipv4Addr::from(last), Ipv4Addr::from(first_host), Ipv4Addr::from(last_host), hosts))
    }
}
pub fn list_json(key: &str, items: &[String]) -> String {
    format!("{{\"{key}\":{}}}", json_list(items))
}
