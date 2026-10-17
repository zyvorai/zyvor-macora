//! Path-MTU discovery. `works(size)` answers "does a do-not-fragment packet of `size` bytes (headers
//! included) reach the target?". Candidate sizes are tested in parallel rounds, so a lost probe costs
//! one timeout per round instead of one per step: a typical search finishes in about three rounds.
use std::thread;

/// The smallest size tried. A path that drops even this has no measurable MTU.
pub const FLOOR: u32 = 576;
/// Sizes that real networks use (Ethernet, PPPoE, 6in4, common VPNs), largest first.
const LADDER: &[u32] = &[
    1500, 1492, 1480, 1472, 1460, 1452, 1440, 1400, 1380, 1360, 1280, 1200, 1000, 800, FLOOR,
];
/// At most this many sizes per refinement round.
const WIDTH: u32 = 24;

#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// The largest size that got through, or `None` when not even `FLOOR` did.
    pub mtu: Option<u32>,
    pub rounds: u32,
    pub probes: u32,
}

fn round(sizes: &[u32], works: &(dyn Fn(u32) -> bool + Sync)) -> Vec<(u32, bool)> {
    thread::scope(|s| {
        let handles: Vec<_> = sizes
            .iter()
            .map(|&size| s.spawn(move || (size, works(size))))
            .collect();
        handles
            .into_iter()
            .zip(sizes)
            .map(|(h, &size)| h.join().unwrap_or((size, false)))
            .collect()
    })
}

pub fn search(link_mtu: u32, works: &(dyn Fn(u32) -> bool + Sync)) -> Outcome {
    let link_mtu = link_mtu.clamp(FLOOR, 9000);
    let mut ladder: Vec<u32> = vec![link_mtu];
    ladder.extend(LADDER.iter().copied().filter(|s| *s < link_mtu));
    let first = round(&ladder, works);
    let mut probes = first.len() as u32;
    let mut rounds = 1;
    if first.first().is_some_and(|(_, ok)| *ok) {
        return Outcome {
            mtu: Some(link_mtu),
            rounds,
            probes,
        };
    }
    // `first` is ordered largest to smallest: the first success is the largest working size.
    let Some(pos) = first.iter().position(|(_, ok)| *ok) else {
        return Outcome {
            mtu: None,
            rounds,
            probes,
        };
    };
    let mut good = first[pos].0;
    let mut bad = first[pos - 1].0;
    while bad - good > 1 {
        let gap = bad - good;
        let n = (gap - 1).min(WIDTH);
        let step = gap / (n + 1);
        let sizes: Vec<u32> = (1..=n)
            .map(|k| good + k * step.max(1))
            .filter(|s| *s < bad)
            .collect();
        let results = round(&sizes, works);
        probes += results.len() as u32;
        rounds += 1;
        let best_ok = results.iter().filter(|(_, ok)| *ok).map(|(s, _)| *s).max();
        if let Some(g) = best_ok {
            good = g;
        }
        if let Some(b) = results
            .iter()
            .filter(|(s, ok)| !*ok && *s > good)
            .map(|(s, _)| *s)
            .min()
        {
            bad = b;
        }
        if best_ok.is_none() && sizes.is_empty() {
            break;
        }
    }
    Outcome {
        mtu: Some(good),
        rounds,
        probes,
    }
}

/// A short human hint for a measured path MTU smaller than the link MTU.
pub fn cause_hint(mtu: u32) -> &'static str {
    match mtu {
        1492 => "1492 is typical of PPPoE DSL/fibre",
        1480 => "1480 is typical of IPv6-in-IPv4 (6in4) tunnels",
        1400..=1479 => "this range is typical of VPNs and tunnels",
        1493..=1499 => "something on the path takes a few bytes of overhead",
        1481..=1491 => "PPPoE or a tunnel on the path takes some bytes of overhead",
        _ => "a tunnel or an unusual link on the path is limiting packet size",
    }
}
