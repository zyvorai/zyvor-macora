//! Plain data describing what the probes observed. `collect` fills it in, `analyze` turns it into
//! steps and a verdict. Nothing here does I/O, so every scenario can be built by hand in tests.
use crate::probe::PingStats;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Warn,
    Fail,
    Skip,
    /// Worth knowing, not a problem (route through a tunnel, proxy configured, no IPv6).
    Info,
}
impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Status::Ok => "OK",
            Status::Warn => "WARN",
            Status::Fail => "FAIL",
            Status::Skip => "SKIP",
            Status::Info => "INFO",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Step {
    pub name: &'static str,
    pub status: Status,
    pub detail: String,
    /// The headline latency of the step in milliseconds, when it has one.
    pub ms: Option<f64>,
}
impl Step {
    pub fn new(name: &'static str, status: Status, detail: impl Into<String>) -> Self {
        Step {
            name,
            status,
            detail: detail.into(),
            ms: None,
        }
    }
    pub fn ms(mut self, ms: Option<f64>) -> Self {
        self.ms = ms;
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Route {
    pub gateway: Option<String>,
    pub interface: Option<String>,
    pub mtu: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct LinkFact {
    pub iface: Option<String>,
    pub label: String,
    pub active: bool,
    /// Usable (not 169.254.x.x) IPv4 addresses.
    pub ipv4: Vec<String>,
    pub self_assigned: bool,
    pub global_ipv6: bool,
    pub is_wifi: bool,
    /// Set when the default route goes through a tunnel (name of the tunnel interface).
    pub tunnel: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Tcp {
    Ok(f64),
    Fail(String),
}

#[derive(Debug, Clone, Default)]
pub struct DnsProbe {
    pub server: String,
    /// Both queries (a real name and a random one that forces recursion) were answered.
    pub ok: bool,
    pub ms: Option<f64>,
    pub fresh_ms: Option<f64>,
    pub detail: String,
}

#[derive(Debug, Clone, Default)]
pub struct Http {
    pub host: String,
    pub family: u8,
    pub ok: bool,
    pub error: Option<String>,
    /// Cumulative milliseconds from the start, as curl reports them.
    pub dns_ms: f64,
    pub connect_ms: f64,
    pub tls_ms: f64,
    pub ttfb_ms: f64,
    pub total_ms: f64,
    pub ip: Option<String>,
    pub code: u32,
}
impl Http {
    pub fn tcp_stage(&self) -> f64 {
        (self.connect_ms - self.dns_ms).max(0.0)
    }
    pub fn tls_stage(&self) -> f64 {
        if self.tls_ms > 0.0 {
            (self.tls_ms - self.connect_ms).max(0.0)
        } else {
            0.0
        }
    }
    pub fn wait_stage(&self) -> f64 {
        let base = if self.tls_ms > 0.0 {
            self.tls_ms
        } else {
            self.connect_ms
        };
        (self.ttfb_ms - base).max(0.0)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Wifi {
    pub associated: bool,
    pub rssi: Option<i32>,
    pub noise: Option<i32>,
    pub rate_mbps: Option<u32>,
    pub channel: Option<u32>,
    pub band: Option<String>,
    pub width: Option<String>,
    pub phy: Option<String>,
    pub mcs: Option<u32>,
}
impl Wifi {
    pub fn snr(&self) -> Option<i32> {
        Some(self.rssi? - self.noise?)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PmtuResult {
    Measured(u32),
    Unmeasurable(String),
}
#[derive(Debug, Clone)]
pub struct Pmtu {
    pub target: String,
    pub link_mtu: u32,
    pub result: PmtuResult,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Proxy {
    pub http: Option<String>,
    pub https: Option<String>,
    pub socks: Option<String>,
    pub pac: Option<String>,
    pub auto_discovery: bool,
}
impl Proxy {
    pub fn any(&self) -> bool {
        self.http.is_some()
            || self.https.is_some()
            || self.socks.is_some()
            || self.pac.is_some()
            || self.auto_discovery
    }
}

#[derive(Debug, Clone, Default)]
pub struct Ipv6Fact {
    pub global_address: bool,
    pub default_route: bool,
    pub tcp: Option<Tcp>,
}

#[derive(Debug, Clone)]
pub struct Speed {
    pub down_mbps: f64,
    pub up_mbps: f64,
    pub rpm: Option<f64>,
    pub base_rtt_ms: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Captive {
    Clear,
    /// Something other than Apple's "Success" page answered; the text is a short excerpt.
    Intercepted(String),
    /// The check itself could not run (no network, timeout).
    Unknown(String),
}

#[derive(Debug, Clone, Default)]
pub struct TargetFacts {
    pub host: String,
    /// The system resolver (hosts file, mDNS, VPN split DNS): what apps actually use.
    pub system: Option<Result<f64, String>>,
    pub dns: Vec<DnsProbe>,
    pub dns_reference: Option<DnsProbe>,
    pub http: Option<Http>,
    pub ping: Option<PingStats>,
    pub pmtu: Option<Pmtu>,
}

#[derive(Debug, Clone)]
pub struct Facts {
    pub link: LinkFact,
    pub route: Route,
    pub route6: Route,
    /// The router address that was pinged (IPv4 preferred, IPv6 when that is all there is).
    pub gateway: Option<String>,
    pub gateway_ping: Option<PingStats>,
    pub internet_tcp: Vec<(String, Tcp)>,
    pub internet_ping: Option<PingStats>,
    pub captive: Option<Captive>,
    pub dns_servers: Vec<String>,
    pub dns: Vec<DnsProbe>,
    pub dns_reference: Option<DnsProbe>,
    /// The system resolver (what apps use): milliseconds, or the error.
    pub dns_system: Option<Result<f64, String>>,
    pub http4: Option<Http>,
    pub http6: Option<Http>,
    pub pmtu: Option<Pmtu>,
    pub wifi: Option<Wifi>,
    pub proxy: Proxy,
    pub ipv6: Ipv6Fact,
    /// Only present when the user asked for `speed`; the error text when the test could not run.
    pub speed: Option<Result<Speed, String>>,
    pub target: Option<TargetFacts>,
    /// `diagnose quick` was used: Wi-Fi details and path MTU were skipped on purpose.
    pub quick: bool,
}

fn ping(avg: f64, stddev: f64, loss: f64) -> PingStats {
    let sent = 10;
    PingStats {
        sent,
        received: (sent as f64 * (100.0 - loss) / 100.0).round() as u32,
        loss_pct: loss,
        min_ms: Some((avg - stddev).max(0.1)),
        avg_ms: Some(avg),
        max_ms: Some(avg + stddev * 2.0),
        stddev_ms: Some(stddev),
    }
}
fn probe(server: &str, ms: f64) -> DnsProbe {
    DnsProbe {
        server: server.into(),
        ok: true,
        ms: Some(ms),
        fresh_ms: Some(ms + 15.0),
        detail: "answered".into(),
    }
}

/// Public addresses macora itself probes. They are constants, so redaction leaves them readable.
pub const PUBLIC_TARGETS: &[&str] = &["1.1.1.1", "8.8.8.8", "9.9.9.9"];

impl Facts {
    /// A fully healthy Wi-Fi network (`en0`, IPv4 + IPv6, 1500-byte path, quick DNS). Tests start from
    /// this and break one thing at a time, so each scenario states exactly what is different.
    pub fn healthy_example() -> Facts {
        Facts {
            link: LinkFact {
                iface: Some("en0".into()),
                label: "Wi-Fi".into(),
                active: true,
                ipv4: vec!["192.0.2.8".into()],
                self_assigned: false,
                global_ipv6: true,
                is_wifi: true,
                tunnel: None,
            },
            route: Route {
                gateway: Some("192.0.2.1".into()),
                interface: Some("en0".into()),
                mtu: Some(1500),
            },
            route6: Route {
                gateway: Some("fe80::1%en0".into()),
                interface: Some("en0".into()),
                mtu: Some(1500),
            },
            gateway: Some("192.0.2.1".into()),
            gateway_ping: Some(ping(4.0, 0.8, 0.0)),
            internet_tcp: vec![
                ("1.1.1.1:443".into(), Tcp::Ok(11.0)),
                ("8.8.8.8:443".into(), Tcp::Ok(13.0)),
                ("9.9.9.9:443".into(), Tcp::Ok(15.0)),
            ],
            internet_ping: Some(ping(12.0, 1.5, 0.0)),
            captive: Some(Captive::Clear),
            dns_servers: vec!["192.0.2.1".into()],
            dns: vec![probe("192.0.2.1", 18.0)],
            dns_reference: Some(probe("1.1.1.1", 14.0)),
            dns_system: Some(Ok(26.0)),
            http4: Some(Http {
                host: "www.apple.com".into(),
                family: 4,
                ok: true,
                dns_ms: 14.0,
                connect_ms: 25.0,
                tls_ms: 38.0,
                ttfb_ms: 61.0,
                total_ms: 95.0,
                ip: Some("198.51.100.7".into()),
                code: 200,
                ..Default::default()
            }),
            http6: Some(Http {
                host: "www.apple.com".into(),
                family: 6,
                ok: true,
                dns_ms: 14.0,
                connect_ms: 27.0,
                tls_ms: 40.0,
                ttfb_ms: 63.0,
                total_ms: 97.0,
                ip: Some("2001:db8::7".into()),
                code: 200,
                ..Default::default()
            }),
            pmtu: Some(Pmtu {
                target: "1.1.1.1".into(),
                link_mtu: 1500,
                result: PmtuResult::Measured(1500),
            }),
            wifi: Some(Wifi {
                associated: true,
                rssi: Some(-55),
                noise: Some(-94),
                rate_mbps: Some(866),
                channel: Some(40),
                band: Some("5GHz".into()),
                width: Some("80MHz".into()),
                phy: Some("802.11ax".into()),
                mcs: Some(9),
            }),
            proxy: Proxy::default(),
            ipv6: Ipv6Fact {
                global_address: true,
                default_route: true,
                tcp: Some(Tcp::Ok(14.0)),
            },
            speed: None,
            target: None,
            quick: false,
        }
    }

    /// Addresses that identify this network; `redact` masks them in every output format.
    pub fn secrets(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        let mut add = |s: &str| {
            // Strip a port and an interface scope: only the address itself identifies the network.
            let s = s.trim_start_matches('[');
            let s = s.split(']').next().unwrap_or(s);
            let host = match s.rsplit_once(':') {
                Some((h, p)) if !h.contains(':') && p.chars().all(|c| c.is_ascii_digit()) => h,
                _ => s,
            };
            for form in [
                host.to_string(),
                host.split('%').next().unwrap_or("").to_string(),
            ] {
                if form.len() > 2 && !v.contains(&form) {
                    v.push(form);
                }
            }
        };
        for r in [&self.route, &self.route6] {
            if let Some(g) = &r.gateway {
                add(g);
            }
        }
        if let Some(g) = &self.gateway {
            add(g);
        }
        for a in &self.link.ipv4 {
            add(a);
        }
        for s in &self.dns_servers {
            // Well-known public probe targets are not identifying; the user's own resolvers are.
            if !PUBLIC_TARGETS.contains(&s.as_str()) {
                add(s);
            }
        }
        // Longest first so a longer address is replaced before any shorter one inside it.
        v.sort_by_key(|s| std::cmp::Reverse(s.len()));
        v
    }
}
