import Foundation

public struct NetworkAddress: Codable, Hashable, Identifiable {
    public let address: String
    public let prefix: Int?
    public let scope: String
    public let attributes: [String]
    public var id: String { address + String(prefix ?? -1) }
    public var display: String { address + (prefix.map { "/\($0)" } ?? "") }
}
public struct TrafficCounters: Codable { public let rx_bytes: UInt64; public let tx_bytes: UInt64 }
public struct NetworkInterface: Codable, Identifiable {
    public let name: String
    public let label: String
    public let group: String
    public let up: Bool
    public let active: Bool
    public let mtu: Int?
    public let mac: String?
    public let media: String?
    public let status: String?
    public let flags: [String]
    public let options: [String]
    public let members: [String]
    public let addresses: [NetworkAddress]
    public let counters: TrafficCounters?
    public var id: String { name }
    public var symbol: String {
        if label.localizedCaseInsensitiveContains("Wi-Fi") { return "wifi" }
        switch group {
        case "Tunnels": return "lock.shield"
        case "Bridges": return "point.3.connected.trianglepath.dotted"
        case "Loopback": return "arrow.trianglehead.2.clockwise.rotate.90"
        case "System": return "antenna.radiowaves.left.and.right"
        default: return "network"
        }
    }
}
public struct NetworkSnapshot: Codable {
    public let schema_version: Int
    public let interfaces: [NetworkInterface]
    public let warnings: [String]
    public static func decode(_ data: Data) throws -> Self {
        let value = try JSONDecoder().decode(Self.self, from: data)
        guard value.schema_version == 1 else { throw MacoraError.message("Unsupported collector schema") }
        let names = value.interfaces.map(\.name)
        guard names.count <= 4096, Set(names).count == names.count, names.allSatisfy({ !$0.isEmpty && $0.count <= 128 }) else {
            throw MacoraError.message("Malformed interface inventory")
        }
        return value
    }
}
public enum MacoraError: LocalizedError {
    case message(String)
    public var errorDescription: String? { if case let .message(s) = self { return s }; return nil }
}
public struct Service: Identifiable, Equatable {
    public let name: String
    public let enabled: Bool
    public var id: String { name }
    public static func parse(_ output: String) -> [Service] {
        output.components(separatedBy: .newlines).dropFirst().compactMap { line in
            let s = line.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !s.isEmpty else { return nil }
            return Service(name: s.hasPrefix("*") ? String(s.dropFirst()) : s, enabled: !s.hasPrefix("*"))
        }
    }
}
public struct TrafficPoint: Identifiable {
    public let id = UUID()
    public let date: Date
    public let rx: Double
    public let tx: Double
    public init(date: Date, rx: Double, tx: Double) { self.date = date; self.rx = rx; self.tx = tx }
}
public enum Traffic {
    public static func rates(previous: TrafficCounters, current: TrafficCounters, elapsed: TimeInterval) -> (Double, Double) {
        guard elapsed > 0, elapsed.isFinite else { return (0, 0) }
        return (Double(current.rx_bytes >= previous.rx_bytes ? current.rx_bytes - previous.rx_bytes : 0) / elapsed,
                Double(current.tx_bytes >= previous.tx_bytes ? current.tx_bytes - previous.tx_bytes : 0) / elapsed)
    }
    public static func format(_ bytes: Double) -> String {
        guard bytes.isFinite, bytes >= 0 else { return "Unavailable" }
        if bytes >= 1_048_576 { return String(format: "%.1f MiB/s", bytes / 1_048_576) }
        if bytes >= 1024 { return String(format: "%.1f KiB/s", bytes / 1024) }
        return String(format: "%.0f B/s", bytes)
    }
}
