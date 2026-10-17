import SwiftUI
import AppKit
import MacoraCore

@MainActor @main struct MacoraApp: App {
    @StateObject private var store = NetworkStore()
    @AppStorage("macora.appearance") private var appearance = "system"
    @AppStorage("macora.accent") private var accent = "blue"
    var body: some Scene {
        WindowGroup("MACORA", id: "main") {
            ContentView().environmentObject(store).frame(minWidth: 1100, minHeight: 740)
                .preferredColorScheme(appearance == "light" ? .light : appearance == "dark" ? .dark : nil)
                .tint(AccentChoice.named(accent).color)
                .task { store.startMonitoring() }
        }
        .windowStyle(.hiddenTitleBar)
        .defaultSize(width: 1320, height: 860)
        .commands {
            CommandGroup(after: .newItem) {
                Button("Command Palette…") { NotificationCenter.default.post(name: .macoraPalette, object: nil) }.keyboardShortcut("k")
                Button("Refresh Network") { Task { await store.refresh() } }.keyboardShortcut("r")
                Button(store.paused ? "Resume Monitoring" : "Pause Monitoring") { store.paused.toggle() }.keyboardShortcut("p", modifiers: [.command, .shift])
                Button("Keyboard Shortcuts") { NotificationCenter.default.post(name: .macoraShortcuts, object: nil) }.keyboardShortcut("/")
                Button("Why Is My Network Slow?") { Task { await store.runDiagnosis() }; NotificationCenter.default.post(name: .macoraDiagnose, object: nil) }.keyboardShortcut("d", modifiers: [.command, .shift])
                Button("Copy Redacted Summary") { NSPasteboard.general.clearContents(); NSPasteboard.general.setString(store.summary(redact: true), forType: .string) }.keyboardShortcut("c", modifiers: [.command, .shift])
                Button("Export Redacted Snapshot…") { Task { await store.export(redact: true) } }.keyboardShortcut("e", modifiers: [.command, .shift])
            }
        }
        MenuBarExtra("MACORA", systemImage: "network") { MenuBarPanel().environmentObject(store) }.menuBarExtraStyle(.window)
        Settings { WorkspacePreferences().environmentObject(store) }
    }
}
extension Notification.Name {
    static let macoraPalette = Notification.Name("MacoraCommandPalette")
    static let macoraShortcuts = Notification.Name("MacoraShortcuts")
    static let macoraDiagnose = Notification.Name("MacoraDiagnose")
}
