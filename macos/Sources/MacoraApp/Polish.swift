import SwiftUI
import AppKit
import MacoraCore

/// Accent choices stored by name so the preference survives renames of the colors.
enum AccentChoice: String, CaseIterable, Identifiable {
    case blue, teal, purple, orange, pink
    var id: String { rawValue }
    var title: String { rawValue.capitalized }
    var color: Color {
        switch self {
        case .blue: return MacoraDesign.blue
        case .teal: return Color(red: 0.0, green: 0.62, blue: 0.62)
        case .purple: return Color(red: 0.55, green: 0.33, blue: 0.9)
        case .orange: return Color(red: 0.92, green: 0.45, blue: 0.0)
        case .pink: return Color(red: 0.86, green: 0.2, blue: 0.5)
        }
    }
    static func named(_ name: String) -> AccentChoice { AccentChoice(rawValue: name) ?? .blue }
}

struct Sparkline: View {
    let values: [Double]
    var color: Color = MacoraDesign.blue
    var body: some View {
        GeometryReader { proxy in
            if values.count >= 2 {
                let w = proxy.size.width, h = proxy.size.height
                let points = values.enumerated().map { CGPoint(x: w * CGFloat($0.offset) / CGFloat(values.count - 1), y: h - (h - 2) * CGFloat($0.element) - 1) }
                ZStack {
                    Path { p in p.move(to: CGPoint(x: 0, y: h)); points.forEach { p.addLine(to: $0) }; p.addLine(to: CGPoint(x: w, y: h)); p.closeSubpath() }.fill(color.opacity(0.12))
                    Path { p in p.move(to: points[0]); points.dropFirst().forEach { p.addLine(to: $0) } }.stroke(color, style: StrokeStyle(lineWidth: 1.5, lineCap: .round, lineJoin: .round))
                }
            } else {
                Rectangle().fill(Color.primary.opacity(0.06)).frame(height: 1).frame(maxHeight: .infinity, alignment: .bottom)
            }
        }.accessibilityHidden(true)
    }
}

@MainActor struct DiagnosisWizard: View {
    @EnvironmentObject private var store: NetworkStore
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var quick = false
    @State private var speed = false
    @State private var site = ""
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Why is my network slow?").font(.title2.bold())
                    Text("Checks each layer from your adapter outward: link, router, internet, sign-in page, DNS, HTTPS, packet size, Wi-Fi and IPv6. It sends a few packets and DNS lookups, only when you press the button.").font(.callout).foregroundStyle(.secondary)
                }
                Spacer()
                Button { Task { await store.runDiagnosis(site: site, quick: quick, speed: speed) } } label: {
                    if store.diagnosing { ProgressView().controlSize(.small).padding(.horizontal, 10) } else { Label(store.diagnosis == nil ? "Run check" : "Run again", systemImage: "waveform.path.ecg") }
                }.buttonStyle(.borderedProminent).disabled(store.diagnosing)
            }
            HStack(spacing: 12) {
                TextField("Check a site too (e.g. github.com)", text: $site).textFieldStyle(.roundedBorder).frame(maxWidth: 300)
                    .onSubmit { Task { await store.runDiagnosis(site: site, quick: quick, speed: speed) } }
                Toggle("Quick (about 2 s)", isOn: $quick).toggleStyle(.checkbox).help("Skips Wi-Fi details and the packet-size search.")
                Toggle("Speed test (about 15 s)", isOn: $speed).toggleStyle(.checkbox).help("Runs Apple's networkQuality, which uses real bandwidth and Apple's servers.")
                Spacer()
            }.controlSize(.small).disabled(store.diagnosing)
            if let report = store.diagnosis {
                VStack(spacing: 0) {
                    ForEach(report.steps) { step in
                        HStack(alignment: .top, spacing: 14) {
                            Image(systemName: step.status.symbol).foregroundStyle(color(step.status)).font(.title3).frame(width: 26)
                            VStack(alignment: .leading, spacing: 3) {
                                HStack { Text(step.step).font(.headline); Text(step.status.label).font(.caption).foregroundStyle(color(step.status)) }
                                Text(step.explanation).font(.caption).foregroundStyle(.secondary)
                                Text(store.presentDiagnosis(step.detail)).font(.callout.monospaced()).textSelection(.enabled)
                            }
                            Spacer()
                        }.padding(.vertical, 12).accessibilityElement(children: .combine)
                        if step.id != report.steps.last?.id { Divider() }
                    }
                }.padding(.horizontal, 18).padding(.vertical, 6).card()
                HStack(alignment: .top, spacing: 12) {
                    Image(systemName: report.worst == .ok ? "checkmark.seal.fill" : "lightbulb.fill").foregroundStyle(color(report.worst)).font(.title2)
                    VStack(alignment: .leading, spacing: 4) { Text("What this means").font(.headline); Text(store.presentDiagnosis(report.verdict)).font(.callout) }
                }.padding(18).frame(maxWidth: .infinity, alignment: .leading).background(color(report.worst).opacity(0.10), in: RoundedRectangle(cornerRadius: 16))
                    .transition(reduceMotion ? .identity : .opacity.combined(with: .move(edge: .bottom)))
                if !report.advice.isEmpty {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("What to try").font(.headline)
                        ForEach(Array(report.advice.enumerated()), id: \.offset) { i, tip in
                            HStack(alignment: .top, spacing: 10) { Text("\(i + 1).").font(.callout.monospacedDigit()).foregroundStyle(.secondary); Text(store.presentDiagnosis(tip)).font(.callout) }
                        }
                    }.padding(18).frame(maxWidth: .infinity, alignment: .leading).card()
                }
                if !report.also.isEmpty {
                    VStack(alignment: .leading, spacing: 6) {
                        Text("Also noticed").font(.subheadline.weight(.semibold)).foregroundStyle(.secondary)
                        ForEach(Array(report.also.enumerated()), id: \.offset) { _, line in Text("· " + store.presentDiagnosis(line)).font(.caption).foregroundStyle(.secondary) }
                    }
                }
                Button { NSPasteboard.general.clearContents(); NSPasteboard.general.setString(report.markdown(redact: true), forType: .string) } label: { Label("Copy report (addresses masked)", systemImage: "doc.on.doc") }
            } else if !store.diagnosing {
                ContentUnavailableView("Nothing checked yet", systemImage: "waveform.path.ecg", description: Text("Press Run check. Nothing is sent until you do."))
            }
        }.animation(reduceMotion ? nil : .easeOut(duration: 0.25), value: store.diagnosis)
    }
    private func color(_ s: DiagnosisStatus) -> Color {
        switch s { case .ok: return .green; case .warn: return .orange; case .fail: return .red; case .skip: return .secondary; case .info: return .blue }
    }
}

struct OnboardingCard: View {
    @Binding var dismissed: Bool
    var open: (String) -> Void
    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack { Label("Welcome to MACORA", systemImage: "sparkles").font(.headline); Spacer(); Button { dismissed = true } label: { Image(systemName: "xmark") }.buttonStyle(.plain).help("Dismiss").accessibilityLabel("Dismiss welcome") }
            Text("Everything here is read-only until you press an explicit button. Three good places to start:").font(.callout).foregroundStyle(.secondary)
            HStack(spacing: 12) {
                tip("Ask “why slow?”", "Walk your connection layer by layer.", "waveform.path.ecg") { open("Diagnostics") }
                tip("Jump anywhere", "⌘K searches every page and action.", "command") { open("palette") }
                tip("Learn the keys", "⌘/ lists every shortcut.", "keyboard") { open("shortcuts") }
            }
        }.padding(20).card()
    }
    private func tip(_ title: String, _ detail: String, _ symbol: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            VStack(alignment: .leading, spacing: 6) { Image(systemName: symbol).foregroundStyle(MacoraDesign.blue); Text(title).font(.subheadline.weight(.semibold)); Text(detail).font(.caption).foregroundStyle(.secondary) }
                .frame(maxWidth: .infinity, alignment: .leading).padding(14).background(Color.primary.opacity(0.04), in: RoundedRectangle(cornerRadius: 12))
        }.buttonStyle(.plain)
    }
}

struct ShortcutSheet: View {
    @Environment(\.dismiss) private var dismiss
    private let groups: [(String, [(String, String)])] = [
        ("Navigate", [("⌘K", "Command palette"), ("⌘/", "This cheat sheet"), ("⌘,", "Workspace preferences")]),
        ("Monitor", [("⌘R", "Refresh now"), ("⇧⌘P", "Pause or resume"), ("⇧⌘D", "Run “why is my network slow?”")]),
        ("Share", [("⇧⌘E", "Export redacted snapshot"), ("⇧⌘C", "Copy redacted summary")])
    ]
    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack { Text("Keyboard shortcuts").font(.title2.bold()); Spacer(); Button("Done") { dismiss() }.keyboardShortcut(.cancelAction) }
            ForEach(groups, id: \.0) { group in
                VStack(alignment: .leading, spacing: 8) {
                    Text(group.0.uppercased()).font(.caption.weight(.semibold)).foregroundStyle(.secondary)
                    ForEach(group.1, id: \.0) { row in
                        HStack { Text(row.1); Spacer(); Text(row.0).font(.body.monospaced()).padding(.horizontal, 8).padding(.vertical, 2).background(Color.primary.opacity(0.07), in: RoundedRectangle(cornerRadius: 6)) }
                        Divider()
                    }
                }
            }
        }.padding(26).frame(width: 440)
    }
}

@MainActor struct MenuBarPanel: View {
    @EnvironmentObject private var store: NetworkStore
    @Environment(\.openWindow) private var openWindow
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Image(systemName: "waveform.path").foregroundStyle(.white).frame(width: 28, height: 28).background(MacoraDesign.blue.gradient, in: RoundedRectangle(cornerRadius: 8))
                VStack(alignment: .leading, spacing: 1) { Text("MACORA").font(.headline); Text(store.demo ? "Demo data" : store.paused ? "Paused" : "This Mac").font(.caption).foregroundStyle(.secondary) }
                Spacer()
            }
            if let error = store.error { Label("Stale: " + error, systemImage: "exclamationmark.triangle").font(.caption).foregroundStyle(.orange) }
            HStack(spacing: 18) {
                rate("Download", store.latest?.rx, "arrow.down")
                rate("Upload", store.latest?.tx, "arrow.up")
            }
            Sparkline(values: store.history.suffix(40).map(\.rx).normalizedForPanel).frame(height: 34)
            Divider()
            ForEach(store.networkInterfaces.prefix(4)) { i in
                HStack { Image(systemName: i.symbol).frame(width: 18); Text(i.label); Spacer(); Circle().fill(i.active ? Color.green : Color.gray).frame(width: 7, height: 7) }.font(.callout)
            }
            if let report = store.diagnosis { Label(report.verdict, systemImage: report.worst.symbol).font(.caption).foregroundStyle(.secondary).lineLimit(3) }
            Divider()
            HStack {
                Button { Task { await store.runDiagnosis() } } label: { if store.diagnosing { ProgressView().controlSize(.small) } else { Text("Why slow?") } }.disabled(store.diagnosing)
                Button("Copy summary") { NSPasteboard.general.clearContents(); NSPasteboard.general.setString(store.summary(redact: true), forType: .string) }
                Spacer()
                Button("Open") { openWindow(id: "main"); NSApp.activate(ignoringOtherApps: true) }.buttonStyle(.borderedProminent)
            }.controlSize(.small)
            Button(store.paused ? "Resume monitoring" : "Pause monitoring") { store.paused.toggle() }.buttonStyle(.plain).font(.caption).foregroundStyle(.secondary)
            Button("Quit MACORA") { NSApp.terminate(nil) }.buttonStyle(.plain).font(.caption).foregroundStyle(.secondary)
        }.padding(16).frame(width: 300)
    }
    private func rate(_ label: String, _ value: Double?, _ symbol: String) -> some View {
        VStack(alignment: .leading, spacing: 2) { Label(label, systemImage: symbol).font(.caption).foregroundStyle(.secondary); Text(value.map { Traffic.format($0) } ?? "Sampling…").font(.system(.title3, design: .rounded).weight(.semibold)).lineLimit(1).minimumScaleFactor(0.7) }
    }
}
private extension Array where Element == Double {
    var normalizedForPanel: [Double] { guard let peak = self.max(), peak > 0 else { return map { _ in 0 } }; return map { $0 / peak } }
}
