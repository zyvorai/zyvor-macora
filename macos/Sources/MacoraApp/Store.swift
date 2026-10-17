import SwiftUI
import AppKit
import MacoraCore
import UniformTypeIdentifiers

@MainActor final class NetworkStore: ObservableObject {
    @Published var snapshot: NetworkSnapshot?
    @Published var services: [Service] = []
    @Published var history: [TrafficPoint] = []
    @Published var findings: [String] = []
    @Published var error: String?
    @Published var actionError: String?
    @Published var lastUpdated: Date?
    @Published var refreshing = false
    @Published var paused = false
    @Published var events: [String] = []
    @Published var baseline: NetworkSnapshot?
    @Published var baselineLabel = "Session start"
    @Published var baselineDate: Date?
    @Published var timeline: [NetworkTimelineEvent] = []
    @Published var diagnosis: DiagnosisReport?
    @Published var diagnosing = false
    @Published var rateHistory = RateHistory()
    private var demoTick = 0
    private var monitor: Task<Void, Never>?
    var comparison: [ConfigurationChange] {
        guard let baseline, let snapshot else { return [] }
        return SnapshotComparison.changes(from: baseline, to: snapshot)
    }
    func startMonitoring() {
        guard monitor == nil else { return }
        monitor = Task {
            await loadServices()
            while !Task.isCancelled {
                await refresh()
                let configured = UserDefaults.standard.double(forKey: "macora.refreshSeconds")
                let interval = configured.isFinite && configured >= 1 ? min(configured, 10) : 2
                try? await Task.sleep(for: .seconds(interval))
            }
        }
    }
    func present(_ text: String) -> String {
        guard UserDefaults.standard.bool(forKey: "macora.maskAddresses") else { return text }
        return SnapshotPrivacy.redacted(text, snapshots: [snapshot, baseline].compactMap { $0 })
    }
    /// Diagnosis text also names the router and DNS servers, which are not in the interface list, so
    /// masking uses address patterns in addition to the interface addresses.
    func presentDiagnosis(_ text: String) -> String {
        let text = present(text)
        return UserDefaults.standard.bool(forKey: "macora.maskAddresses") ? DiagnosisPrivacy.redact(text) : text
    }
    func present(_ change: ConfigurationChange) -> String {
        if UserDefaults.standard.bool(forKey: "macora.maskAddresses"), ["Addresses", "MAC"].contains(change.field) {
            return "\(change.interface) · \(change.field): <masked> → <masked>"
        }
        return present(change.summary)
    }
    func captureBaseline() {
        guard let snapshot else { return }
        baseline = snapshot; baselineLabel = "Captured in this session"; baselineDate = Date()
    }
    func importBaseline() {
        let panel = NSOpenPanel(); panel.allowedContentTypes = [.json]; panel.allowsMultipleSelection = false
        guard panel.runModal() == .OK, let url = panel.url else { return }
        do {
            let size = try url.resourceValues(forKeys: [.fileSizeKey]).fileSize ?? 0
            guard size <= 8 * 1024 * 1024 else { throw MacoraError.message("Baseline exceeds 8 MiB") }
            baseline = try NetworkSnapshot.decode(Data(contentsOf: url))
            baselineLabel = url.lastPathComponent; baselineDate = nil
        } catch { self.actionError = error.localizedDescription }
    }
    let demo = ProcessInfo.processInfo.arguments.contains("--demo")
    private var previous: NetworkSnapshot?
    private var previousTime: Date?
    var cli: String { Bundle.main.url(forResource: "macora", withExtension: nil)?.path ?? "/usr/local/bin/macora" }
    var latest: TrafficPoint? { history.last }
    var networkInterfaces: [NetworkInterface] { snapshot?.interfaces.filter { $0.group == "Network" } ?? [] }
    var wifiInterfaces: [String] { networkInterfaces.filter { $0.label.localizedCaseInsensitiveContains("Wi-Fi") || $0.label.localizedCaseInsensitiveContains("AirPort") }.map(\.name) }

    func refresh() async {
        guard !refreshing, !paused else { return }
        refreshing = true
        defer { refreshing = false }
        do {
            let text: String
            if demo {
                guard let url = Bundle.main.url(forResource: "demo", withExtension: "json") else { throw MacoraError.message("Demo fixture is missing") }
                text = try String(contentsOf: url, encoding: .utf8)
            } else { text = try await ProcessRunner.run(cli, ["show", "json", "all"]) }
            let next = try NetworkSnapshot.decode(Data(text.utf8))
            let now = Date()
            if baseline == nil { baseline = next; baselineDate = now }
            if let old = previous, let at = previousTime {
                let changes = SnapshotComparison.changes(from: old, to: next)
                timeline.insert(contentsOf: changes.map { NetworkTimelineEvent(date: now, change: $0) }, at: 0)
                if timeline.count > 200 { timeline.removeLast(timeline.count - 200) }
                var rx = 0.0, tx = 0.0
                demoTick += 1
                // Sum physical network adapters only: bridges and tunnels can double count traffic.
                for i in next.interfaces where i.group == "Network" {
                    if let a = old.interfaces.first(where: { $0.name == i.name })?.counters, let b = i.counters {
                        let r = Traffic.rates(previous: a, current: b, elapsed: now.timeIntervalSince(at)); rx += r.0; tx += r.1
                        rateHistory.append(i.name, r.0 + r.1)
                    }
                }
                if demo && rx == 0 && tx == 0 {
                    // The demo fixture has constant counters; draw a clearly simulated wave so the UI has something to show.
                    let t = Double(demoTick)
                    rx = 2_400_000 * (0.55 + 0.45 * sin(t / 3)); tx = 480_000 * (0.5 + 0.5 * sin(t / 4 + 1))
                    rateHistory.append("en0", rx + tx)
                }
                if demo || next.interfaces.contains(where: { $0.group == "Network" && $0.counters != nil }) { history.append(TrafficPoint(date: now, rx: rx, tx: tx)) }
                if history.count > 90 { history.removeFirst(history.count - 90) }
            }
            previous = next; previousTime = now; snapshot = next; lastUpdated = now; error = nil
        } catch { self.error = error.localizedDescription }
    }
    func loadServices() async {
        guard !demo else { services = Service.parse("Header\nWi-Fi\nThunderbolt Bridge\n"); return }
        do { services = Service.parse(try await ProcessRunner.run("/usr/sbin/networksetup", ["-listallnetworkservices"])) }
        catch { self.actionError = error.localizedDescription }
    }
    func inspect(_ command: String) async throws -> String {
        if demo { return "DEMO MODE — Live \(command) inspection is disabled. Launch without --demo to inspect this Mac." }
        return try await ProcessRunner.run(cli, [command])
    }
    func diagnose() async {
        do {
            if demo { findings = ["DEMO MODE — findings require a live Mac."]; return }
            let text = try await ProcessRunner.run(cli, ["doctor", "json"])
            findings = try JSONDecoder().decode([String: [String]].self, from: Data(text.utf8))["findings"] ?? []
        } catch { self.actionError = error.localizedDescription }
    }
    func runDiagnosis(site: String = "", quick: Bool = false, speed: Bool = false) async {
        guard !diagnosing else { return }
        diagnosing = true
        defer { diagnosing = false }
        let site = site.trimmingCharacters(in: .whitespacesAndNewlines)
        if !site.isEmpty, !SiteName.isValid(site) { actionError = "Enter a site name like github.com (letters, digits, dots and hyphens)."; return }
        if demo {
            try? await Task.sleep(for: .milliseconds(600))
            diagnosis = DiagnosisReport(steps: [
                .init(step: "Link", status: .ok, detail: "Simulated: en0 (Wi-Fi): a routable IPv4 address, global IPv6"),
                .init(step: "Wi-Fi", status: .ok, detail: "Simulated: 5GHz ch 40 (80MHz) · 802.11ax · signal -58 dBm, noise -94 dBm (SNR 36 dB)"),
                .init(step: "Gateway", status: .ok, detail: "Simulated: 0% loss, 3.7 ms avg, jitter 0.6 ms", ms: 3.7),
                .init(step: "Internet", status: .ok, detail: "Simulated: 1.1.1.1 9 ms · 8.8.8.8 11 ms", ms: 9),
                .init(step: "DNS", status: .warn, detail: "Simulated: your DNS server answers in 412 ms; 1.1.1.1 answers in 14 ms", ms: 412),
                .init(step: "HTTPS", status: .ok, detail: "Simulated: dns 14 · tcp 11 · tls 13 · first byte 23 ms", ms: 95),
                .init(step: "MTU", status: .warn, detail: "Simulated: path MTU is 1480 bytes, below this Mac's 1500"),
                .init(step: "IPv6", status: .ok, detail: "Simulated: IPv6 reaches the internet")
            ], verdict: "Simulated result. Connected, but DNS is slow (your server answers in 412 ms). A different resolver may speed up page loads.",
               advice: ["Try 1.1.1.1 or 8.8.8.8: System Settings → Network → your connection → Details → DNS."],
               also: ["Packets larger than 1480 bytes are dropped on the way to the internet."], target: site.isEmpty ? nil : site)
            return
        }
        var args = ["diagnose", "json"]
        if quick { args.append("quick") }
        if speed { args.append("speed") }
        if !site.isEmpty { args += ["to", site] }
        do {
            // diagnose exits 1 when a layer fails but still prints the full report.
            let text = try await ProcessRunner.run(cli, args, timeout: speed ? 120 : 60, acceptedStatuses: [0, 1])
            diagnosis = try DiagnosisReport.decode(text)
        } catch { actionError = error.localizedDescription }
    }
    func summary(redact: Bool) -> String {
        StatusSummary.text(snapshot: snapshot, rx: latest?.rx, tx: latest?.tx, report: diagnosis, redact: redact)
    }
    func apply(_ plan: CommandPlan) async throws {
        guard !demo else { throw MacoraError.message("Changes are disabled in demo mode") }
        let result = try await ProcessRunner.apply(plan)
        events.insert("\(Date().formatted(date: .omitted, time: .standard)) · \(plan.title) · \(result.isEmpty ? "Completed" : result)", at: 0)
        await loadServices(); await refresh()
    }
    func export(redact: Bool) async {
        do {
            let text: String
            if demo, let snapshot {
                let original = String(decoding: try JSONEncoder().encode(snapshot), as: UTF8.self)
                text = redact ? SnapshotPrivacy.redacted(original, snapshots: [snapshot]) : original
            }
            else { text = try await ProcessRunner.run(cli, ["show", "all", "json"] + (redact ? ["redact"] : [])) }
            let panel = NSSavePanel(); panel.nameFieldStringValue = redact ? "macora-redacted.json" : "macora-snapshot.json"
            if panel.runModal() == .OK, let url = panel.url {
                try text.write(to: url, atomically: true, encoding: .utf8)
                try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: url.path)
            }
        } catch { self.actionError = error.localizedDescription }
    }
}
