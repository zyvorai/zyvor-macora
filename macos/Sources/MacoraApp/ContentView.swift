import SwiftUI
import Charts
import AppKit
import MacoraCore

private enum Page: String, CaseIterable, Identifiable {
    case overview = "Overview", interfaces = "Interfaces", wifi = "Wi-Fi", services = "Services"
    case routes = "Routes", dns = "DNS", neighbors = "Neighbors", sockets = "Connections"
    case doctor = "Diagnostics", activity = "Change History"
    case topology = "Topology", compare = "Compare", tools = "Network Tools"
    var id: String { rawValue }
    var icon: String {
        switch self {
        case .overview: return "gauge.with.dots.needle.50percent"
        case .interfaces: return "network"
        case .wifi: return "wifi"
        case .services: return "slider.horizontal.3"
        case .routes: return "point.topleft.down.to.point.bottomright.curvepath"
        case .dns: return "globe"
        case .neighbors: return "laptopcomputer.and.iphone"
        case .sockets: return "cable.connector"
        case .doctor: return "stethoscope"
        case .activity: return "clock.arrow.circlepath"
        case .topology: return "point.3.connected.trianglepath.dotted"
        case .compare: return "rectangle.on.rectangle"
        case .tools: return "wrench.and.screwdriver"
        }
    }
    var command: String? {
        switch self {
        case .routes: return "routes"; case .dns: return "dns"; case .neighbors: return "neighbors"; case .sockets: return "sockets"
        default: return nil
        }
    }
}
@MainActor struct ContentView: View {
    @EnvironmentObject private var store: NetworkStore
    @State private var page: Page? = {
        // `--page Diagnostics` opens a page directly; used for documentation screenshots.
        let args = ProcessInfo.processInfo.arguments
        if let i = args.firstIndex(of: "--page"), args.indices.contains(i + 1), let p = Page(rawValue: args[i + 1]) { return p }
        return .overview
    }()
    @State private var search = ""
    @State private var palette = false
    @State private var preferences = false
    @State private var pendingCommand: WorkspaceCommand?
    @AppStorage("macora.pinnedInterfaces") private var pinnedNames = ""
    @AppStorage("macora.density") private var density = "comfortable"
    @AppStorage("macora.maskAddresses") private var maskAddresses = false
    @State private var settings = false
    @State private var shortcuts = false
    @AppStorage("macora.onboarded") private var onboarded = false
    @State private var selectedInterface: NetworkInterface?
    @State private var rawOutput = ""
    @State private var loading = false
    @State private var inspectorRequest = UUID()
    @State private var showSystem = false
    @Environment(\.colorScheme) private var colorScheme
    private let columns = [GridItem(.adaptive(minimum: 285), spacing: 16)]
    private var filtered: [NetworkInterface] {
        (store.snapshot?.interfaces ?? []).filter { i in
            (showSystem || isPinned(i.name) || !["System", "Loopback", "Tunnels"].contains(i.group)) &&
            (search.isEmpty || "\(i.name) \(i.label) \(i.group) \(i.addresses.map(\.address).joined(separator: " "))".localizedCaseInsensitiveContains(search))
        }.sorted { a,b in
            if isPinned(a.name) != isPinned(b.name) { return isPinned(a.name) }
            return a.active == b.active ? a.name < b.name : a.active
        }
    }
    var body: some View {
        NavigationSplitView {
            VStack(alignment: .leading, spacing: 0) {
                HStack(spacing: 10) {
                    Image(systemName: "waveform.path").font(.title3.bold()).foregroundStyle(.white).frame(width: 36, height: 36).background(.blue.gradient, in: RoundedRectangle(cornerRadius: 11))
                    VStack(alignment: .leading, spacing: 2) { Text("MACORA").font(.headline); Text("ZYVOR · FOR MAC").font(.system(size: 9, weight: .semibold)).foregroundStyle(.secondary).tracking(1.3) }
                }.padding(20).padding(.top, 14)
                List(selection: $page) {
                    Section("Network") { ForEach([Page.overview, .interfaces, .wifi, .services]) { p in Label(p.rawValue, systemImage: p.icon).tag(p).padding(.vertical, 4) } }
                    Section("Explore") { ForEach([Page.topology, .routes, .dns, .neighbors, .sockets, .tools]) { p in Label(p.rawValue, systemImage: p.icon).tag(p).padding(.vertical, 4) } }
                    Section("Intelligence") { ForEach([Page.doctor, .compare, .activity]) { p in Label(p.rawValue, systemImage: p.icon).tag(p).padding(.vertical, 4) } }
                }.listStyle(.sidebar)
                VStack(alignment: .leading, spacing: 5) {
                    Label(Host.current().localizedName ?? "This Mac", systemImage: "desktopcomputer").font(.caption.bold())
                    Text(store.demo ? "Demo data · changes disabled" : store.paused ? "Monitoring paused" : "Local collection · no cloud").font(.caption2).foregroundStyle(.secondary)
                }.padding(14).frame(maxWidth: .infinity, alignment: .leading).background(Color.primary.opacity(0.04), in: RoundedRectangle(cornerRadius: 12)).padding(14)
            }.navigationSplitViewColumnWidth(220)
        } detail: {
            VStack(spacing: 0) {
                header
                ScrollView {
                    VStack(alignment: .leading, spacing: 22) {
                        if store.demo { banner("DEMO DATA · This preview does not show this Mac’s live network.", icon: "play.rectangle", color: .orange) }
                        if let error = store.error { banner("Refresh failed. Displayed data may be stale. \(error)", icon: "exclamationmark.triangle", color: .orange) }
                        if let error = store.actionError { HStack { banner(error, icon: "exclamationmark.triangle", color: .orange); Button("Dismiss") { store.actionError = nil } } }
                        switch page ?? .overview {
                        case .overview: overview
                        case .interfaces: interfacesView
                        case .wifi: WiFiView().environmentObject(store)
                        case .services: servicesView
                        case .doctor: doctorView
                        case .activity: activityView
                        case .topology: NetworkTopology()
                        case .compare: BaselineComparison()
                        case .tools: NetworkTools()
                        default: inspectorView
                        }
                    }.padding(26)
                }.background(MacoraDesign.page(colorScheme))
                HStack {
                    Circle().fill(store.error != nil ? .orange : store.paused ? .gray : .green).frame(width: 6, height: 6)
                    Text(store.demo ? "DEMO" : store.paused ? "PAUSED" : "LIVE").font(.system(size: 9, weight: .bold)).padding(.horizontal, 6).padding(.vertical, 2).background((store.demo ? Color.orange : store.paused ? Color.gray : Color.green).opacity(0.18), in: Capsule())
                    if let date = store.lastUpdated { Text("· Updated \(date.formatted(date: .omitted, time: .standard))") }
                    Spacer(); Text("macOS native · Apache 2.0" + (maskAddresses ? " · IP/MAC masked" : ""))
                }.font(.system(size: 10)).foregroundStyle(.secondary).padding(.horizontal, 26).padding(.vertical, 10)
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .macoraPalette)) { _ in palette = true }
        .onReceive(NotificationCenter.default.publisher(for: .macoraShortcuts)) { _ in shortcuts = true }
        .onReceive(NotificationCenter.default.publisher(for: .macoraDiagnose)) { _ in page = .doctor }
        .sheet(isPresented: $shortcuts) { ShortcutSheet() }
        .sheet(isPresented: $palette, onDismiss: {
            if let command = pendingCommand { pendingCommand = nil; performCommand(command) }
        }) { CommandPalette { command in pendingCommand = command; palette = false } }
        .sheet(isPresented: $preferences) { WorkspacePreferences().environmentObject(store) }
        .sheet(isPresented: $settings) { SettingsSheet().environmentObject(store) }
        .sheet(item: $selectedInterface) { interfaceDetail($0) }
        .task(id: page) {
            inspectorRequest = UUID(); rawOutput = ""; loading = false
            if let command = page?.command { await inspect(command) }
            if page == .doctor { await store.diagnose() }
            if page == .doctor, store.demo, store.diagnosis == nil { await store.runDiagnosis() }
        }
    }
    private var header: some View {
        HStack(spacing: 16) {
            VStack(alignment: .leading, spacing: 3) {
                Text(page?.rawValue ?? "Overview").font(.system(size: 21, weight: .bold))
                Text("Your Mac’s network, made clear.").font(.caption).foregroundStyle(.secondary)
            }
            Spacer()
            Button { palette = true } label: { HStack { Image(systemName: "magnifyingglass"); Text("Search actions"); Text("⌘K").font(.caption.monospaced()) } }.buttonStyle(.bordered).help("Command palette")
            Button { preferences = true } label: { Image(systemName: "circle.lefthalf.filled") }.help("Workspace preferences")
            Button { store.paused.toggle() } label: { Image(systemName: store.paused ? "play.fill" : "pause.fill") }.help("Pause or resume collection")
            Button { Task { await store.refresh() } } label: { Image(systemName: "arrow.clockwise") }.help("Refresh")
            Menu { Button("Export redacted snapshot…") { Task { await store.export(redact: true) } }; Button("Export full snapshot…") { Task { await store.export(redact: false) } } } label: { Image(systemName: "square.and.arrow.up") }.menuStyle(.button).menuIndicator(.hidden).fixedSize().help("Export")
            Button { settings = true } label: { Label("Configure", systemImage: "slider.horizontal.3") }.buttonStyle(.borderedProminent).disabled(store.demo)
        }.padding(.horizontal, 26).padding(.vertical, 18).background(.bar)
    }
    private var overview: some View {
        VStack(alignment: .leading, spacing: 26) {
            VStack(alignment: .leading, spacing: 12) {
                Text("MACORA · NETWORK WORKSPACE").font(.caption.weight(.bold)).tracking(2).foregroundStyle(MacoraDesign.blue)
                Text("Your network. In full view.").font(.system(size: 38, weight: .bold, design: .rounded))
                Text("Understand the connection. Explore the evidence. Make the next change with confidence.").font(.title3).foregroundStyle(.secondary).frame(maxWidth: 760, alignment: .leading)
                HStack(spacing: 18) {
                    Button { page = .interfaces } label: { Text("Explore Your Network").padding(.horizontal, 8).padding(.vertical, 5) }.buttonStyle(.borderedProminent).buttonBorderShape(.capsule)
                    Button("Run diagnostics ›") { page = .doctor }.buttonStyle(.plain).foregroundStyle(MacoraDesign.blue)
                    Spacer()
                    Label(store.demo ? "Demo data" : "On this Mac", systemImage: store.demo ? "play.rectangle" : "desktopcomputer").font(.caption).foregroundStyle(.secondary)
                }.padding(.top, 8)
            }.padding(.vertical, 8)
            if !onboarded {
                OnboardingCard(dismissed: $onboarded) { target in
                    switch target { case "palette": palette = true; case "shortcuts": shortcuts = true; default: page = .doctor }
                }
            }
            VStack(alignment: .leading, spacing: 22) {
                HStack { Text("Your connection stack").font(.title2.bold()); Spacer(); Text(store.paused ? "Monitoring paused" : "Local collection").font(.caption).foregroundStyle(.secondary) }
                LazyVGrid(columns: [GridItem(.adaptive(minimum: 185), spacing: 14)], spacing: 14) {
                    stageLink("Adapters", "Every interface, clearly grouped.", "network", .interfaces)
                    stageLink("Wireless", "Nearby networks and signal.", "wifi", .wifi)
                    stageLink("Control", "Services, DNS and locations.", "slider.horizontal.3", .services)
                    stageLink("Evidence", "Baseline before. Compare after.", "rectangle.on.rectangle", .compare)
                }
                Divider()
                HStack(alignment: .top, spacing: 24) {
                    bandMetric("Active adapters", String(store.networkInterfaces.filter(\.active).count))
                    bandMetric("Download", store.latest.map { Traffic.format($0.rx) } ?? "Sampling…")
                    bandMetric("Upload", store.latest.map { Traffic.format($0.tx) } ?? "Sampling…")
                    bandMetric("Baseline changes", String(store.comparison.count))
                }
            }.padding(density == "compact" ? 20 : 26).card()
            HStack(alignment: .top, spacing: 20) {
                VStack(alignment: .leading, spacing: 18) {
                    Text("Your connections").font(.title3.bold())
                    if store.networkInterfaces.isEmpty { Text("Waiting for an adapter capture.").foregroundStyle(.secondary) }
                    ForEach(filtered.filter { $0.group == "Network" }.prefix(3)) { interfaceCard($0) }
                }.frame(maxWidth: .infinity, alignment: .leading)
                VStack(alignment: .leading, spacing: 16) {
                    Text("Workspace evidence").font(.title3.bold())
                    evidenceRow("Interface inventory", store.snapshot == nil ? "Waiting" : "Captured")
                    evidenceRow("Bridge memberships", String(store.snapshot?.interfaces.filter { !$0.members.isEmpty }.count ?? 0))
                    evidenceRow("VPN ownership", "Not inferred")
                    evidenceRow("Internet reachability", "Not tested")
                    Text("A link-active interface is not proof of internet access. Use a targeted tool when you need endpoint evidence.").font(.caption).foregroundStyle(.secondary).padding(.top, 6)
                    Button("Open Network Tools ›") { page = .tools }.buttonStyle(.plain).foregroundStyle(MacoraDesign.blue)
                }.padding(22).frame(maxWidth: 350, alignment: .leading).card()
            }
            trafficChart
            if store.snapshot == nil { ContentUnavailableView("Reading this Mac’s network", systemImage: "network", description: Text("The bundled Rust collector is loading interface details.")) }
        }
    }
    private func stageLink(_ title: String, _ detail: String, _ symbol: String, _ destination: Page) -> some View {
        Button { page = destination } label: {
            VStack(alignment: .leading, spacing: 10) {
                Image(systemName: symbol).font(.title2).foregroundStyle(MacoraDesign.blue)
                Text(title).font(.headline)
                Text(detail).font(.caption).foregroundStyle(.secondary).frame(minHeight: 32, alignment: .topLeading)
                Text("Open ›").font(.caption).foregroundStyle(MacoraDesign.blue)
            }.padding(density == "compact" ? 12 : 18).frame(maxWidth: .infinity, alignment: .leading).card()
        }.buttonStyle(.plain)
    }
    private func bandMetric(_ label: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: 7) { Text(value).font(.system(size: 23, weight: .bold, design: .rounded)).lineLimit(1).minimumScaleFactor(0.6); Text(label).font(.caption).foregroundStyle(.secondary) }.frame(maxWidth: .infinity, alignment: .leading)
    }
    private func evidenceRow(_ title: String, _ value: String) -> some View { VStack(spacing: 12) { HStack { Text(title).foregroundStyle(.secondary); Spacer(); Text(value).fontWeight(.medium) }.font(.caption); Divider() } }
    private var trafficChart: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack { Text("Traffic, in real time").font(.headline); Spacer(); Label("Download", systemImage: "circle.fill").foregroundStyle(MacoraDesign.blue); Label("Upload", systemImage: "circle.fill").foregroundStyle(.secondary) }.font(.caption)
            if store.history.isEmpty {
                Text("Waiting for two successful counter samples. Unavailable counters are never estimated.").font(.caption).foregroundStyle(.secondary).frame(maxWidth: .infinity, minHeight: 140)
            } else {
                Chart(store.history) { point in
                    AreaMark(x: .value("Time", point.date), y: .value("RX", point.rx)).foregroundStyle(MacoraDesign.blue.opacity(0.10)).interpolationMethod(.monotone)
                    LineMark(x: .value("Time", point.date), y: .value("Bytes/s", point.rx), series: .value("Direction", "Download")).foregroundStyle(MacoraDesign.blue).interpolationMethod(.monotone)
                    LineMark(x: .value("Time", point.date), y: .value("Bytes/s", point.tx), series: .value("Direction", "Upload")).foregroundStyle(Color.secondary).interpolationMethod(.monotone)
                }.chartYAxis { AxisMarks(position: .leading) { value in AxisGridLine(); AxisValueLabel { if let n = value.as(Double.self) { Text(Traffic.format(n)) } } } }.chartXAxis(.hidden).frame(height: 150)
            }
            Text("Adapter totals · bridges and tunnels excluded to reduce double counting · up to 90 samples in memory").font(.system(size: 10)).foregroundStyle(.secondary)
        }.padding(20).card()
    }
    private var interfacesView: some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack { Text("Interface explorer").font(.title2.bold()); Spacer(); Toggle("Include system & tunnels", isOn: $showSystem).toggleStyle(.switch).controlSize(.small) }
            TextField("Search names, roles or addresses", text: $search).textFieldStyle(.roundedBorder)
            ForEach(["Network", "Bridges", "Virtual", "Tunnels", "System", "Loopback"], id: \.self) { group in
                let items = filtered.filter { $0.group == group }
                if !items.isEmpty {
                    Text("\(group.uppercased()) · \(items.count)").font(.caption.weight(.semibold)).foregroundStyle(.secondary)
                    LazyVGrid(columns: columns, spacing: 16) { ForEach(items) { interfaceCard($0) } }
                }
            }
            if filtered.isEmpty { ContentUnavailableView.search(text: search) }
        }
    }
    private var servicesView: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Network services").font(.title2.bold())
            Text("Manage DHCP, static IPv4/IPv6, DNS, search domains, proxies, Wi-Fi power, MTU, locations and service order. All changes require a preview and macOS authorization.").foregroundStyle(.secondary)
            ForEach(Array(store.services.enumerated()), id: \.element.id) { index, service in
                HStack { Text(String(index + 1)).font(.headline.monospacedDigit()).foregroundStyle(.secondary).frame(width: 28); Label(service.name, systemImage: "network"); Spacer(); Text(service.enabled ? "Enabled" : "Disabled").foregroundStyle(service.enabled ? .green : .secondary) }.padding(18).card()
            }
            Button("Configure a service…") { settings = true }.buttonStyle(.borderedProminent).disabled(store.demo)
            Divider().padding(.vertical, 8)
            Text("Managed by macOS").font(.headline)
            Text("VPN profiles, 802.1X enterprise authentication, Internet Sharing, firewall rules and network extensions are managed through Apple’s system interfaces. MACORA opens the appropriate settings; it does not install a VPN or packet filter.").font(.callout).foregroundStyle(.secondary)
            HStack { systemButton("VPN", "com.apple.VPN-Settings.extension"); systemButton("Network", "com.apple.Network-Settings.extension"); systemButton("Firewall", "com.apple.Network-Settings.extension?Firewall") }
        }
    }
    private var doctorView: some View {
        VStack(alignment: .leading, spacing: 28) {
            DiagnosisWizard()
            Divider()
            localFindings
        }
    }
    private var localFindings: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack { Text("Understand before you troubleshoot.").font(.title2.bold()); Spacer(); Button("Run again") { Task { await store.diagnose() } } }
            Text("Local checks explain self-assigned addresses, inactive bridges, IPv6 rotation and low tunnel MTUs. No external endpoint is contacted.").foregroundStyle(.secondary)
            ForEach(Array(store.findings.enumerated()), id: \.offset) { _, finding in banner(finding, icon: finding.hasPrefix("WARN") ? "exclamationmark.triangle" : "info.circle", color: finding.hasPrefix("WARN") ? .orange : .blue) }
            if store.findings.isEmpty { ProgressView().frame(maxWidth: .infinity) }
            Button("Inspect default route & reachability") { Task { await inspect("path") } }
            if !rawOutput.isEmpty { outputBlock(rawOutput) }
        }
    }
    private var activityView: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Your network timeline").font(.largeTitle.bold())
            Text("Observed configuration changes").font(.headline)
            if store.timeline.isEmpty { Text("No interface changes observed in this session.").foregroundStyle(.secondary) }
            ForEach(store.timeline) { event in
                HStack(alignment: .top, spacing: 14) {
                    Image(systemName: "arrow.triangle.branch").foregroundStyle(MacoraDesign.blue)
                    VStack(alignment: .leading, spacing: 5) { Text(store.present(event.change)).font(.callout).textSelection(.enabled); Text(event.date.formatted(date: .omitted, time: .standard)).font(.caption).foregroundStyle(.secondary) }
                    Spacer()
                }.padding(16).card()
            }
            Text("Actions you authorized").font(.headline)
            Text("Successful configuration actions from this app session. History stays in memory and is cleared when the app exits.").foregroundStyle(.secondary)
            if store.events.isEmpty { ContentUnavailableView("No changes yet", systemImage: "checkmark.shield", description: Text("Monitoring does not modify your network.")) }
            ForEach(Array(store.events.enumerated()), id: \.offset) { _, event in Text(event).font(.callout).padding(16).frame(maxWidth: .infinity, alignment: .leading).card() }
        }
    }
    private var inspectorView: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack { Text(page?.rawValue ?? "Inspect").font(.title2.bold()); Spacer(); Button("Refresh") { if let c = page?.command { Task { await inspect(c) } } }; Button("Copy") { NSPasteboard.general.clearContents(); NSPasteboard.general.setString(rawOutput, forType: .string) } }
            if page == .dns { Text("All resolver scopes are retained. VPN and interface-specific DNS settings can differ from the default resolver.").foregroundStyle(.secondary) }
            if page == .sockets { Text("Only connections visible to your user are shown. No privilege escalation is used for inspection.").foregroundStyle(.secondary) }
            if loading { ProgressView() } else { outputBlock(rawOutput) }
        }
    }
    private func inspect(_ command: String) async {
        let request = UUID(); inspectorRequest = request; loading = true; rawOutput = ""
        do {
            let output = try await store.inspect(command)
            if inspectorRequest == request { rawOutput = output; loading = false }
        } catch {
            if inspectorRequest == request { rawOutput = error.localizedDescription; loading = false }
        }
    }
    private func metric(_ name: String, value: String, detail: String, icon: String, color: Color) -> some View {
        VStack(alignment: .leading, spacing: 16) {
            Label(name, systemImage: icon).font(.caption.weight(.semibold)).foregroundStyle(color)
            Text(value).font(.system(size: 29, weight: .bold, design: .rounded)).lineLimit(1).minimumScaleFactor(0.6)
            Text(detail).font(.caption).foregroundStyle(.secondary)
        }.frame(maxWidth: .infinity, alignment: .leading).padding(20).card()
    }
    private func interfaceCard(_ i: NetworkInterface) -> some View {
        Button { selectedInterface = i } label: {
            VStack(alignment: .leading, spacing: 12) {
                HStack {
                    Image(systemName: i.symbol).font(.title3).foregroundStyle(.blue).frame(width: 36, height: 36).background(.blue.opacity(0.12), in: RoundedRectangle(cornerRadius: 10))
                    VStack(alignment: .leading, spacing: 3) { Text(i.label).font(.headline); Text(i.name + (isPinned(i.name) ? " · Pinned" : "")).font(.caption.monospaced()).foregroundStyle(.secondary) }
                    Spacer(); Circle().fill(i.active ? .green : .gray).frame(width: 7, height: 7)
                }
                Text(store.present(i.addresses.first(where: { !$0.address.contains(":") })?.display ?? i.addresses.first?.display ?? "No address assigned")).font(.system(size: 13, weight: .medium, design: .monospaced)).lineLimit(1).textSelection(.enabled)
                if store.rateHistory.values(i.name).count >= 2 {
                    Sparkline(values: store.rateHistory.normalized(i.name)).frame(height: 26)
                }
                HStack { Text(i.active ? "Link active" : i.up ? "Enabled · idle" : "Down"); Spacer(); Text("MTU \(i.mtu.map(String.init) ?? "—")") }.font(.caption).foregroundStyle(.secondary)
            }.padding(density == "compact" ? 12 : 18).frame(maxWidth: .infinity, alignment: .leading).card()
        }.buttonStyle(.plain)
    }
    private func interfaceDetail(_ i: NetworkInterface) -> some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack { Label("\(i.label) · \(i.name)", systemImage: i.symbol).font(.title2.bold()); Spacer(); Button("Done") { selectedInterface = nil } }
            Text("\(i.group) · \(i.active ? "Link active" : "Idle/down") · MTU \(i.mtu.map(String.init) ?? "unknown")").foregroundStyle(.secondary)
            Button(isPinned(i.name) ? "Unpin interface" : "Pin interface") { togglePin(i.name) }
            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    ForEach(i.addresses) { a in VStack(alignment: .leading, spacing: 5) { Text(store.present(a.display)).font(.body.monospaced()).textSelection(.enabled); Text("\(a.scope) · \(a.attributes.joined(separator: " · "))").font(.caption).foregroundStyle(.secondary) }.padding(12).frame(maxWidth: .infinity, alignment: .leading).card() }
                    if let mac = i.mac { Text(store.present("MAC \(mac)")).font(.callout.monospaced()).textSelection(.enabled) }
                    if let media = i.media { Text("Media: \(media)").font(.caption) }
                    Text("Flags: \(i.flags.joined(separator: " · "))").font(.caption)
                    Text("Options: \(i.options.joined(separator: " · "))").font(.caption)
                    if !i.members.isEmpty { Text("Bridge members: \(i.members.joined(separator: ", "))") }
                    Text("Link-active is a driver state, not proof of internet access. Temporary IPv6 addresses support privacy. Deprecated addresses are normally avoided for new connections. Tunnel ownership is not inferred from its interface name.").font(.callout).foregroundStyle(.secondary)
                }
            }
        }.padding(26).frame(width: 650, height: 600)
    }
    private func isPinned(_ name: String) -> Bool { pinnedNames.split(separator: ",").map(String.init).contains(name) }
    private func togglePin(_ name: String) {
        var names = Set(pinnedNames.split(separator: ",").map(String.init))
        if names.contains(name) { names.remove(name) } else { names.insert(name) }
        pinnedNames = names.sorted().joined(separator: ",")
    }
    private func performCommand(_ command: WorkspaceCommand) {
        if let destination = Page(rawValue: command.id) { page = destination; return }
        switch command.id {
        case "configure": if !store.demo { settings = true }
        case "export": Task { await store.export(redact: true) }
        case "baseline": store.captureBaseline(); page = .compare
        case "preferences": preferences = true
        case "diagnose": page = .doctor; Task { await store.runDiagnosis() }
        case "shortcuts": shortcuts = true
        default: break
        }
    }

    private func outputBlock(_ text: String) -> some View { ScrollView(.horizontal) { Text(text.isEmpty ? "No output available." : text).font(.system(size: 12, design: .monospaced)).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading).padding(20) }.card() }
    private func banner(_ text: String, icon: String, color: Color) -> some View { Label(text, systemImage: icon).font(.callout).foregroundStyle(color).padding(16).frame(maxWidth: .infinity, alignment: .leading).background(color.opacity(0.10), in: RoundedRectangle(cornerRadius: 14)) }
    private func systemButton(_ title: String, _ anchor: String) -> some View { Button(title) { if let url = URL(string: "x-apple.systempreferences:\(anchor)") { NSWorkspace.shared.open(url) } } }
}
@MainActor private struct CardStyle: ViewModifier {
    @Environment(\.colorScheme) private var scheme
    func body(content: Content) -> some View {
        content.background(MacoraDesign.surface(scheme), in: RoundedRectangle(cornerRadius: 19))
            .overlay(RoundedRectangle(cornerRadius: 19).strokeBorder(Color.primary.opacity(scheme == .dark ? 0.12 : 0.10), lineWidth: 0.7))
    }
}
extension View { @MainActor func card() -> some View { modifier(CardStyle()) } }
