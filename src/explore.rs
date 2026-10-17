//! Read-only topology and an evidence report; no reachability score is inferred.
use crate::{
    insight,
    model::{json_list, json_string, Snapshot},
    view,
};
pub fn topology(s: &Snapshot, json: bool) -> String {
    let links: Vec<_> = s
        .interfaces
        .iter()
        .flat_map(|i| i.members.iter().map(move |m| (i.name.clone(), m.clone())))
        .collect();
    if json {
        let nodes = s
            .interfaces
            .iter()
            .map(|i| {
                format!(
                    "{{\"id\":{},\"group\":{},\"active\":{}}}",
                    json_string(&i.name),
                    json_string(i.group()),
                    i.active()
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let edges = links
            .iter()
            .map(|(a, b)| {
                format!(
                    "{{\"bridge\":{},\"member\":{}}}",
                    json_string(a),
                    json_string(b)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        return format!("{{\"nodes\":[{nodes}],\"bridge_memberships\":[{edges}],\"note\":\"Interface relationships only; no inferred internet or tunnel route\"}}");
    }
    let mut lines = vec![
        "MACORA · Interface topology".into(),
        "Observed bridge membership; tunnels are listed separately, not inferred as a route."
            .into(),
    ];
    for i in &s.interfaces {
        lines.push(format!(
            "{} [{}] · {} · {}",
            i.name,
            i.group(),
            i.label(),
            if i.active() {
                "link-active"
            } else {
                "idle/down"
            }
        ));
        if !i.members.is_empty() {
            lines.push(format!("  Members: {}", i.members.join(", ")));
        }
    }
    view::safe(&lines.join("\n"))
}
pub fn report(s: &Snapshot, json: bool) -> String {
    if json {
        return format!("{{\"scope\":\"local interface evidence; not an internet test\",\"snapshot\":{},\"findings\":{}}}",s.json(),json_list(&insight::findings(s)));
    }
    let mut text = String::from("# MACORA network evidence\n\nLocal interface capture. This is not an internet reachability test or a security certification.\n\n## Interfaces\n\n");
    for i in &s.interfaces {
        text += &format!(
            "- {} · {} · {} · MTU {}\n",
            i.name,
            i.label(),
            if i.active() {
                "link-active"
            } else {
                "idle/down"
            },
            i.mtu.map(|n| n.to_string()).unwrap_or("unknown".into())
        );
        for a in &i.addresses {
            text += &format!(
                "  - {} · {} · {}\n",
                a.cidr(),
                a.scope(),
                a.attributes.join(", ")
            );
        }
    }
    text += "\n## Findings\n\n";
    for f in insight::findings(s) {
        text += &format!("- {f}\n");
    }
    text+="\n## Boundaries\n\nNo DNS lookup, gateway probe, external endpoint, packet capture or VPN-owner attribution was performed. IP/MAC redaction does not anonymize labels or arbitrary inspector output.\n";
    view::safe(&text)
}
