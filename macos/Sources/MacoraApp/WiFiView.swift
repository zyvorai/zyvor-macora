import SwiftUI
import AppKit
import CoreWLAN
import CoreLocation
import MacoraCore

private struct WiFiNetwork: Identifiable {
    let network: CWNetwork
    var id: String { "\(network.ssid ?? "Hidden")-\(network.bssid ?? String(network.hash))" }
    var name: String { network.ssid ?? "Hidden / privacy-redacted network" }
    var enterprise: Bool { network.supportsSecurity(.wpa2Enterprise) || network.supportsSecurity(.wpaEnterprise) || network.supportsSecurity(.wpa3Enterprise) }
    var open: Bool { network.supportsSecurity(.none) }
    var signal: String { "\(network.rssiValue) dBm" }
}
@MainActor private final class WiFiController: NSObject, ObservableObject, CLLocationManagerDelegate {
    @Published var networks: [WiFiNetwork] = []
    @Published var busy = false
    @Published var error: String?
    @Published var status = "Scan nearby networks to get started."
    @Published var joined: WiFiNetwork?
    private let location = CLLocationManager()
    override init() { super.init(); location.delegate = self }
    func requestLocation() { location.requestWhenInUseAuthorization() }
    func locationManagerDidChangeAuthorization(_ manager: CLLocationManager) {
        switch manager.authorizationStatus {
        case .denied, .restricted: status = "Location permission is denied. macOS may hide Wi-Fi names; use System Settings to manage permission."
        default: break
        }
    }
    func scan() async {
        guard !busy else { return }; busy = true; error = nil
        defer { busy = false }
        do {
            let found = try await Task.detached(priority: .utility) {
                guard let interface = CWWiFiClient.shared().interface() else { throw MacoraError.message("No Wi-Fi adapter available") }
                return try interface.scanForNetworks(withName: nil).map { WiFiNetwork(network: $0) }.sorted { $0.network.rssiValue > $1.network.rssiValue }
            }.value
            networks = found
            status = "\(found.count) networks · macOS may redact SSIDs without Location permission."
        } catch { self.error = error.localizedDescription }
    }
    func connect(_ selected: WiFiNetwork, password: String) async {
        guard !busy else { return }; busy = true; error = nil; defer { busy = false }
        do {
            try await Task.detached(priority: .utility) {
                guard let interface = CWWiFiClient.shared().interface() else { throw MacoraError.message("No Wi-Fi adapter available") }
                try interface.associate(to: selected.network, password: selected.open ? nil : password)
            }.value
            joined = nil; status = "Associated with \(selected.name)."
        } catch { self.error = error.localizedDescription }
    }
    func disconnect() async {
        guard !busy else { return }; busy = true; defer { busy = false }
        do {
            try await Task.detached {
                guard let interface = CWWiFiClient.shared().interface() else { throw MacoraError.message("No Wi-Fi adapter available") }
                interface.disassociate()
            }.value
            status = "Disconnect requested. macOS auto-join policies may reconnect."
        } catch { self.error = error.localizedDescription }
    }
}
@MainActor struct WiFiView: View {
    @EnvironmentObject private var store: NetworkStore
    @StateObject private var wifi = WiFiController()
    @State private var password = ""
    @State private var disconnectPrompt = false
    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            HStack {
                VStack(alignment: .leading, spacing: 6) { Text("Nearby. Connected. Understood.").font(.title2.bold()); Text("Wi-Fi discovery and association through Apple CoreWLAN.").foregroundStyle(.secondary) }
                Spacer()
                Button("Allow Wi-Fi discovery") { wifi.requestLocation() }.disabled(store.demo)
                Button { Task { await wifi.scan() } } label: { Label(wifi.busy ? "Working…" : "Scan Networks", systemImage: "wifi") }.buttonStyle(.borderedProminent).disabled(wifi.busy || store.demo)
            }
            Text(wifi.status).font(.callout).foregroundStyle(.secondary)
            Text("A scan is user initiated. Passwords go directly to CoreWLAN; MACORA does not write them to files, logs or process arguments. macOS manages its own saved-network credentials.").font(.caption).foregroundStyle(.secondary)
            if let error = wifi.error { Label(error, systemImage: "exclamationmark.triangle").foregroundStyle(.orange) }
            ForEach(wifi.networks) { row in
                HStack(spacing: 14) {
                    Image(systemName: row.open ? "wifi" : "wifi.circle.fill").font(.title2).foregroundStyle(.blue)
                    VStack(alignment: .leading, spacing: 4) { Text(row.name).font(.headline); Text("\(row.signal) · channel \(row.network.wlanChannel?.channelNumber ?? 0) · \(row.enterprise ? "Enterprise" : row.open ? "Open" : "Secured")").font(.caption).foregroundStyle(.secondary) }
                    Spacer()
                    if row.enterprise { Button("Open Wi-Fi Settings") { openSettings() } }
                    else { Button("Join…") { password = ""; wifi.joined = row }.disabled(wifi.busy || store.demo || row.network.ssid == nil) }
                }.padding(18).card()
            }
            if wifi.networks.isEmpty { ContentUnavailableView("Explore nearby Wi-Fi", systemImage: "wifi", description: Text("Allow discovery if needed, then scan. Enterprise 802.1X connections use System Settings.")) }
            HStack { Button("Disconnect Wi-Fi…") { disconnectPrompt = true }.disabled(wifi.busy || store.demo); Spacer(); Button("Manage saved networks") { openSettings() } }
        }
        .sheet(item: $wifi.joined, onDismiss: { password = "" }) { row in
            VStack(alignment: .leading, spacing: 18) {
                Text("Join \(row.name)?").font(.title2.bold())
                Text("This switches this Mac’s Wi-Fi network and may interrupt existing connections.").foregroundStyle(.secondary)
                if !row.open { SecureField("Wi-Fi password", text: $password).textFieldStyle(.roundedBorder) }
                if let error = wifi.error { Text(error).foregroundStyle(.orange) }
                HStack { Button("Cancel") { password = ""; wifi.joined = nil }.disabled(wifi.busy); Spacer(); Button("Join Network") { let secret = password; password = ""; Task { await wifi.connect(row, password: secret); await store.refresh() } }.buttonStyle(.borderedProminent).disabled(wifi.busy) }
            }.padding(26).frame(width: 500).interactiveDismissDisabled(wifi.busy)
        }
        .confirmationDialog("Disconnect Wi-Fi? Active connections may be interrupted.", isPresented: $disconnectPrompt, titleVisibility: .visible) { Button("Disconnect", role: .destructive) { Task { await wifi.disconnect(); await store.refresh() } } }
    }
    private func openSettings() { if let url = URL(string: "x-apple.systempreferences:com.apple.wifi-settings-extension") { NSWorkspace.shared.open(url) } }
}
