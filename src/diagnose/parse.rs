//! Pure parsers for the output of the tools `diagnose` runs. None of them does I/O and none of
//! them panics: unrecognised or truncated text yields `None` / an empty result.
use super::facts::{Http, Proxy, Route, Speed, Wifi};
use crate::json;

/// `route -n get default` / `route -n get -inet6 default`.
pub fn route(text: &str) -> Route {
    let mut r = Route::default();
    let mut header: Option<Vec<&str>> = None;
    for line in text.lines() {
        let t = line.trim();
        if let Some(v) = t.strip_prefix("gateway:") {
            r.gateway = Some(v.trim().to_string()).filter(|s| !s.is_empty());
        } else if let Some(v) = t.strip_prefix("interface:") {
            r.interface = Some(v.trim().to_string()).filter(|s| !s.is_empty());
        } else if t.starts_with("recvpipe") {
            header = Some(t.split_whitespace().collect());
        } else if let Some(h) = header.take() {
            let vals: Vec<&str> = t.split_whitespace().collect();
            if let Some(i) = h.iter().position(|c| *c == "mtu") {
                r.mtu = vals.get(i).and_then(|v| v.parse().ok()).filter(|m| *m > 0);
            }
        }
    }
    r
}

/// Name servers from `scutil --dns`, in order of first appearance, without duplicates.
pub fn dns_servers(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("nameserver[") {
            if let Some((_, v)) = t.split_once(':') {
                let v = v.trim();
                if !v.is_empty() && !out.iter().any(|s| s == v) {
                    out.push(v.to_string());
                }
            }
        }
    }
    out
}

/// `scutil --proxy`.
pub fn proxy(text: &str) -> Proxy {
    let mut kv = std::collections::HashMap::new();
    for line in text.lines() {
        if let Some((k, v)) = line.trim().split_once(" : ") {
            kv.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    let on = |k: &str| kv.get(k).is_some_and(|v| v == "1");
    let host = |enable: &str, host: &str, port: &str| {
        on(enable).then(|| {
            let h = kv.get(host).cloned().unwrap_or_default();
            match kv.get(port) {
                Some(p) if !h.is_empty() => format!("{h}:{p}"),
                _ => h,
            }
        })
    };
    Proxy {
        http: host("HTTPEnable", "HTTPProxy", "HTTPPort"),
        https: host("HTTPSEnable", "HTTPSProxy", "HTTPSPort"),
        socks: host("SOCKSEnable", "SOCKSProxy", "SOCKSPort"),
        pac: on("ProxyAutoConfigEnable").then(|| {
            kv.get("ProxyAutoConfigURLString")
                .cloned()
                .unwrap_or_default()
        }),
        auto_discovery: on("ProxyAutoDiscoveryEnable"),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Dig {
    /// NOERROR, NXDOMAIN, SERVFAIL, REFUSED… or `None` when no answer arrived.
    pub status: Option<String>,
    pub ms: Option<f64>,
    pub timed_out: bool,
}
impl Dig {
    /// A reply that proves the server is alive and recursing (NXDOMAIN is a fine answer to a
    /// name that does not exist).
    pub fn answered(&self) -> bool {
        matches!(self.status.as_deref(), Some("NOERROR") | Some("NXDOMAIN"))
    }
}
/// `dig +noall +comments +stats`.
pub fn dig(text: &str) -> Dig {
    let mut d = Dig {
        status: None,
        ms: None,
        timed_out: false,
    };
    for line in text.lines() {
        if let Some(i) = line.find("status: ") {
            let s: String = line[i + 8..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect();
            if !s.is_empty() {
                d.status = Some(s);
            }
        }
        if let Some(i) = line.find("Query time: ") {
            d.ms = line[i + 12..]
                .split_whitespace()
                .next()
                .and_then(|n| n.parse().ok());
        }
        if line.contains("timed out") || line.contains("no servers could be reached") {
            d.timed_out = true;
        }
    }
    d
}

/// The `-w` format used by `collect`: `dns=… connect=… tls=… ttfb=… total=… ip=… http=…`.
pub fn curl_timing(text: &str, host: &str, family: u8) -> Option<Http> {
    let line = text
        .lines()
        .rev()
        .find(|l| l.contains("dns=") && l.contains("total="))?;
    let get = |k: &str| {
        line.split_whitespace()
            .find_map(|t| t.strip_prefix(k).and_then(|v| v.strip_prefix('=')))
    };
    let ms = |k: &str| {
        get(k)
            .and_then(|v| v.parse::<f64>().ok())
            .map(|s| s * 1000.0)
    };
    let code: u32 = get("http").and_then(|v| v.parse().ok()).unwrap_or(0);
    Some(Http {
        host: host.into(),
        family,
        ok: code != 0,
        error: (code == 0).then(|| "no HTTP response".to_string()),
        dns_ms: ms("dns")?,
        connect_ms: ms("connect")?,
        tls_ms: ms("tls").unwrap_or(0.0),
        ttfb_ms: ms("ttfb").unwrap_or(0.0),
        total_ms: ms("total")?,
        ip: get("ip").filter(|v| !v.is_empty()).map(|v| v.to_string()),
        code,
    })
}

fn first_int(s: &str) -> Option<i32> {
    s.split_whitespace().next().and_then(|t| t.parse().ok())
}

/// `system_profiler SPAirPortDataType -json`, for the interface named `iface`.
pub fn wifi(text: &str, iface: &str) -> Option<Wifi> {
    let root = json::parse(text).ok()?;
    let interfaces = root
        .get("SPAirPortDataType")?
        .idx(0)?
        .get("spairport_airport_interfaces")?
        .as_array()?;
    let me = interfaces
        .iter()
        .find(|i| i.get("_name").and_then(|n| n.as_str()) == Some(iface))?;
    let Some(cur) = me.get("spairport_current_network_information") else {
        return Some(Wifi::default());
    };
    let mut w = Wifi {
        associated: true,
        ..Default::default()
    };
    if let Some(sn) = cur.get("spairport_signal_noise").and_then(|v| v.as_str()) {
        let mut parts = sn.split('/');
        w.rssi = parts.next().and_then(first_int);
        w.noise = parts.next().and_then(first_int);
    }
    w.rate_mbps = cur
        .get("spairport_network_rate")
        .and_then(|v| v.as_f64())
        .filter(|r| *r >= 0.0)
        .map(|r| r as u32);
    w.mcs = cur
        .get("spairport_network_mcs")
        .and_then(|v| v.as_f64())
        .map(|m| m as u32);
    w.phy = cur
        .get("spairport_network_phymode")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    if let Some(ch) = cur
        .get("spairport_network_channel")
        .and_then(|v| v.as_str())
    {
        w.channel = ch.split_whitespace().next().and_then(|n| n.parse().ok());
        if let (Some(a), Some(b)) = (ch.find('('), ch.find(')')) {
            if a < b {
                let mut it = ch[a + 1..b].split(',').map(|s| s.trim().to_string());
                // macOS calls the 2.4 GHz band "2GHz".
                w.band = it.next().filter(|s| !s.is_empty()).map(|b| {
                    if b == "2GHz" {
                        "2.4GHz".to_string()
                    } else {
                        b
                    }
                });
                w.width = it.next().filter(|s| !s.is_empty());
            }
        }
    }
    Some(w)
}

/// `networkQuality -c`.
pub fn network_quality(text: &str) -> Result<Speed, String> {
    let v = json::parse(text)?;
    let num = |k: &str| v.get(k).and_then(|x| x.as_f64());
    let down = num("dl_throughput").ok_or("networkQuality reported no download throughput")?;
    let up = num("ul_throughput").ok_or("networkQuality reported no upload throughput")?;
    Ok(Speed {
        down_mbps: down / 1e6,
        up_mbps: up / 1e6,
        rpm: num("responsiveness"),
        base_rtt_ms: num("base_rtt"),
    })
}
