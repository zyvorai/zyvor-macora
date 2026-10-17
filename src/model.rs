use std::net::IpAddr;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Address {
    pub ip: String,
    pub prefix: Option<u8>,
    pub attributes: Vec<String>,
}
impl Address {
    pub fn scope(&self) -> &'static str {
        match self.ip.split('%').next().unwrap_or("").parse::<IpAddr>() {
            Ok(IpAddr::V4(ip)) if ip.is_loopback() => "loopback",
            Ok(IpAddr::V4(ip)) if ip.is_link_local() => "link-local",
            Ok(IpAddr::V4(ip)) if ip.is_private() => "private",
            Ok(IpAddr::V4(ip)) if ip.is_unspecified() => "unspecified",
            Ok(IpAddr::V4(ip)) if ip.is_multicast() => "multicast",
            Ok(IpAddr::V4(_)) => "global-range",
            Ok(IpAddr::V6(ip)) if ip.is_loopback() => "loopback",
            Ok(IpAddr::V6(ip)) if (ip.segments()[0] & 0xffc0) == 0xfe80 => "link-local",
            Ok(IpAddr::V6(ip)) if (ip.segments()[0] & 0xfe00) == 0xfc00 => "private",
            Ok(IpAddr::V6(ip)) if ip.is_unspecified() => "unspecified",
            Ok(IpAddr::V6(ip)) if ip.is_multicast() => "multicast",
            Ok(IpAddr::V6(_)) => "global-range",
            Err(_) => "unknown",
        }
    }
    pub fn cidr(&self) -> String {
        match self.prefix {
            Some(n) => format!("{}/{}", self.ip, n),
            None => self.ip.clone(),
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Counters {
    pub rx: u64,
    pub tx: u64,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Interface {
    pub name: String,
    pub flags: Vec<String>,
    pub mtu: Option<u32>,
    pub mac: Option<String>,
    pub status: Option<String>,
    pub media: Option<String>,
    pub addresses: Vec<Address>,
    pub members: Vec<String>,
    pub options: Vec<String>,
    pub counters: Option<Counters>,
    pub hardware: Option<String>,
    pub raw: Vec<String>,
    /// Position in the original `ifconfig -a` output (the parsed list is sorted by name).
    pub order: usize,
}
impl Interface {
    pub fn group(&self) -> &'static str {
        let n = self.name.as_str();
        if self.flags.iter().any(|f| f == "LOOPBACK") || n == "lo" || n.starts_with("lo0") {
            "Loopback"
        } else if n.starts_with("utun")
            || n.starts_with("tun")
            || n.starts_with("tap")
            || n.starts_with("wg")
            || n.starts_with("ipsec")
        {
            "Tunnels"
        } else if n.starts_with("bridge")
            || n.starts_with("br-")
            || n == "docker0"
            || !self.members.is_empty()
        {
            "Bridges"
        } else if ["awdl", "llw", "anpi", "ap", "nan", "gif", "stf"]
            .iter()
            .any(|p| n.starts_with(p))
        {
            "System"
        } else if n.starts_with("veth") || n.starts_with("virbr") || n.starts_with("vnet") {
            "Virtual"
        } else {
            "Network"
        }
    }
    pub fn up(&self) -> bool {
        self.flags.iter().any(|f| f == "UP")
    }
    pub fn active(&self) -> bool {
        match self.status.as_deref() {
            Some("active" | "UP") => true,
            Some("inactive" | "DOWN" | "LOWERLAYERDOWN" | "DORMANT") => false,
            _ => self.up() && self.flags.iter().any(|f| f == "RUNNING" || f == "LOWER_UP"),
        }
    }
    pub fn label(&self) -> String {
        if let Some(h) = &self.hardware {
            return h.clone();
        }
        match self.group() {
            "Tunnels" => "Tunnel (owner unknown)".into(),
            "Loopback" => "Local host".into(),
            "Bridges" => "Network bridge".into(),
            "Virtual" => "Virtual interface".into(),
            "System" => match self.name.as_str() {
                n if n.starts_with("awdl") => "Apple peer-to-peer".into(),
                n if n.starts_with("llw") => "Apple low-latency peer link".into(),
                _ => "System interface".into(),
            },
            _ if self.name.starts_with("wl") => "Wi-Fi".into(),
            _ => "Network adapter".into(),
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub interfaces: Vec<Interface>,
    pub warnings: Vec<String>,
}

pub fn json_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < ' ' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
pub fn json_list(items: &[String]) -> String {
    format!(
        "[{}]",
        items
            .iter()
            .map(|s| json_string(s))
            .collect::<Vec<_>>()
            .join(",")
    )
}
impl Snapshot {
    pub fn json(&self) -> String {
        let items = self.interfaces.iter().map(|i| {
            let addresses = i.addresses.iter().map(|a| format!(
                "{{\"address\":{},\"prefix\":{},\"scope\":{},\"attributes\":{}}}",
                json_string(&a.ip), a.prefix.map(|p| p.to_string()).unwrap_or("null".into()),
                json_string(a.scope()), json_list(&a.attributes))).collect::<Vec<_>>().join(",");
            format!("{{\"name\":{},\"label\":{},\"group\":{},\"up\":{},\"active\":{},\"mtu\":{},\"mac\":{},\"status\":{},\"media\":{},\"flags\":{},\"options\":{},\"members\":{},\"addresses\":[{}],\"counters\":{}}}",
                json_string(&i.name), json_string(&i.label()), json_string(i.group()), i.up(), i.active(),
                i.mtu.map(|n| n.to_string()).unwrap_or("null".into()),
                i.mac.as_ref().map(|s| json_string(s)).unwrap_or("null".into()),
                i.status.as_ref().map(|s| json_string(s)).unwrap_or("null".into()),
                i.media.as_ref().map(|s| json_string(s)).unwrap_or("null".into()),
                json_list(&i.flags), json_list(&i.options), json_list(&i.members), addresses,
                i.counters.as_ref().map(|c| format!("{{\"rx_bytes\":{},\"tx_bytes\":{}}}", c.rx, c.tx)).unwrap_or("null".into()))
        }).collect::<Vec<_>>().join(",");
        format!(
            "{{\"schema_version\":1,\"interfaces\":[{}],\"warnings\":{}}}",
            items,
            json_list(&self.warnings)
        )
    }
}
