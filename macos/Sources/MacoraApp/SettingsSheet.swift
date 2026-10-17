import SwiftUI
import MacoraCore

@MainActor struct SettingsSheet: View {
    @EnvironmentObject private var store: NetworkStore
    @Environment(\.dismiss) private var dismiss
    @State private var action: SettingAction = .dns
    @State private var target = ""
    @State private var values = ["", "", ""]
    @State private var order: [String] = []
    @State private var plan: CommandPlan?
    @State private var error: String?
    @State private var applying = false
    @State private var baseline = ""
    @State private var locations: [String] = []
    @State private var completed = false
    private var targets: [String] {
        if action == .wifiOn || action == .wifiOff || action == .forgetWiFi { return store.wifiInterfaces }
        if action == .mtu { return store.snapshot?.interfaces.map(\.name) ?? [] }
        return store.services.map(\.name)
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack {
                VStack(alignment: .leading, spacing: 4) { Text("Configure your network").font(.title2.bold()); Text("Review a change before macOS asks you to authorize it.").foregroundStyle(.secondary).font(.callout) }
                Spacer(); Button("Done") { dismiss() }.disabled(applying)
            }
            Divider()
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    Picker("Action", selection: $action) { ForEach(SettingAction.allCases) { a in Text(a.rawValue).tag(a) } }
                    if ![SettingAction.flushDNS, .serviceOrder].contains(action) && !action.usesLocation {
                        Picker(action.usesInterface ? "Interface" : "Service", selection: $target) { ForEach(targets, id: \.self) { Text($0).tag($0) } }
                        if targets.isEmpty { Text("No matching interfaces or services were detected.").foregroundStyle(.orange) }
                    }
                    if action == .serviceOrder {
                        Text("Service priority · top is preferred").font(.headline)
                        ForEach(Array(order.enumerated()), id: \.element) { index, name in
                            HStack { Text("\(index + 1). \(name)"); Spacer(); Button { order.swapAt(index, index - 1); invalidate() } label: { Image(systemName: "arrow.up") }.disabled(index == 0); Button { order.swapAt(index, index + 1); invalidate() } label: { Image(systemName: "arrow.down") }.disabled(index == order.count - 1) }
                        }
                    }
                    if action.usesLocation && !locations.isEmpty { Text("Available locations: " + locations.joined(separator: " · ")).font(.caption).foregroundStyle(.secondary) }
                    ForEach(Array(action.fields.enumerated()), id: \.offset) { index, title in
                        VStack(alignment: .leading, spacing: 6) {
                            Text(title).font(.caption).foregroundStyle(.secondary)
                            TextField(title, text: $values[index]).textFieldStyle(.roundedBorder)
                        }
                    }
                    if action == .httpProxy || action == .httpsProxy || action == .socksProxy { Text("Proxy authentication is not stored by MACORA. Use System Settings for authenticated proxy credentials.").font(.caption).foregroundStyle(.secondary) }
                    if action == .flushDNS { Text("Flushes the directory service DNS cache. mDNSResponder is not restarted.").font(.caption).foregroundStyle(.secondary) }
                    if !baseline.isEmpty {
                        DisclosureGroup("Current target configuration") { Text(baseline).font(.system(size: 11, design: .monospaced)).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading).padding(12).card() }
                    }
                    if let error { Label(error, systemImage: "exclamationmark.triangle").foregroundStyle(.orange).textSelection(.enabled) }
                    if completed { Label("Change completed. Live data has been refreshed.", systemImage: "checkmark.circle.fill").foregroundStyle(.green) }
                    if let plan {
                        VStack(alignment: .leading, spacing: 12) {
                            Label("Review this change", systemImage: "checkmark.shield").font(.headline)
                            Text(plan.consequence).font(.callout).foregroundStyle(.secondary)
                            Text(plan.shellCommand).font(.system(size: 12, design: .monospaced)).textSelection(.enabled).padding(14).frame(maxWidth: .infinity, alignment: .leading).background(Color.primary.opacity(0.04), in: RoundedRectangle(cornerRadius: 10))
                            Button {
                                applying = true; error = nil
                                Task {
                                    do { try await store.apply(plan); self.plan = nil; completed = true; await loadLocations() }
                                    catch { self.error = error.localizedDescription }
                                    applying = false
                                }
                            } label: { Label(applying ? "Waiting for macOS…" : "Authorize & Apply", systemImage: "lock.fill") }.buttonStyle(.borderedProminent).disabled(applying || store.demo)
                        }.padding(20).card()
                    }
                }.padding(2)
            }.disabled(applying)
            HStack { Label("No automatic rollback", systemImage: "info.circle").font(.caption).foregroundStyle(.secondary); Spacer(); Button("Preview Change") { preview() }.disabled(applying || store.demo) }
        }.padding(28).frame(width: 730, height: 710)
        .interactiveDismissDisabled(applying)
        .onAppear { target = targets.first ?? ""; order = store.services.map(\.name) }
        .onChange(of: action) { _, _ in target = targets.first ?? ""; values = ["", "", ""]; invalidate() }
        .onChange(of: values) { _, _ in invalidate() }
        .onChange(of: target) { _, _ in invalidate() }
        .task(id: target + action.rawValue) { await readBaseline() }
        .task { await loadLocations() }
    }
    private func invalidate() { plan = nil; error = nil; completed = false }
    private func preview() {
        do {
            plan = try CommandPlan.make(action: action, service: target, values: action == .serviceOrder ? order : values,
                                        services: store.services, interfaces: (action == .wifiOn || action == .wifiOff || action == .forgetWiFi) ? store.wifiInterfaces : store.snapshot?.interfaces.map(\.name) ?? [], locations: locations)
            error = nil
        } catch { self.error = error.localizedDescription; plan = nil }
    }
    private func loadLocations() async {
        guard !store.demo else { return }
        do { locations = try await ProcessRunner.run("/usr/sbin/networksetup", ["-listlocations"]).components(separatedBy: .newlines).filter { !$0.isEmpty } }
        catch { self.error = error.localizedDescription }
    }
    private func readBaseline() async {
        baseline = ""
        guard !store.demo else { return }
        let selected = target
        if action.usesInterface {
            guard targets.contains(selected) else { return }
            do {
                let flag = action == .mtu ? "-getMTU" : "-listpreferredwirelessnetworks"
                let info = try await ProcessRunner.run("/usr/sbin/networksetup", [flag, selected])
                if target == selected { baseline = info }
            } catch { if target == selected { baseline = error.localizedDescription } }
            return
        }
        guard store.services.contains(where: { $0.name == target }) else { return }
        do {
            let info = try await ProcessRunner.run("/usr/sbin/networksetup", ["-getinfo", selected])
            let dns = try await ProcessRunner.run("/usr/sbin/networksetup", ["-getdnsservers", selected])
            guard target == selected else { return }
            baseline = "\(info)\nDNS servers:\n\(dns)"
        } catch { if target == selected { baseline = error.localizedDescription } }
    }
}
