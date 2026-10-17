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
print("JSON schema/fixture and Info.plist contract checks passed")
