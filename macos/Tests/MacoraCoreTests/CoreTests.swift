import XCTest
@testable import MacoraCore

final class CoreTests: XCTestCase {
    private var services: [Service] { Service.parse("An asterisk denotes disabled services.\nWi-Fi\n*Thunderbolt Bridge\nOffice's Ethernet\n") }
    private func make(_ action: SettingAction, _ values: [String] = [], service: String = "Wi-Fi") throws -> CommandPlan {
        try CommandPlan.make(action: action, service: service, values: values, services: services, interfaces: ["en0", "en1"])
    }
    func testDisabledServicesRetainTheirNames() {
        XCTAssertEqual(services.count, 3)
        XCTAssertFalse(services[1].enabled)
        XCTAssertEqual(services[1].name, "Thunderbolt Bridge")
    }
    func testDHCPAndStaticPlans() throws {
        XCTAssertEqual(try make(.dhcp).arguments, ["-setdhcp", "Wi-Fi"])
        XCTAssertEqual(try make(.staticIPv4, ["192.168.1.8", "255.255.255.0", "192.168.1.1"]).arguments,
                       ["-setmanual", "Wi-Fi", "192.168.1.8", "255.255.255.0", "192.168.1.1"])
        XCTAssertThrowsError(try make(.staticIPv4, ["bad", "255.255.255.0", "1.1.1.1"]))
        XCTAssertThrowsError(try make(.staticIPv4, ["1.1.1.2", "255.0.255.0", "1.1.1.1"]))
    }
    func testDNSValidationAndAutomaticReset() throws {
        XCTAssertEqual(try make(.dns, [""]).arguments, ["-setdnsservers", "Wi-Fi", "Empty"])
        XCTAssertEqual(try make(.dns, ["1.1.1.1, 2606:4700:4700::1111"]).arguments.count, 4)
        XCTAssertThrowsError(try make(.dns, ["1.1.1.1;id"]))
        XCTAssertThrowsError(try make(.dns, ["resolver.example"]))
    }
    func testProxyPlansAndPorts() throws {
        XCTAssertEqual(try make(.httpsProxy, ["proxy.example", "8443"]).arguments,
                       ["-setsecurewebproxy", "Wi-Fi", "proxy.example", "8443", "off"])
        XCTAssertThrowsError(try make(.httpProxy, ["proxy.example", "0"]))
        XCTAssertThrowsError(try make(.httpProxy, ["proxy.example", "65536"]))
        XCTAssertThrowsError(try make(.pacProxy, ["file:///tmp/evil"]))
        XCTAssertEqual(try make(.proxyOff, ["SOCKS"]).arguments, ["-setsocksfirewallproxystate", "Wi-Fi", "off"])
    }
    func testDeviceAndServiceValidation() throws {
        XCTAssertEqual(try make(.mtu, ["1400"], service: "en0").arguments, ["-setMTU", "en0", "1400"])
        XCTAssertThrowsError(try make(.mtu, ["9001"], service: "en0"))
        XCTAssertThrowsError(try make(.wifiOff, service: "en9"))
        XCTAssertThrowsError(try make(.dhcp, service: "Missing"))
        XCTAssertEqual(try make(.wifiOn, service: "en0").arguments, ["-setairportpower", "en0", "on"])
    }
    func testShellQuotingDoesNotBecomeAnAppleScriptOrShellProgram() throws {
        XCTAssertEqual(CommandPlan.quote("Office's Ethernet"), "'Office'\\''s Ethernet'")
        XCTAssertEqual(CommandPlan.quote("$(id); `whoami`"), "'$(id); `whoami`'")
        let plan = try make(.dhcp, service: "Office's Ethernet")
        XCTAssertTrue(plan.shellCommand.contains("'Office'\\''s Ethernet'"))
    }
    func testServiceOrderMustBeAnExactPermutation() throws {
        let order = services.map(\.name).reversed()
        XCTAssertEqual(try make(.serviceOrder, Array(order)).arguments.first, "-ordernetworkservices")
        XCTAssertThrowsError(try make(.serviceOrder, ["Wi-Fi", "Wi-Fi"]))
        XCTAssertThrowsError(try make(.serviceOrder, ["Wi-Fi"]))
    }
    func testIPv6AndDirectoryCachePlans() throws {
        XCTAssertEqual(try make(.ipv6Automatic).arguments, ["-setv6automatic", "Wi-Fi"])
        XCTAssertEqual(try make(.ipv6LinkLocal).arguments, ["-setv6linklocal", "Wi-Fi"])
        XCTAssertEqual(try make(.flushDNS).executable, "/usr/bin/dscacheutil")
    }
    func testStaticIPv6AndBypassPlans() throws {
        XCTAssertEqual(try make(.ipv6Manual, ["2001:db8::2", "64", "2001:db8::1"]).arguments, ["-setv6manual", "Wi-Fi", "2001:db8::2", "64", "2001:db8::1"])
        XCTAssertThrowsError(try make(.ipv6Manual, ["2001:db8::2", "129", "2001:db8::1"]))
        XCTAssertEqual(try make(.proxyBypass, ["*.example.test,10.0.0.0/8"]).arguments, ["-setproxybypassdomains", "Wi-Fi", "*.example.test", "10.0.0.0/8"])
        XCTAssertEqual(try make(.forgetWiFi, ["Office Wi-Fi"], service: "en0").arguments, ["-removepreferredwirelessnetwork", "en0", "Office Wi-Fi"])
    }
    func testLocationsRequireKnownTargets() throws {
        let plan = try CommandPlan.make(action: .locationSwitch, service: "", values: ["Office"], services: services, interfaces: [], locations: ["Automatic", "Office"])
        XCTAssertEqual(plan.arguments, ["-switchtolocation", "Office"])
        XCTAssertThrowsError(try CommandPlan.make(action: .locationDelete, service: "", values: ["Missing"], services: services, interfaces: [], locations: ["Automatic"]))
        XCTAssertEqual(try make(.locationCreate, ["New Office"]).arguments, ["-createlocation", "New Office", "populate"])
    }
    func testRateResetAndFormatting() throws {
        let decode = { (s: String) in try JSONDecoder().decode(TrafficCounters.self, from: Data(s.utf8)) }
        let a = try decode("{\"rx_bytes\":100,\"tx_bytes\":200}")
        let b = try decode("{\"rx_bytes\":300,\"tx_bytes\":50}")
        XCTAssertEqual(Traffic.rates(previous: a, current: b, elapsed: 2).0, 100)
        XCTAssertEqual(Traffic.rates(previous: a, current: b, elapsed: 2).1, 0)
        XCTAssertEqual(Traffic.rates(previous: a, current: b, elapsed: 0).0, 0)
        XCTAssertEqual(Traffic.format(2048), "2.0 KiB/s")
    }
    func testRustJSONContractAndVersionGuard() throws {
        let json = "{\"schema_version\":1,\"interfaces\":[{\"name\":\"en0\",\"label\":\"Wi-Fi\",\"group\":\"Network\",\"up\":true,\"active\":true,\"mtu\":1500,\"mac\":null,\"status\":\"active\",\"media\":null,\"flags\":[\"UP\"],\"options\":[],\"members\":[],\"addresses\":[{\"address\":\"fe80::1%en0\",\"prefix\":64,\"scope\":\"link-local\",\"attributes\":[]}],\"counters\":null}],\"warnings\":[]}"
        let snapshot = try NetworkSnapshot.decode(Data(json.utf8))
        XCTAssertEqual(snapshot.interfaces[0].addresses[0].display, "fe80::1%en0/64")
        XCTAssertEqual(snapshot.interfaces[0].symbol, "wifi")
        XCTAssertThrowsError(try NetworkSnapshot.decode(Data(json.replacingOccurrences(of: "schema_version\":1", with: "schema_version\":99").utf8)))
    }
}
