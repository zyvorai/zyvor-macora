import SwiftUI
import MacoraCore

@MainActor struct NetworkTopology: View {
    @EnvironmentObject private var store: NetworkStore
    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            Text("Connections, made visible.").font(.largeTitle.bold())
            Text("Observed interface roles and bridge members. This view does not infer a route to the internet or guess which app owns a tunnel.").foregroundStyle(.secondary)
            ForEach(["Network", "Bridges", "Tunnels", "System", "Virtual", "Loopback"], id: \.self) { group in
                let interfaces = store.snapshot?.interfaces.filter { $0.group == group } ?? []
                if !interfaces.isEmpty {
                    VStack(alignment: .leading, spacing: 14) {
                        Text(group.uppercased()).font(.caption.weight(.semibold)).foregroundStyle(.secondary).tracking(1)
                        ForEach(interfaces) { i in
                            HStack(alignment: .top, spacing: 16) {
                                Image(systemName: i.symbol).foregroundStyle(MacoraDesign.blue).frame(width: 30)
                                VStack(alignment: .leading, spacing: 6) {
                                    HStack { Text(i.label).font(.headline); Text(i.name).font(.caption.monospaced()).foregroundStyle(.secondary); Spacer(); Text(i.active ? "Link active" : "Idle/down").font(.caption).foregroundStyle(.secondary) }
                                    if !i.members.isEmpty {
                                        HStack { Image(systemName: "arrow.turn.down.right").foregroundStyle(.secondary); ForEach(i.members, id: \.self) { Text($0).font(.caption.monospaced()).padding(.horizontal, 12).padding(.vertical, 6).background(MacoraDesign.blue.opacity(0.08), in: Capsule()) } }
                                    }
                                    if group == "Network", let a = i.addresses.first(where: { !$0.address.contains(":") }) { Text(store.present(a.display)).font(.caption.monospaced()).foregroundStyle(.secondary) }
                                }
                            }.padding(.vertical, 8)
                            if i.id != interfaces.last?.id { Divider() }
                        }
                    }.padding(22).card()
                }
            }
        }
    }
}
@MainActor struct BaselineComparison: View {
    @EnvironmentObject private var store: NetworkStore
    @State private var query = ""
    private var changes: [ConfigurationChange] { store.comparison.filter { query.isEmpty || $0.summary.localizedCaseInsensitiveContains(query) } }
    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            HStack {
                VStack(alignment: .leading, spacing: 7) { Text("What changed? Exactly.").font(.largeTitle.bold()); Text("Interface configuration comparison. Traffic counters are excluded.").foregroundStyle(.secondary) }
                Spacer(); Button("Capture Baseline") { store.captureBaseline() }.buttonStyle(.borderedProminent).disabled(store.snapshot == nil)
            }
            HStack {
                Label(store.baselineLabel, systemImage: "flag").font(.callout)
                if let at = store.baselineDate { Text(at.formatted()).font(.caption).foregroundStyle(.secondary) }
                Spacer(); Button("Import JSON baseline…") { store.importBaseline() }
            }.padding(18).card()
            HStack { TextField("Filter configuration changes", text: $query).textFieldStyle(.roundedBorder); Text("\(store.comparison.count) changes").font(.caption).foregroundStyle(.secondary) }
            if changes.isEmpty { ContentUnavailableView(store.comparison.isEmpty ? "No configuration drift" : "No matching changes", systemImage: "rectangle.on.rectangle", description: Text("Use Capture Baseline before a change, or import a full MACORA JSON snapshot. An unchanged state is not a connectivity guarantee.")) }
            ForEach(changes) { c in
                VStack(alignment: .leading, spacing: 12) {
                    HStack { Text(c.interface).font(.headline.monospaced()); Text(c.field).font(.caption).foregroundStyle(.secondary) }
                    HStack(alignment: .top, spacing: 20) {
                        VStack(alignment: .leading, spacing: 6) { Text("BEFORE").font(.caption2).foregroundStyle(.secondary); Text(store.present(c.before.isEmpty ? "None" : c.before)).textSelection(.enabled) }.frame(maxWidth: .infinity, alignment: .leading)
                        Image(systemName: "arrow.right").foregroundStyle(.secondary)
                        VStack(alignment: .leading, spacing: 6) { Text("CURRENT").font(.caption2).foregroundStyle(.secondary); Text(store.present(c.after.isEmpty ? "None" : c.after)).textSelection(.enabled) }.frame(maxWidth: .infinity, alignment: .leading)
                    }.font(.system(size: 12, design: .monospaced))
                }.padding(22).card()
            }
        }
    }
}
