import SwiftUI
import AppKit
import MacoraCore

@MainActor @main struct MacoraApp: App {
    @StateObject private var store = NetworkStore()
    @AppStorage("macora.appearance") private var appearance = "system"
    var body: some Scene {
        WindowGroup("MACORA", id: "main") {
            ContentView().environmentObject(store).frame(minWidth: 1100, minHeight: 740)
                .preferredColorScheme(appearance == "light" ? .light : appearance == "dark" ? .dark : nil)
                .tint(MacoraDesign.blue)
                .task { store.startMonitoring() }
        }
        .windowStyle(.hiddenTitleBar)
        .defaultSize(width: 1320, height: 860)
        .commands {
            CommandGroup(after: .newItem) {
                Button("Command Palette…") { NotificationCenter.default.post(name: .macoraPalette, object: nil) }.keyboardShortcut("k")
                Button("Refresh Network") { Task { await store.refresh() } }.keyboardShortcut("r")
                Button(store.paused ? "Resume Monitoring" : "Pause Monitoring") { store.paused.toggle() }.keyboardShortcut("p", modifiers: [.command, .shift])
                Button("Export Redacted Snapshot…") { Task { await store.export(redact: true) } }.keyboardShortcut("e", modifiers: [.command, .shift])
            }
        }
        MenuBarExtra("MACORA", systemImage: "network") { MenuBarStatus().environmentObject(store) }
        Settings { WorkspacePreferences().environmentObject(store) }
    }
}
extension Notification.Name { static let macoraPalette = Notification.Name("MacoraCommandPalette") }
@MainActor struct MenuBarStatus: View {
    @EnvironmentObject private var store: NetworkStore
    @Environment(\.openWindow) private var openWindow
    var body: some View {
        Text(store.demo ? "MACORA · Demo data" : "MACORA · This Mac")
        if let at = store.lastUpdated { Text("Updated " + at.formatted(date: .omitted, time: .standard)) }
        if let error = store.error { Text("Stale: " + error) }
        Divider()
        ForEach(store.networkInterfaces) { i in Label(i.label + " · " + (i.active ? "Link active" : "Idle/down"), systemImage: i.symbol) }
        if let rate = store.latest { Text("RX " + Traffic.format(rate.rx) + " · TX " + Traffic.format(rate.tx)) }
        Divider()
        Button(store.paused ? "Resume monitoring" : "Pause monitoring") { store.paused.toggle() }
        Button("Open MACORA") { openWindow(id: "main"); NSApp.activate(ignoringOtherApps: true) }
        Button("Quit MACORA") { NSApp.terminate(nil) }
    }
}
