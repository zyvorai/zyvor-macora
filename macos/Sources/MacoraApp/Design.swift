import SwiftUI

/// Fresh native tokens inspired by Nuvora's public visual contract.
/// No Nuvora source, stylesheet or assets are embedded in this Apache project.
enum MacoraDesign {
    static let blue = Color(red: 0, green: 0.443, blue: 0.89)
    static func page(_ scheme: ColorScheme) -> Color { scheme == .dark ? .black : .white }
    static func surface(_ scheme: ColorScheme) -> Color { scheme == .dark ? Color(red: 0.114, green: 0.114, blue: 0.122) : .white }
    static func primary(_ scheme: ColorScheme) -> Color { scheme == .dark ? Color(white: 0.96) : Color(white: 0.114) }
    static func secondary(_ scheme: ColorScheme) -> Color { scheme == .dark ? Color(white: 0.65) : Color(white: 0.43) }
}
@MainActor struct WorkspacePreferences: View {
    @Environment(\.dismiss) private var dismiss
    @AppStorage("macora.appearance") private var appearance = "system"
    @AppStorage("macora.density") private var density = "comfortable"
    @AppStorage("macora.refreshSeconds") private var refreshSeconds = 2.0
    @AppStorage("macora.maskAddresses") private var maskAddresses = false
    @EnvironmentObject private var store: NetworkStore
    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            HStack { Text("Your workspace.").font(.largeTitle.bold()); Spacer(); Button("Done") { dismiss() } }
            Text("Display preferences stay on this Mac. They do not change network settings.").foregroundStyle(.secondary)
            Form {
                Picker("Appearance", selection: $appearance) { Text("System").tag("system"); Text("Light").tag("light"); Text("Dark").tag("dark") }
                Picker("Density", selection: $density) { Text("Comfortable").tag("comfortable"); Text("Compact").tag("compact") }
                Picker("Refresh interval", selection: $refreshSeconds) { Text("1 second").tag(1.0); Text("2 seconds").tag(2.0); Text("5 seconds").tag(5.0); Text("10 seconds").tag(10.0) }
                Toggle("Mask modeled IP and MAC addresses in workspace views", isOn: $maskAddresses)
            }.formStyle(.grouped)
            Text("Masking covers modeled addresses and MACs. Raw DNS, routes, socket inspectors and tool results remain unredacted.").font(.caption).foregroundStyle(.secondary)
            Button("Clear in-memory traffic and timeline") { store.history.removeAll(); store.timeline.removeAll() }
        }.padding(28).frame(width: 640, height: 450)
            .onChange(of: maskAddresses) { _, _ in store.objectWillChange.send() }
    }
}
