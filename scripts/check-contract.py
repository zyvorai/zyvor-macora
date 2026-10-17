#!/usr/bin/env python3
"""Check the packaged Swift demo contract and Apple bundle metadata without a Mac."""
import json
import plistlib
from pathlib import Path
root = Path(__file__).resolve().parents[1]
data = json.loads((root / "macos/demo.json").read_text())
assert data["schema_version"] == 1
assert len(data["interfaces"]) == 17
assert any("DEMO" in w for w in data["warnings"])
required = {"name", "label", "group", "up", "active", "mtu", "mac", "media", "status", "flags", "options", "members", "addresses", "counters"}
for interface in data["interfaces"]:
    assert required <= interface.keys()
    for address in interface["addresses"]:
        assert {"address", "prefix", "scope", "attributes"} <= address.keys()
info = plistlib.loads((root / "macos/Info.plist").read_bytes())
assert info["CFBundleExecutable"] == "Macora"
assert info["LSMinimumSystemVersion"] == "14.0"
assert info["NSLocationWhenInUseUsageDescription"]

# The step names the Rust analyzer can emit must match what the Swift app knows and explains.
import re
rust = (root / "src/diagnose/analyze.rs").read_text()
rust_steps = set(re.findall(r'Step::new\(\s*"([A-Za-z0-9 \-]+)"', rust))
rust_steps |= set(re.findall(r'for name in \[([^\]]+)\]', rust) and re.findall(r'"([A-Za-z0-9 \-]+)"', re.search(r'for name in \[([^\]]+)\]', rust).group(1)))
swift = (root / "macos/Sources/MacoraCore/Diagnosis.swift").read_text()
known = re.search(r'static let known: \[String\] = \[(.*?)\]', swift, re.S).group(1)
swift_known = set(re.findall(r'"([^"]+)"', known))
explained = set(re.findall(r'case "([^"]+)": return', swift))
assert rust_steps == swift_known, f"Rust emits {sorted(rust_steps - swift_known)} the app does not know; app knows {sorted(swift_known - rust_steps)} Rust never emits"
assert swift_known <= explained, f"no explanation for {sorted(swift_known - explained)}"
# Statuses must match too.
rust_status = set(re.findall(r'Status::\w+ => "([A-Z]+)"', (root / "src/diagnose/facts.rs").read_text()))
swift_status = set(re.findall(r'= "([A-Z]+)"', swift.split("public enum DiagnosisStatus")[1].split("public struct")[0]))
assert rust_status == swift_status, (rust_status, swift_status)
print(f"diagnose contract: {len(rust_steps)} step names and {len(rust_status)} statuses agree between Rust and Swift")
print("JSON schema/fixture and Info.plist contract checks passed")
