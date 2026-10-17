import Foundation

public enum DiagnosisStatus: String, Codable, Equatable {
    case ok = "OK", warn = "WARN", fail = "FAIL", skip = "SKIP"
    public var symbol: String {
        switch self {
        case .ok: return "checkmark.circle.fill"
        case .warn: return "exclamationmark.triangle.fill"
        case .fail: return "xmark.octagon.fill"
        case .skip: return "minus.circle"
        }
    }
    public var label: String {
        switch self { case .ok: return "Passed"; case .warn: return "Needs attention"; case .fail: return "Failed"; case .skip: return "Skipped" }
    }
}
public struct DiagnosisStep: Codable, Identifiable, Equatable {
    public let step: String
    public let status: DiagnosisStatus
    public let detail: String
    public var id: String { step }
    public init(step: String, status: DiagnosisStatus, detail: String) { self.step = step; self.status = status; self.detail = detail }
    /// What each layer means, in words a non-specialist can act on.
    public var explanation: String {
        switch step {
        case "Link": return "Is a network adapter connected and holding an address?"
        case "Gateway": return "Can this Mac reach your router?"
        case "Internet": return "Can this Mac reach a public server by IP address?"
        case "DNS": return "Can names like example.com be turned into addresses, and how fast?"
        case "MTU": return "Do full-size packets get through, or does a VPN/tunnel shrink them?"
        default: return step
        }
    }
}
public struct DiagnosisReport: Codable, Equatable {
    public let steps: [DiagnosisStep]
    public let verdict: String
    public init(steps: [DiagnosisStep], verdict: String) { self.steps = steps; self.verdict = verdict }
    public var worst: DiagnosisStatus {
        if steps.contains(where: { $0.status == .fail }) { return .fail }
        if steps.contains(where: { $0.status == .warn }) { return .warn }
        return .ok
    }
    public static func decode(_ text: String) throws -> DiagnosisReport {
        let report = try JSONDecoder().decode(DiagnosisReport.self, from: Data(text.utf8))
        guard !report.steps.isEmpty, report.steps.count <= 32, !report.verdict.isEmpty else { throw MacoraError.message("Malformed diagnosis") }
        return report
    }
}

/// Bounded per-interface rate history for sparklines. Counters that are unavailable are never invented.
public struct RateHistory: Equatable {
    public private(set) var series: [String: [Double]] = [:]
    public let limit: Int
    public init(limit: Int = 60) { self.limit = max(2, limit) }
    public mutating func append(_ name: String, _ value: Double) {
        guard value.isFinite, value >= 0 else { return }
        var values = series[name, default: []]
        values.append(value)
        if values.count > limit { values.removeFirst(values.count - limit) }
        series[name] = values
    }
    public func values(_ name: String) -> [Double] { series[name] ?? [] }
    /// Scales to 0...1 against the series maximum; a flat zero series stays flat.
    public func normalized(_ name: String) -> [Double] {
        let v = values(name)
        guard let peak = v.max(), peak > 0 else { return v.map { _ in 0 } }
        return v.map { $0 / peak }
    }
    public mutating func removeAll() { series.removeAll() }
}

public enum StatusSummary {
    /// One-paragraph text suitable for the clipboard; addresses are masked when asked.
    public static func text(snapshot: NetworkSnapshot?, rx: Double?, tx: Double?, report: DiagnosisReport?, redact: Bool) -> String {
        guard let snapshot else { return "MACORA: no network data yet." }
        var lines = ["MACORA network summary"]
        for i in snapshot.interfaces where i.group == "Network" {
            let address = i.addresses.first(where: { !$0.address.contains(":") })?.display ?? "no address"
            lines.append("\(i.name) (\(i.label)): \(i.active ? "link active" : "idle/down"), \(redact ? "<IPv4>" : address)")
        }
        if let rx, let tx { lines.append("Traffic: ↓ \(Traffic.format(rx)) ↑ \(Traffic.format(tx))") }
        if let report { lines.append("Diagnosis: \(report.verdict)") }
        return lines.joined(separator: "\n")
    }
}
