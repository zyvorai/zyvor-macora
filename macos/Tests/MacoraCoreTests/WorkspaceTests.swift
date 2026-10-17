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

final class DiagnosisTests: XCTestCase {
    private let json = """
    {"steps":[{"step":"Link","status":"OK","detail":"ok"},{"step":"Gateway","status":"WARN","detail":"2% loss"},{"step":"Internet","status":"SKIP","detail":"Skipped"}],"verdict":"Connected, but with packet loss."}
    """
    func testDecodesCliReportAndFindsWorstStatus() throws {
        let report = try DiagnosisReport.decode(json)
        XCTAssertEqual(report.steps.map(\.status), [.ok, .warn, .skip])
        XCTAssertEqual(report.worst, .warn)
        XCTAssertEqual(report.steps[1].explanation, "Can this Mac reach your router, and how steadily?")
    }
    func testFailBeatsWarn() {
        let report = DiagnosisReport(steps: [.init(step: "Link", status: .warn, detail: ""), .init(step: "Gateway", status: .fail, detail: "")], verdict: "x")
        XCTAssertEqual(report.worst, .fail)
    }
    func testRejectsMalformedReports() {
        XCTAssertThrowsError(try DiagnosisReport.decode("{\"steps\":[],\"verdict\":\"x\"}"))
        XCTAssertThrowsError(try DiagnosisReport.decode("{\"steps\":[{\"step\":\"Link\",\"status\":\"BOGUS\",\"detail\":\"\"}],\"verdict\":\"x\"}"))
        XCTAssertThrowsError(try DiagnosisReport.decode("not json"))
    }
    func testRateHistoryIsBoundedAndIgnoresBadValues() {
        var history = RateHistory(limit: 3)
        for v in [1.0, 2, 3, 4] { history.append("en0", v) }
        history.append("en0", .nan); history.append("en0", -5); history.append("en0", .infinity)
        XCTAssertEqual(history.values("en0"), [2, 3, 4])
        XCTAssertEqual(history.normalized("en0"), [0.5, 0.75, 1])
        XCTAssertEqual(history.values("missing"), [])
        history.removeAll()
        XCTAssertTrue(history.values("en0").isEmpty)
    }
    func testFlatZeroSeriesStaysFlat() {
        var history = RateHistory()
        history.append("en0", 0); history.append("en0", 0)
        XCTAssertEqual(history.normalized("en0"), [0, 0])
    }
    func testSummaryMasksAddressesWhenAsked() throws {
        let object: [String: Any] = ["schema_version": 1, "interfaces": [["name": "en0", "label": "Wi-Fi", "group": "Network", "up": true, "active": true, "mtu": 1500, "mac": "02:00:00:00:00:08", "media": "autoselect", "status": "active", "flags": [], "options": [], "members": [], "addresses": [["address": "192.168.1.8", "prefix": 24, "scope": "private", "attributes": []]], "counters": ["rx_bytes": 1, "tx_bytes": 1]]], "warnings": []]
        let snapshot = try NetworkSnapshot.decode(JSONSerialization.data(withJSONObject: object))
        XCTAssertTrue(StatusSummary.text(snapshot: snapshot, rx: 1500, tx: 0, report: nil, redact: false).contains("192.168.1.8/24"))
        let masked = StatusSummary.text(snapshot: snapshot, rx: nil, tx: nil, report: nil, redact: true)
        XCTAssertFalse(masked.contains("192.168"))
        XCTAssertTrue(masked.contains("<IPv4>"))
        XCTAssertEqual(StatusSummary.text(snapshot: nil, rx: nil, tx: nil, report: nil, redact: true), "MACORA: no network data yet.")
    }
    func testPaletteHasDiagnoseAndShortcuts() {
        XCTAssertTrue(WorkspaceCatalog.filtered("slow").contains { $0.id == "diagnose" })
        XCTAssertTrue(WorkspaceCatalog.filtered("keyboard").contains { $0.id == "shortcuts" })
    }
    func testProcessRunnerAcceptsDeclaredNonZeroStatus() async throws {
        let ok = try await ProcessRunner.run("/bin/sh", ["-c", "echo hi; exit 1"], acceptedStatuses: [0, 1])
        XCTAssertEqual(ok.trimmingCharacters(in: .whitespacesAndNewlines), "hi")
        do { _ = try await ProcessRunner.run("/bin/sh", ["-c", "exit 1"]); XCTFail("expected failure") } catch {}
    }
    private let newJson = """
    {"steps":[{"step":"Link","status":"OK","detail":"en0 (Wi-Fi): 192.0.2.8","ms":null},{"step":"Route","status":"INFO","detail":"Default route uses en0.","ms":null},{"step":"Gateway","status":"WARN","detail":"192.0.2.1: 0% loss, 80 ms avg","ms":80.25},{"step":"MTU","status":"SKIP","detail":"Skipped in quick mode.","ms":null}],"verdict":"Connected, but with high latency to the router.","advice":["Restart the router."],"also":["Packets larger than 1480 bytes are dropped."],"target":"shop.example.test"}
    """
    func testDecodesNewReportWithInfoMsAdviceAndTarget() throws {
        let r = try DiagnosisReport.decode(newJson)
        XCTAssertEqual(r.steps.map(\.status), [.ok, .info, .warn, .skip])
        XCTAssertEqual(r.steps[2].ms, 80.25)
        XCTAssertNil(r.steps[0].ms)
        XCTAssertEqual(r.advice, ["Restart the router."])
        XCTAssertEqual(r.also.count, 1)
        XCTAssertEqual(r.target, "shop.example.test")
        XCTAssertEqual(r.worst, .warn, "INFO and SKIP are not problems")
        XCTAssertEqual(DiagnosisStatus.info.symbol, "info.circle.fill")
    }
    func testOldReportsWithoutNewFieldsStillDecode() throws {
        let old = #"{"steps":[{"step":"Link","status":"OK","detail":"ok"}],"verdict":"fine"}"#
        let r = try DiagnosisReport.decode(old)
        XCTAssertEqual(r.advice, []); XCTAssertEqual(r.also, []); XCTAssertNil(r.target); XCTAssertNil(r.steps[0].ms)
    }
    func testUnknownStatusIsRejectedNotGuessed() {
        XCTAssertThrowsError(try DiagnosisReport.decode(#"{"steps":[{"step":"Link","status":"MAYBE","detail":"x"}],"verdict":"v"}"#))
    }
    func testEveryKnownStepHasItsOwnExplanation() {
        XCTAssertEqual(Set(DiagnosisStep.known).count, DiagnosisStep.known.count)
        for name in DiagnosisStep.known {
            let e = DiagnosisStep(step: name, status: .ok, detail: "").explanation
            XCTAssertNotEqual(e, name, "\(name) has no explanation")
            XCTAssertTrue(e.hasSuffix("?"), "\(name): \(e)")
        }
        XCTAssertEqual(DiagnosisStep(step: "Brand new", status: .ok, detail: "").explanation, "Brand new", "unknown steps degrade to their name")
    }
    func testRedactionMasksAddressesButKeepsPublicTargets() {
        let text = "192.0.2.1: ok · dns fe80::82ea:bff:fec5:3b4c%en0 · 2001:db8::7 · reference 1.1.1.1 · 8.8.8.8 · 5GHz ch 40 · SNR 28 dB · 0% loss"
        let out = DiagnosisPrivacy.redact(text)
        XCTAssertEqual(out, "<IPv4>: ok · dns <IPv6> · <IPv6> · reference 1.1.1.1 · 8.8.8.8 · 5GHz ch 40 · SNR 28 dB · 0% loss")
        XCTAssertEqual(DiagnosisPrivacy.redact(out), out, "idempotent")
        XCTAssertEqual(DiagnosisPrivacy.redact(""), "")
        XCTAssertEqual(DiagnosisPrivacy.redact("version 3.2.1 build 30201"), "version 3.2.1 build 30201", "version numbers are not addresses")
    }
    func testMarkdownReportRedactsAndEscapes() throws {
        let r = try DiagnosisReport.decode(newJson)
        let md = r.markdown(redact: true)
        XCTAssertTrue(md.hasPrefix("# MACORA network diagnosis"))
        XCTAssertFalse(md.contains("192.0.2."), md)
        XCTAssertTrue(md.contains("<IPv4>") && md.contains("## What to try") && md.contains("## Also noticed"))
        XCTAssertTrue(r.markdown(redact: false).contains("192.0.2.1"))
        let pipe = DiagnosisReport(steps: [.init(step: "Link", status: .ok, detail: "a|b")], verdict: "v")
        XCTAssertTrue(pipe.markdown(redact: false).contains("a\\|b"))
    }
    func testSiteNameValidation() {
        for ok in ["github.com", "a-b.example.test", "localhost", "x_y.example"] { XCTAssertTrue(SiteName.isValid(ok), ok) }
        for bad in ["", "-x", "a b", "a;b", "http://x", "a/b", "host:443", "é.com", String(repeating: "a", count: 254)] { XCTAssertFalse(SiteName.isValid(bad), bad) }
    }
}
