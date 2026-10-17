import Foundation

public struct WorkspaceCommand: Identifiable, Equatable {
    public let id: String
    public let title: String
    public let detail: String
    public let symbol: String
}
public enum WorkspaceCatalog {
    public static let commands: [WorkspaceCommand] = [
        .init(id: "Overview", title: "Open overview", detail: "Your network at a glance", symbol: "gauge.with.dots.needle.50percent"),
        .init(id: "Interfaces", title: "Explore interfaces", detail: "Addresses, flags and pinned adapters", symbol: "network"),
        .init(id: "Wi-Fi", title: "Explore Wi-Fi", detail: "Scan and join with CoreWLAN", symbol: "wifi"),
        .init(id: "Services", title: "Manage network services", detail: "DHCP, IPv6, proxies and locations", symbol: "slider.horizontal.3"),
        .init(id: "Topology", title: "Open topology", detail: "Observed bridges and interface roles", symbol: "point.3.connected.trianglepath.dotted"),
        .init(id: "Compare", title: "Compare baseline", detail: "Review configuration drift", symbol: "rectangle.on.rectangle"),
        .init(id: "Network Tools", title: "Open network tools", detail: "TCP, subnet and DNS tools", symbol: "wrench.and.screwdriver"),
        .init(id: "Routes", title: "Inspect routes", detail: "IPv4 and IPv6 routing tables", symbol: "arrow.triangle.branch"),
        .init(id: "DNS", title: "Inspect DNS", detail: "Every scoped system resolver", symbol: "globe"),
        .init(id: "Neighbors", title: "Inspect neighbors", detail: "ARP and IPv6 neighbor tables", symbol: "laptopcomputer.and.iphone"),
        .init(id: "Connections", title: "Inspect connections", detail: "User-visible TCP and UDP sockets", symbol: "cable.connector"),
        .init(id: "Diagnostics", title: "Run diagnostics", detail: "Local evidence without an invented health score", symbol: "stethoscope"),
        .init(id: "Change History", title: "Open change timeline", detail: "Observed changes and successful actions", symbol: "clock.arrow.circlepath"),
        .init(id: "configure", title: "Configure a service", detail: "Preview before authorization", symbol: "lock.shield"),
        .init(id: "export", title: "Export redacted snapshot", detail: "Mask modeled IP and MAC values", symbol: "square.and.arrow.up"),
        .init(id: "baseline", title: "Capture baseline", detail: "Keep current interface state in memory", symbol: "flag"),
        .init(id: "preferences", title: "Workspace preferences", detail: "Appearance, density and refresh rate", symbol: "gearshape")
    ]
    public static func matches(_ text: String, query: String) -> Bool {
        let q = query.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if q.isEmpty { return true }
        let source = text.lowercased()
        if source.contains(q) { return true }
        var chars = source.makeIterator()
        for target in q {
            var found = false
            while let c = chars.next() { if c == target { found = true; break } }
            if !found { return false }
        }
        return true
    }
    public static func filtered(_ query: String) -> [WorkspaceCommand] {
        commands.filter { matches($0.title + " " + $0.detail, query: query) }
    }
}
public struct ConfigurationChange: Identifiable, Equatable {
    public let interface: String
    public let field: String
    public let before: String
    public let after: String
    public var id: String { interface + ":" + field + ":" + before + ":" + after }
    public var summary: String { "\(interface) · \(field): \(before) → \(after)" }
}
public enum SnapshotComparison {
    public static func changes(from old: NetworkSnapshot, to new: NetworkSnapshot) -> [ConfigurationChange] {
        var changes: [ConfigurationChange] = []
        let oldMap = Dictionary(old.interfaces.map { ($0.name, $0) }, uniquingKeysWith: { first, _ in first })
        let newMap = Dictionary(new.interfaces.map { ($0.name, $0) }, uniquingKeysWith: { first, _ in first })
        for name in Set(oldMap.keys).union(newMap.keys).sorted() {
            guard let a = oldMap[name] else { changes.append(.init(interface: name, field: "Presence", before: "Absent", after: "Added")); continue }
            guard let b = newMap[name] else { changes.append(.init(interface: name, field: "Presence", before: "Present", after: "Removed")); continue }
            func append(_ field: String, _ before: String, _ after: String) { if before != after { changes.append(.init(interface: name, field: field, before: before, after: after)) } }
            append("Link", a.active ? "Active" : "Idle/down", b.active ? "Active" : "Idle/down")
            append("Admin", a.up ? "Up" : "Down", b.up ? "Up" : "Down")
            append("MTU", a.mtu.map(String.init) ?? "Unknown", b.mtu.map(String.init) ?? "Unknown")
            append("MAC", a.mac ?? "None", b.mac ?? "None")
            append("Media", a.media ?? "Unknown", b.media ?? "Unknown")
            append("Flags", a.flags.sorted().joined(separator: ", "), b.flags.sorted().joined(separator: ", "))
            append("Members", a.members.sorted().joined(separator: ", "), b.members.sorted().joined(separator: ", "))
            let addresses: (NetworkInterface) -> String = { $0.addresses.map { $0.display + " " + $0.attributes.sorted().joined(separator: " ") }.sorted().joined(separator: "; ") }
            append("Addresses", addresses(a), addresses(b))
        }
        return changes
    }
}
public struct NetworkTimelineEvent: Identifiable {
    public let id = UUID()
    public let date: Date
    public let change: ConfigurationChange
    public init(date: Date, change: ConfigurationChange) { self.date = date; self.change = change }
}
public enum SnapshotPrivacy {
    public static func redacted(_ source: String, snapshots: [NetworkSnapshot]) -> String {
        var text = source
        var values: [String: String] = [:]
        for snapshot in snapshots { for i in snapshot.interfaces {
            if let mac = i.mac, !mac.isEmpty { values[mac] = "<MAC>" }
            for a in i.addresses where !a.address.isEmpty { values[a.address] = a.address.contains(":") ? "<IPv6>" : "<IPv4>" }
        } }
        for key in values.keys.sorted(by: { $0.count > $1.count }) { text = text.replacingOccurrences(of: key, with: values[key] ?? "<redacted>") }
        return text
    }
}
