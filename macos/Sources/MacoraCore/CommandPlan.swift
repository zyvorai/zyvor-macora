import Foundation
import Darwin

public struct CommandPlan: Identifiable, Equatable {
    public let id = UUID()
    public let title: String
    public let executable: String
    public let arguments: [String]
    public let consequence: String
    public var shellCommand: String { ([executable] + arguments).map(Self.quote).joined(separator: " ") }
    public static func quote(_ s: String) -> String { "'" + s.replacingOccurrences(of: "'", with: "'\\''") + "'" }
    private static func validText(_ s: String) -> Bool {
        !s.isEmpty && !s.hasPrefix("-") && !s.unicodeScalars.contains { CharacterSet.controlCharacters.contains($0) }
    }
    public static func isIP(_ s: String, family: Int32? = nil) -> Bool {
        var v4 = in_addr(); var v6 = in6_addr()
        let a = s.withCString { inet_pton(AF_INET, $0, &v4) == 1 }
        let b = s.withCString { inet_pton(AF_INET6, $0, &v6) == 1 }
        if family == AF_INET { return a }; if family == AF_INET6 { return b }; return a || b
    }
    public static func make(action: SettingAction, service: String, values: [String], services: [Service], interfaces: [String], locations: [String] = []) throws -> CommandPlan {
        let serviceActions: Set<SettingAction> = [.dhcp, .staticIPv4, .dns, .searchDomains, .httpProxy, .httpsProxy, .socksProxy, .pacProxy, .proxyOff, .serviceOn, .serviceOff, .ipv6Automatic, .ipv6LinkLocal, .ipv6Manual, .proxyBypass]
        if serviceActions.contains(action) {
            guard validText(service), services.contains(where: { $0.name == service }) else { throw MacoraError.message("Select an existing network service") }
        }
        let val = { (n: Int) -> String in values.indices.contains(n) ? values[n].trimmingCharacters(in: .whitespacesAndNewlines) : "" }
        var args: [String] = []
        var executable = "/usr/sbin/networksetup"
        switch action {
        case .dhcp: args = ["-setdhcp", service]
        case .staticIPv4:
            let ip = val(0), mask = val(1), gateway = val(2)
            guard [ip, mask, gateway].allSatisfy({ isIP($0, family: AF_INET) }), contiguousMask(mask) else { throw MacoraError.message("Enter valid IPv4 address, contiguous netmask and router") }
            args = ["-setmanual", service, ip, mask, gateway]
        case .dns:
            let servers = val(0).split(whereSeparator: { $0 == "," || $0.isWhitespace }).map(String.init)
            guard servers.allSatisfy({ isIP($0) }) else { throw MacoraError.message("DNS servers must be IPv4 or IPv6 addresses") }
            args = ["-setdnsservers", service] + (servers.isEmpty ? ["Empty"] : servers)
        case .searchDomains:
            let domains = val(0).split(whereSeparator: { $0 == "," || $0.isWhitespace }).map(String.init)
            guard domains.allSatisfy({ validText($0) && $0.allSatisfy { $0.isASCII && ($0.isLetter || $0.isNumber || $0 == "." || $0 == "-") } }) else { throw MacoraError.message("Enter DNS search domains separated by commas") }
            args = ["-setsearchdomains", service] + (domains.isEmpty ? ["Empty"] : domains)
        case .httpProxy, .httpsProxy, .socksProxy:
            let host = val(0), port = val(1)
            guard validText(host), !host.contains(where: { $0.isWhitespace }), let p = Int(port), (1...65535).contains(p) else { throw MacoraError.message("Enter a proxy hostname and port 1–65535") }
            let flag = action == .httpProxy ? "-setwebproxy" : action == .httpsProxy ? "-setsecurewebproxy" : "-setsocksfirewallproxy"
            args = [flag, service, host, String(p), "off"]
        case .pacProxy:
            guard let url = URL(string: val(0)), ["https", "http"].contains(url.scheme ?? ""), url.host != nil else { throw MacoraError.message("Enter an HTTP(S) PAC URL") }
            args = ["-setautoproxyurl", service, val(0)]
        case .proxyOff:
            let options = ["HTTP": "-setwebproxystate", "HTTPS": "-setsecurewebproxystate", "SOCKS": "-setsocksfirewallproxystate", "PAC": "-setautoproxystate"]
            guard let flag = options[val(0)] else { throw MacoraError.message("Choose HTTP, HTTPS, SOCKS or PAC") }
            args = [flag, service, "off"]
        case .wifiOn, .wifiOff:
            guard validText(service), interfaces.contains(service) else { throw MacoraError.message("Select a Wi-Fi interface") }
            args = ["-setairportpower", service, action == .wifiOn ? "on" : "off"]
        case .mtu:
            guard validText(service), interfaces.contains(service), let mtu = Int(val(0)), (576...9000).contains(mtu) else { throw MacoraError.message("Select an interface and MTU 576–9000") }
            args = ["-setMTU", service, String(mtu)]
        case .serviceOn, .serviceOff: args = ["-setnetworkserviceenabled", service, action == .serviceOn ? "on" : "off"]
        case .ipv6Manual:
            guard isIP(val(0), family: AF_INET6), let prefix = Int(val(1)), (0...128).contains(prefix), isIP(val(2), family: AF_INET6) else { throw MacoraError.message("Enter IPv6 address, prefix 0–128 and IPv6 router") }
            args = ["-setv6manual", service, val(0), String(prefix), val(2)]
        case .proxyBypass:
            let domains = val(0).split(whereSeparator: { $0 == "," || $0.isWhitespace }).map(String.init)
            guard domains.allSatisfy({ validText($0) }) else { throw MacoraError.message("Invalid proxy bypass entry") }
            args = ["-setproxybypassdomains", service] + (domains.isEmpty ? ["Empty"] : domains)
        case .forgetWiFi:
            guard validText(service), interfaces.contains(service), validText(val(0)) else { throw MacoraError.message("Select a Wi-Fi interface and saved network name") }
            args = ["-removepreferredwirelessnetwork", service, val(0)]
        case .locationCreate:
            guard validText(val(0)), !locations.contains(val(0)) else { throw MacoraError.message("Enter a new location name") }
            args = ["-createlocation", val(0), "populate"]
        case .locationSwitch, .locationDelete:
            guard validText(val(0)), locations.contains(val(0)) else { throw MacoraError.message("Select an existing network location") }
            args = [action == .locationSwitch ? "-switchtolocation" : "-deletelocation", val(0)]
        case .ipv6Automatic: args = ["-setv6automatic", service]
        case .ipv6LinkLocal: args = ["-setv6linklocal", service]
        case .serviceOrder:
            let order = values.filter { !$0.isEmpty }
            guard !order.isEmpty, order.allSatisfy(validText), order.count == services.count, Set(order).count == order.count, Set(order) == Set(services.map(\.name)) else { throw MacoraError.message("Service order must include every service exactly once") }
            args = ["-ordernetworkservices"] + order
        case .flushDNS:
            executable = "/usr/bin/dscacheutil"; args = ["-flushcache"]
        }
        return CommandPlan(title: action.rawValue, executable: executable, arguments: args,
                           consequence: "This changes this Mac’s network settings and may interrupt connectivity. macOS will request administrator authorization. Applied changes are not automatically rolled back.")
    }
    public static func contiguousMask(_ s: String) -> Bool {
        let parts = s.split(separator: ".").compactMap { UInt8($0) }
        guard parts.count == 4 else { return false }
        let mask = parts.reduce(UInt32(0)) { ($0 << 8) | UInt32($1) }
        let inverse = ~mask
        return inverse & (inverse &+ 1) == 0
    }
}
public enum SettingAction: String, CaseIterable, Hashable, Identifiable {
    case dhcp = "Use DHCP", staticIPv4 = "Set static IPv4", dns = "Set DNS servers"
    case searchDomains = "Set search domains", httpProxy = "Set HTTP proxy", httpsProxy = "Set HTTPS proxy"
    case socksProxy = "Set SOCKS proxy", pacProxy = "Set PAC URL", proxyOff = "Disable a proxy"
    case wifiOn = "Turn Wi-Fi on", wifiOff = "Turn Wi-Fi off", mtu = "Set MTU"
    case serviceOn = "Enable service", serviceOff = "Disable service"
    case ipv6Automatic = "IPv6 automatic", ipv6LinkLocal = "IPv6 link-local only"
    case ipv6Manual = "Set static IPv6", proxyBypass = "Set proxy bypass domains"
    case forgetWiFi = "Forget saved Wi-Fi", locationCreate = "Create network location"
    case locationSwitch = "Switch network location", locationDelete = "Delete network location"
    case serviceOrder = "Reorder services", flushDNS = "Flush directory DNS cache"
    public var id: String { rawValue }
    public var fields: [String] {
        switch self {
        case .staticIPv4: return ["IPv4 address", "Subnet mask", "Router"]
        case .dns: return ["DNS servers, comma separated; blank restores automatic"]
        case .searchDomains: return ["Search domains; blank clears custom domains"]
        case .httpProxy, .httpsProxy, .socksProxy: return ["Proxy hostname", "Port"]
        case .pacProxy: return ["PAC URL"]
        case .proxyOff: return ["Proxy type: HTTP, HTTPS, SOCKS or PAC"]
        case .mtu: return ["MTU"]
        case .ipv6Manual: return ["IPv6 address", "Prefix length", "IPv6 router"]
        case .proxyBypass: return ["Bypass domains / CIDRs, comma separated; blank clears"]
        case .forgetWiFi: return ["Saved Wi-Fi network name (SSID)"]
        case .locationCreate: return ["New location name"]
        case .locationSwitch, .locationDelete: return ["Existing location name"]
        default: return []
        }
    }
    public var usesLocation: Bool { [.locationCreate, .locationSwitch, .locationDelete].contains(self) }
    public var usesInterface: Bool { [.wifiOn, .wifiOff, .mtu, .forgetWiFi].contains(self) }
}
