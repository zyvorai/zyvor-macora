import Foundation

public enum DiagnosisStatus: String, Codable, Equatable {
    case ok = "OK", warn = "WARN", fail = "FAIL", skip = "SKIP", info = "INFO"
    public var symbol: String {
        switch self {
        case .ok: return "checkmark.circle.fill"
        case .warn: return "exclamationmark.triangle.fill"
        case .fail: return "xmark.octagon.fill"
        case .skip: return "minus.circle"
        case .info: return "info.circle.fill"
        }
    }
    public var label: String {
        switch self {
        case .ok: return "Passed"
        case .warn: return "Needs attention"
        case .fail: return "Failed"
        case .skip: return "Skipped"
        case .info: return "For your information"
        }
    }
}
public struct DiagnosisStep: Codable, Identifiable, Equatable {
    public let step: String
    public let status: DiagnosisStatus
    public let detail: String
    /// The step's headline latency in milliseconds, when it has one (absent in older collectors).
    public let ms: Double?
    public var id: String { step }
    public init(step: String, status: DiagnosisStatus, detail: String, ms: Double? = nil) {
        self.step = step; self.status = status; self.detail = detail; self.ms = ms
    }
    /// Every step name the collector can emit. A contract test keeps this in sync with the Rust analyzer.
    public static let known: [String] = [
        "Link", "Route", "Wi-Fi", "Gateway", "Internet", "Portal", "DNS", "HTTPS", "MTU", "IPv6", "Proxy", "Speed",
        "Site DNS", "Site HTTPS", "Site path"
    ]
    /// What each check means, in words a non-specialist can act on.
    public var explanation: String {
        switch step {
        case "Link": return "Is a network adapter connected and holding an address?"
        case "Route": return "Which connection does this Mac send internet traffic through?"
        case "Wi-Fi": return "How strong and clean is the Wi-Fi signal?"
        case "Gateway": return "Can this Mac reach your router, and how steadily?"
        case "Internet": return "Can this Mac reach public servers, with how much delay and loss?"
        case "Portal": return "Is a sign-in page (captive portal) blocking web traffic?"
        case "DNS": return "Do your DNS servers turn names into addresses, and how fast?"
        case "HTTPS": return "How long does a secure web request take, stage by stage?"
        case "MTU": return "How large a packet fits all the way to the internet?"
        case "IPv6": return "Does the newer IPv6 protocol work, or does it delay connections?"
        case "Proxy": return "Is this Mac configured to send web traffic through a proxy?"
        case "Speed": return "How fast is the connection, and how well does it hold up under load?"
        case "Site DNS": return "Can your DNS servers find this site?"
        case "Site HTTPS": return "How long does a secure request to this site take?"
        case "Site path": return "How is the network path to this site: delay, loss, packet size?"
        default: return step
        }
    }
}
public struct DiagnosisReport: Codable, Equatable {
    public let steps: [DiagnosisStep]
    public let verdict: String
    /// Concrete next steps for the problem the verdict names (absent in older collectors).
    public let advice: [String]
    /// Other problems besides the one the verdict names.
    public let also: [String]
    /// The site checked with `diagnose to HOST`, if any.
    public let target: String?
    public init(steps: [DiagnosisStep], verdict: String, advice: [String] = [], also: [String] = [], target: String? = nil) {
        self.steps = steps; self.verdict = verdict; self.advice = advice; self.also = also; self.target = target
    }
    private enum CodingKeys: String, CodingKey { case steps, verdict, advice, also, target }
    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        steps = try c.decode([DiagnosisStep].self, forKey: .steps)
        verdict = try c.decode(String.self, forKey: .verdict)
        advice = try c.decodeIfPresent([String].self, forKey: .advice) ?? []
        also = try c.decodeIfPresent([String].self, forKey: .also) ?? []
        target = try c.decodeIfPresent(String.self, forKey: .target)
    }
    public var worst: DiagnosisStatus {
        if steps.contains(where: { $0.status == .fail }) { return .fail }
        if steps.contains(where: { $0.status == .warn }) { return .warn }
        return .ok
    }
    public static func decode(_ text: String) throws -> DiagnosisReport {
        let report = try JSONDecoder().decode(DiagnosisReport.self, from: Data(text.utf8))
        guard !report.steps.isEmpty, report.steps.count <= 40, !report.verdict.isEmpty else { throw MacoraError.message("Malformed diagnosis") }
        return report
    }
    /// A shareable Markdown report. Redaction masks IP addresses (the public probe targets stay readable).
    public func markdown(redact: Bool) -> String {
        func clean(_ text: String) -> String { redact ? DiagnosisPrivacy.redact(text) : text }
        var out = "# MACORA network diagnosis\n\n"
        if let target { out += "Site checked: `\(clean(target))`\n\n" }
        out += "| Result | Check | Details |\n|---|---|---|\n"
        for s in steps { out += "| \(s.status.rawValue) | \(s.step) | \(clean(s.detail).replacingOccurrences(of: "|", with: "\\|")) |\n" }
        out += "\n**Verdict:** \(clean(verdict))\n"
        if !advice.isEmpty { out += "\n## What to try\n\n" + advice.enumerated().map { "\($0.offset + 1). \(clean($0.element))" }.joined(separator: "\n") + "\n" }
        if !also.isEmpty { out += "\n## Also noticed\n\n" + also.map { "- \(clean($0))" }.joined(separator: "\n") + "\n" }
        return out
    }
}

public enum SiteName {
    /// Hostnames only (letters, digits, dots, hyphens, underscores), as the CLI also requires.
    public static func isValid(_ site: String) -> Bool {
        !site.isEmpty && site.count <= 253 && !site.hasPrefix("-")
            && site.allSatisfy { $0.isASCII && ($0.isLetter || $0.isNumber || $0 == "." || $0 == "-" || $0 == "_") }
    }
}

public enum DiagnosisPrivacy {
    /// Addresses macora itself probes; they are constants, not identifying.
    public static let publicTargets: Set<String> = ["1.1.1.1", "8.8.8.8", "9.9.9.9"]
    private static let ipv4 = try! NSRegularExpression(pattern: #"\b(?:\d{1,3}\.){3}\d{1,3}\b"#)
    private static let ipv6 = try! NSRegularExpression(pattern: #"(?<![\w:])(?:[0-9A-Fa-f]{0,4}:){2,7}[0-9A-Fa-f]{0,4}(?:%[A-Za-z0-9]+)?(?![\w:])"#)
    public static func redact(_ text: String) -> String {
        var out = text
        for (regex, mask) in [(ipv6, "<IPv6>"), (ipv4, "<IPv4>")] {
            let range = NSRange(out.startIndex..., in: out)
            let matches = regex.matches(in: out, range: range).reversed()
            for m in matches {
                guard let r = Range(m.range, in: out) else { continue }
                let found = String(out[r])
                if publicTargets.contains(found) { continue }
                out.replaceSubrange(r, with: mask)
            }
        }
        return out
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
