import SwiftUI
import MacoraCore
import AppKit

@MainActor struct NetworkTools: View {
    @EnvironmentObject private var store: NetworkStore
    @State private var kind = "Subnet"
    @State private var target = "192.168.1.8/24"
    @State private var output = ""
    @State private var busy = false
    private let kinds = ["Subnet", "TCP probe", "DNS lookup", "Evidence report"]
    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            Text("Know where to look next.").font(.largeTitle.bold())
            Text("Small, explicit tools for a focused investigation. TCP probes and DNS lookups contact the target or your configured resolver only when you press Run.").foregroundStyle(.secondary)
            VStack(alignment: .leading, spacing: 18) {
                Picker("Tool", selection: $kind) { ForEach(kinds, id: \.self) { Text($0).tag($0) } }.pickerStyle(.segmented)
                if kind != "Evidence report" { TextField(kind == "TCP probe" ? "Numeric IP:port, e.g. 1.1.1.1:443" : kind == "DNS lookup" ? "Hostname, e.g. example.com" : "IPv4/prefix", text: $target).textFieldStyle(.roundedBorder) }
                HStack {
                    Text(kind == "Evidence report" ? "Interface evidence only. No internet or security score is inferred." : "No background probing. No scan of your subnet.").font(.caption).foregroundStyle(.secondary)
                    Spacer(); Button(busy ? "Running…" : "Run Tool") { Task { await run() } }.buttonStyle(.borderedProminent).disabled(busy || (store.demo && kind != "Subnet"))
                }
            }.padding(22).card()
            if !output.isEmpty {
                HStack { Text("Result").font(.headline); Spacer(); Button("Copy") { NSPasteboard.general.clearContents(); NSPasteboard.general.setString(output, forType: .string) } }
                Text(output).font(.system(size: 12, design: .monospaced)).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading).padding(22).card()
            }
            VStack(alignment: .leading, spacing: 12) {
                Text("A practical investigation").font(.headline)
                Label("Inspect an adapter and its assigned addresses.", systemImage: "1.circle")
                Label("Check local diagnostics, then the default route and scoped DNS.", systemImage: "2.circle")
                Label("Run a targeted TCP probe when you need endpoint evidence.", systemImage: "3.circle")
                Label("Capture a baseline before changing a service. Compare afterward.", systemImage: "4.circle")
            }.font(.callout).foregroundStyle(.secondary).padding(22).card()
        }.onChange(of: kind) { _, value in output = ""; target = value == "TCP probe" ? "1.1.1.1:443" : value == "DNS lookup" ? "example.com" : "192.168.1.8/24" }
    }
    private func run() async {
        busy = true; defer { busy = false }
        let args: [String]
        switch kind { case "TCP probe": args = ["probe", target, "--timeout-ms", "2000"]; case "DNS lookup": args = ["resolve", target]; case "Evidence report": args = ["report", "--redact"]; default: args = ["subnet", target] }
        do { output = try await ProcessRunner.run(store.cli, args) }
        catch { output = error.localizedDescription }
    }
}
