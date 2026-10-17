#!/usr/bin/env python3
"""Live verification of `macora diagnose` against independent measurements. Read-only.
Usage: python3 scripts/verify-diagnose.py target/release/macora"""
import html.parser, json, re, subprocess, sys, time

BIN = sys.argv[1]
results = []
def sh(cmd, timeout=120):
    r = subprocess.run(cmd, shell=isinstance(cmd, str), capture_output=True, text=True, timeout=timeout)
    return r.returncode, r.stdout, r.stderr
def m(*a, timeout=120):
    return sh([BIN, *a], timeout)
def check(name, ok, detail=""):
    results.append(ok); print(("PASS  " if ok else "FAIL  ") + name + ("" if ok else f"   <-- {detail}"))

# ---- timing budget and basic shape
t = time.time(); rc, full, _ = m("diagnose", "json"); dt = time.time() - t
check(f"full run finishes within the 15 s budget ({dt:.1f} s)", dt < 15)
d = json.loads(full)
names = [s["step"] for s in d["steps"]]
check("full run has the expected steps in layer order", names[:1] == ["Link"] and {"Gateway", "Internet", "Portal", "DNS", "HTTPS", "MTU", "IPv6"} <= set(names), str(names))
check("every status is one of OK/WARN/FAIL/SKIP/INFO", all(s["status"] in {"OK", "WARN", "FAIL", "SKIP", "INFO"} for s in d["steps"]))
check("exit code agrees with the report (1 only if some step FAILs)", (rc == 1) == any(s["status"] == "FAIL" for s in d["steps"]), f"rc={rc}")
check("verdict is non-empty text", len(d["verdict"]) > 20)
t = time.time(); rc, q, _ = m("diagnose", "quick", "json"); dq = time.time() - t
check(f"quick finishes in under 5 s ({dq:.1f} s) and skips Wi-Fi + MTU", dq < 5 and {s["step"]: s["status"] for s in json.loads(q)["steps"]}.get("MTU") == "SKIP")

# ---- independent ground truth
steps = {s["step"]: s for s in d["steps"]}
_, route, _ = sh("/sbin/route -n get default"); gw = re.search(r"gateway: (\S+)", route).group(1); iface = re.search(r"interface: (\S+)", route).group(1)
check("Gateway step names the real default gateway", gw in steps["Gateway"]["detail"], steps["Gateway"]["detail"])
check("Link step names the real default-route interface", iface in steps["Link"]["detail"], steps["Link"]["detail"])
_, ping, _ = sh(f"/sbin/ping -c 10 -i 0.2 -n -q {gw}")
mine = steps["Gateway"]["ms"]; theirs = float(re.search(r"= [\d.]+/([\d.]+)/", ping).group(1))
check(f"Gateway latency agrees with an independent ping ({mine:.1f} vs {theirs:.1f} ms)", mine is not None and abs(mine - theirs) < 25)
_, dig, _ = sh("/usr/bin/dig +time=2 +tries=1 @1.1.1.1 example.com")
refms = float(re.search(r"Query time: (\d+)", dig).group(1))
ref = re.search(r"reference 1\.1\.1\.1 ([\d.]+) ms", steps["DNS"]["detail"])
check(f"DNS reference timing agrees with dig ({ref.group(1) if ref else '?'} vs {refms:.0f} ms)", bool(ref) and abs(float(ref.group(1)) - refms) < 80)
_, curl, _ = sh("/usr/bin/curl -4 -s -o /dev/null -w '%{time_total}' --max-time 8 https://www.apple.com")
tot = re.search(r"\(total ([\d.]+) ms\)", steps["HTTPS"]["detail"])
check(f"HTTPS total agrees with curl ({tot.group(1) if tot else '?'} vs {float(curl)*1000:.0f} ms)", bool(tot) and abs(float(tot.group(1)) - float(curl) * 1000) < 400)
# Path MTU against manual bisection.
def df(size):
    r = subprocess.run(["/sbin/ping", "-c", "2", "-i", "0.2", "-n", "-q", "-D", "-s", str(size - 28), "-t", "1", "1.1.1.1"], capture_output=True, text=True)
    return "0 packets received" not in r.stdout and "100.0%" not in r.stdout
manual = next((s for s in range(1500, 1279, -1) if df(s)), None)
mm = re.search(r"is (\d+) bytes", steps["MTU"]["detail"]); mt = int(mm.group(1)) if mm else 1500 if "full link size" in steps["MTU"]["detail"] else None
check(f"path MTU equals a manual DF bisection ({mt} vs {manual})", mt is not None and manual is not None and abs(mt - manual) <= 2, steps["MTU"]["detail"])
_, sp, _ = sh("/usr/sbin/system_profiler SPAirPortDataType -json", timeout=60)
sn = re.search(r'"spairport_signal_noise" : "(-?\d+) dBm / (-?\d+) dBm"', sp)
wf = re.search(r"signal (-?\d+) dBm, noise (-?\d+) dBm", steps["Wi-Fi"]["detail"]) if "Wi-Fi" in steps else None
check(f"Wi-Fi signal agrees with system_profiler ({wf.groups() if wf else None} vs {sn.groups() if sn else None})", bool(sn and wf) and abs(int(wf.group(1)) - int(sn.group(1))) <= 8)
_, cap, _ = sh("/usr/bin/curl -s --max-time 5 http://captive.apple.com/hotspot-detect.html")
check("Portal step agrees with a direct captive-portal fetch", (steps["Portal"]["status"] == "OK") == ("Success" in cap))

# ---- site check
rc, out, _ = m("diagnose", "quick", "to", "github.com", "json"); s2 = {s["step"]: s for s in json.loads(out)["steps"]}
check("to github.com adds Site DNS, Site HTTPS and Site path, all OK", all(s2.get(k, {}).get("status") == "OK" for k in ["Site DNS", "Site HTTPS", "Site path"]), str({k: s2.get(k, {}).get("status") for k in s2 if k.startswith("Site")}))
rc, out, _ = m("diagnose", "quick", "to", "no-such-host-macora-test.invalid", "json"); s3 = {s["step"]: s["status"] for s in json.loads(out)["steps"]}
check("a nonexistent site fails Site DNS, skips the rest, and exits 1", s3.get("Site DNS") == "FAIL" and s3.get("Site HTTPS") == "SKIP" and rc == 1, str(s3))
rc, out, _ = m("diagnose", "quick", "to", "localhost", "json"); s4 = {s["step"]: s["status"] for s in json.loads(out)["steps"]}
check("localhost resolves via the system resolver (WARN, not a failure)", s4.get("Site DNS") == "WARN", str(s4))
for bad in ["a;b", "a b", "http://x", "host:443"]:
    rc, _, err = m("diagnose", "to", bad);
    if rc != 1: check(f"rejects {bad!r}", False)
check("hostile site names are rejected before any probe", True)

# ---- formats
rc, md, _ = m("diagnose", "quick", "md"); check("markdown has the table and verdict", md.startswith("# MACORA network diagnosis") and "**Verdict:**" in md and md.count("\n| ") >= 5)
rc, hp, _ = m("diagnose", "quick", "html")
class P(html.parser.HTMLParser):
    def __init__(s): super().__init__(); s.tags = []; s.err = False
    def handle_starttag(s, t, a): s.tags.append(t)
p = P(); p.feed(hp)
check("html parses, has a table, and no script tag", "table" in p.tags and "script" not in p.tags and hp.startswith("<!doctype html>"))
rc, txt, _ = m("diagnose", "quick"); check("plain text has VERDICT and no escape codes in a pipe", "VERDICT" in txt and "\x1b" not in txt)

# ---- redaction leak test, every format
_, ifa, _ = sh("/sbin/ifconfig -a")
secrets = set(re.findall(r"inet (\d+\.\d+\.\d+\.\d+)", ifa)) - {"127.0.0.1"}
_, sc, _ = sh("/usr/sbin/scutil --dns"); secrets |= set(re.findall(r"nameserver\[\d+\] : (\S+)", sc)) | {gw}
secrets |= {x.split("%")[0] for x in secrets if "%" in x}
secrets -= {"1.1.1.1", "8.8.8.8", "9.9.9.9"}
leaks = []
for fmt in ([], ["json"], ["md"], ["html"]):
    _, o, _ = m("diagnose", "quick", "redact", *fmt)
    leaks += [(" ".join(fmt) or "text", s) for s in secrets if s in o]
check(f"redact leaks none of {len(secrets)} local identifiers in text, json, md and html", not leaks, str(leaks[:3]))
_, o, _ = m("diagnose", "quick", "redact"); check("public probe targets stay readable under redact", "1.1.1.1" in o and "<IPv4>" in o)

# ---- repeat mode and strict
rc, out, _ = m("diagnose", "quick", "count", "3", "interval", "1")
check("count 3 prints 3 round lines and a summary", len(re.findall(r"^\[\d/3\]", out, re.M)) == 3 and "3 rounds" in out and "Gateway: min" in out, out[-200:])
rc1, _, _ = m("diagnose", "strict"); rc0, _, _ = m("diagnose")
warn = any(s["status"] == "WARN" for s in d["steps"])
check(f"strict exits 1 exactly when there is a warning (plain={rc0}, strict={rc1}, warn={warn})", (rc1 == 1) == (warn or rc0 == 1))

# ---- speed (opt-in; ~15 s)
t = time.time(); rc, out, _ = m("diagnose", "quick", "speed", "json", timeout=120); ds = time.time() - t
sp = {s["step"]: s for s in json.loads(out)["steps"]}.get("Speed")
mm = re.search(r"down (\d+) Mbps · up (\d+) Mbps", sp["detail"]) if sp else None
check(f"speed adds a Speed step with Mbps and responsiveness ({ds:.0f} s)", bool(mm) and "responsiveness" in sp["detail"] and 5 < ds < 60, sp["detail"] if sp else "no step")
_, nq, _ = sh("/usr/bin/networkQuality -c", timeout=90)
nqd = json.loads(nq)["dl_throughput"] / 1e6
check(f"speed result is the same order as a second networkQuality run ({mm.group(1) if mm else '?'} vs {nqd:.0f} Mbps)", bool(mm) and 0.4 < float(mm.group(1)) / nqd < 2.5)
rc, out, _ = m("diagnose", "quick", "json"); check("speed is never run by default", "Speed" not in {s["step"] for s in json.loads(out)["steps"]})

print(f"\n{sum(results)} passed, {len(results) - sum(results)} failed")
sys.exit(0 if all(results) else 1)
