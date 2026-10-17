import XCTest
@testable import MacoraCore

final class WorkspaceTests: XCTestCase {
    private func sample(name: String = "en0", mtu: Int = 1500, rx: Int = 100, active: Bool = true, address: String = "192.168.1.8") throws -> NetworkSnapshot {
        let object: [String: Any] = ["schema_version": 1, "interfaces": [["name": name, "label": "Wi-Fi", "group": "Network", "up": true, "active": active, "mtu": mtu, "mac": "02:00:00:00:00:08", "media": "autoselect", "status": active ? "active" : "inactive", "flags": ["UP", "RUNNING"], "options": [], "members": [], "addresses": [["address": address, "prefix": 24, "scope": "private", "attributes": []]], "counters": ["rx_bytes": rx, "tx_bytes": 50]]], "warnings": []]
        return try NetworkSnapshot.decode(JSONSerialization.data(withJSONObject: object))
    }
    func testPaletteFuzzyMatchingAndNoResults() {
        XCTAssertFalse(WorkspaceCatalog.filtered("").isEmpty)
        XCTAssertTrue(WorkspaceCatalog.filtered("DNS").contains { $0.id == "DNS" })
        XCTAssertTrue(WorkspaceCatalog.matches("capture baseline", query: "cbs"))
        XCTAssertTrue(WorkspaceCatalog.filtered("this-cannot-match-123456789").isEmpty)
        XCTAssertEqual(Set(WorkspaceCatalog.commands.map(\.id)).count, WorkspaceCatalog.commands.count)
    }
    func testCounterOnlyChangeIsNotConfigurationDrift() throws {
        XCTAssertTrue(SnapshotComparison.changes(from: try sample(rx: 100), to: try sample(rx: 100000)).isEmpty)
    }
    func testTypedConfigurationDiff() throws {
        let changes = SnapshotComparison.changes(from: try sample(), to: try sample(mtu: 1400, active: false, address: "10.0.0.8"))
        XCTAssertEqual(Set(changes.map(\.field)), Set(["MTU", "Link", "Addresses"]))
        XCTAssertTrue(changes.contains { $0.field == "MTU" && $0.before == "1500" && $0.after == "1400" })
    }
    func testAddedAndRemovedInterfaces() throws {
        let changes = SnapshotComparison.changes(from: try sample(name: "en0"), to: try sample(name: "en1"))
        XCTAssertEqual(changes.count, 2)
        XCTAssertTrue(changes.contains { $0.interface == "en0" && $0.after == "Removed" })
        XCTAssertTrue(changes.contains { $0.interface == "en1" && $0.after == "Added" })
    }
    func testPrivacyCoversBothBaselineAndCurrent() throws {
        let a = try sample(address: "192.168.1.8"), b = try sample(address: "10.0.0.8")
        let text = SnapshotPrivacy.redacted("192.168.1.8 → 10.0.0.8 / 02:00:00:00:00:08", snapshots: [a, b])
        XCTAssertEqual(text, "<IPv4> → <IPv4> / <MAC>")
    }
    func testRedactedJSONIsStillWellFormed() throws {
        let a = try sample()
        let json = String(decoding: try JSONEncoder().encode(a), as: UTF8.self)
        let masked = SnapshotPrivacy.redacted(json, snapshots: [a])
        XCTAssertNoThrow(try JSONSerialization.jsonObject(with: Data(masked.utf8)))
        XCTAssertFalse(masked.contains("192.168.1.8"))
    }
    func testDuplicateInterfaceNamesAreRejected() throws {
        let a = try sample()
        var object = try XCTUnwrap(JSONSerialization.jsonObject(with: JSONEncoder().encode(a)) as? [String: Any])
        let interfaces = try XCTUnwrap(object["interfaces"] as? [[String: Any]])
        object["interfaces"] = interfaces + interfaces
        XCTAssertThrowsError(try NetworkSnapshot.decode(JSONSerialization.data(withJSONObject: object)))
    }
}
