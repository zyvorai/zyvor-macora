#if canImport(AppIntents)
import AppIntents
import Foundation
import MacoraCore

private func collectorPath() -> String {
    Bundle.main.url(forResource: "macora", withExtension: nil)?.path ?? "/usr/local/bin/macora"
}

struct NetworkStatusIntent: AppIntent {
    static let title: LocalizedStringResource = "Network status"
    static let description = IntentDescription("Summarizes this Mac's network adapters with IP addresses masked. Read-only.")
    func perform() async throws -> some IntentResult & ReturnsValue<String> {
        let json = try await ProcessRunner.run(collectorPath(), ["show", "json", "all", "redact"])
        let snapshot = try NetworkSnapshot.decode(Data(json.utf8))
        return .result(value: StatusSummary.text(snapshot: snapshot, rx: nil, tx: nil, report: nil, redact: true))
    }
}

struct RunDiagnosisIntent: AppIntent {
    static let title: LocalizedStringResource = "Why is my network slow?"
    static let description = IntentDescription("Checks adapter, router, internet, DNS and MTU, then explains the result. Sends a few packets.")
    func perform() async throws -> some IntentResult & ReturnsValue<String> {
        let text = try await ProcessRunner.run(collectorPath(), ["diagnose", "json"], timeout: 45, acceptedStatuses: [0, 1])
        return .result(value: try DiagnosisReport.decode(text).verdict)
    }
}

struct MacoraShortcuts: AppShortcutsProvider {
    static var appShortcuts: [AppShortcut] {
        AppShortcut(intent: NetworkStatusIntent(), phrases: ["Show my network status in \(.applicationName)"], shortTitle: "Network status", systemImageName: "network")
        AppShortcut(intent: RunDiagnosisIntent(), phrases: ["Why is my network slow in \(.applicationName)"], shortTitle: "Why slow?", systemImageName: "waveform.path.ecg")
    }
}
#endif
