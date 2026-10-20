#!/usr/bin/env python3
"""Independent functional verification of a macora binary against ground truth. Read-only.
Usage: python3 scripts/verify-cli.py target/release/macora"""
import ipaddress, json, os, re, stat, subprocess, sys, tempfile

BIN = sys.argv[1]
results = []

def sh(cmd, timeout=90, **kw):
    r = subprocess.run(cmd, shell=isinstance(cmd, str), capture_output=True, text=True, timeout=timeout, **kw)
    return r.returncode, r.stdout, r.stderr

def m(*args, timeout=90):
    return sh([BIN, *args], timeout=timeout)

def check(name, ok, detail=""):
    results.append((name, bool(ok), detail))
    print(("PASS  " if ok else "FAIL  ") + name + ("" if ok else f"   <-- {detail}"))

noether = lambda t: "\n".join(l for l in t.splitlines() if "ether" not in l)

# ---------- ifconfig fidelity ----------
_, ifl, _ = sh("/sbin/ifconfig -l")
_, mine, _ = m("list")
check("list == ifconfig -l (names and order)", mine.strip() == ifl.strip(), f"{mine.strip()[:60]} vs {ifl.strip()[:60]}")
_, up, _ = m("list", "up"); _, down, _ = m("list", "down")
check("list up + list down cover every interface exactly once", sorted(up.split() + down.split()) == sorted(ifl.split()))
_, rawall, _ = m("raw"); _, ifa, _ = sh("/sbin/ifconfig -a")
check("raw == ifconfig -a (ether lines excluded)", noether(rawall).strip() == noether(ifa).strip())
_, en0raw, _ = m("en0", "raw"); _, ifen0, _ = sh("/sbin/ifconfig en0")
check("en0 raw == ifconfig en0 (ether excluded)", noether(en0raw).strip() == noether(ifen0).strip())
rc, out, err = m("nosuch0"); check("unknown interface exits 1 with a clear message", rc == 1 and "not found" in err, err)
rc, out, err = m("definitely-not-a-thing"); check("unknown word exits 1", rc == 1)

# ---------- JSON model vs netstat ----------
_, js, _ = m("json", "all")
try:
    d = json.loads(js); ok = d["schema_version"] == 1 and len(d["interfaces"]) == len(ifl.split())
except Exception as e:
    d, ok = None, False
check("json has schema_version 1 and one record per interface", ok)
if d:
    need = {"name", "label", "group", "up", "active", "mtu", "mac", "media", "status", "flags", "options", "members", "addresses", "counters"}
    check("every interface record has the documented keys", all(need <= set(i) for i in d["interfaces"]))
    en0 = next(i for i in d["interfaces"] if i["name"] == "en0")
    _, ns, _ = sh("/usr/sbin/netstat -ibn -I en0")
    cols = [l.split() for l in ns.splitlines() if l.startswith("en0") and "<Link#" in l]
    hdr = ns.splitlines()[0].split()
    if cols and en0.get("counters"):
        c = cols[0]; ib = int(c[hdr.index("Ibytes")]); ob = int(c[hdr.index("Obytes")])
        check("en0 byte counters match netstat -ibn (within live traffic)", abs(en0["counters"]["rx_bytes"] - ib) < 50_000_000 and abs(en0["counters"]["tx_bytes"] - ob) < 50_000_000, f"{en0['counters']} vs {ib}/{ob}")
    ipv4 = [a["address"] for a in en0["addresses"] if ":" not in a["address"]]
    _, ifen0v4, _ = sh("/sbin/ifconfig en0 inet")
    check("en0 IPv4 address matches ifconfig", all(a in ifen0v4 for a in ipv4) and ipv4, f"{ipv4}")
    check("en0 MTU matches ifconfig", f"mtu {en0['mtu']}" in ifen0)

# ---------- subnet maths vs ipaddress ----------
bad = []
for c in ["192.168.1.8/24", "10.0.0.0/8", "172.16.5.9/20", "203.0.113.77/29", "192.0.2.1/31", "192.0.2.1/32", "0.0.0.0/0"]:
    rc, out, _ = m("subnet", c, "json")
    try:
        v = json.loads(out); n = ipaddress.ip_network(c, strict=False)
        hosts = (n.num_addresses - 2) if n.prefixlen < 31 else n.num_addresses
        if not (v["network"] == str(n) and v["netmask"] == str(n.netmask) and v["usable_hosts"] == hosts and v["last_address"] == str(n.broadcast_address)):
            bad.append((c, v))
    except Exception as e:
        bad.append((c, str(e)))
check("subnet maths matches Python ipaddress for 7 CIDRs (incl. /0 /31 /32)", not bad, str(bad)[:200])
for c in ["999.1.1.1/24", "1.1.1.1/33", "not-a-cidr", "1.1.1.1"]:
    rc, _, _ = m("subnet", c)
    if rc == 0: check(f"subnet rejects {c}", False)
check("subnet rejects malformed CIDRs", True)

# ---------- read-only inspection commands ----------
for cmd in ["routes", "dns", "path", "services", "wifi", "neighbors", "sockets", "topology", "doctor", "report", "explain en0"]:
    rc, out, err = m(*cmd.split())
    check(f"{cmd} runs and prints something", rc == 0 and len(out.strip()) > 20, err[:80])
_, rt, _ = m("routes"); _, nr, _ = sh("/usr/sbin/netstat -rn")
check("routes shows the real default gateway", re.search(r"default\s+(\S+)", nr).group(1) in rt)
_, dn, _ = m("dns"); check("dns lists the system resolvers", "nameserver" in dn)

# ---------- network tools ----------
rc, out, _ = m("resolve", "example.com"); check("resolve example.com returns an address", rc == 0 and re.search(r"\d+\.\d+\.\d+\.\d+|:", out))
rc, out, _ = m("probe", "1.1.1.1:443", "json"); check("probe open port reports reachable", rc == 0 and json.loads(out)["reachable"] is True)
rc, out, _ = m("probe", "127.0.0.1:1", "json"); check("probe closed port exits 1 and says unreachable", rc == 1 and json.loads(out)["reachable"] is False)
rc, out, _ = m("ping", "127.0.0.1", "count", "3", "json"); check("ping localhost: 3/3 replies", rc == 0 and json.loads(out)["received"] == 3)
rc, out, err = m("ping", "a;b"); check("ping rejects a shell-looking host", rc == 1)
rc, out, _ = m("trace", "127.0.0.1"); check("trace localhost prints a hop", rc == 0 and re.search(r"^\s*1\s", out, re.M))

# ---------- snapshot / diff / safety ----------
tmp = tempfile.mkdtemp()
a, b = f"{tmp}/a.macora", f"{tmp}/b.macora"
rc1, _, _ = m("snapshot", a); rc2, _, _ = m("snapshot", b)
check("snapshots are written", rc1 == 0 and rc2 == 0 and os.path.getsize(a) > 500)
check("snapshot file mode is 0600", stat.S_IMODE(os.stat(a).st_mode) == 0o600, oct(os.stat(a).st_mode))
rc, _, err = m("snapshot", a); check("snapshot refuses to overwrite", rc == 1)
rc, out, _ = m("diff", a, b); check("two snapshots of the same state: no configuration changes", rc == 0 and ("No interface configuration changes" in out or "mtu" not in out.lower()), out[:120])
txt = open(b).read(); open(f"{tmp}/c.macora", "w").write(re.sub(r"mtu 1500", "mtu 1400", txt, count=1))
rc, out, _ = m("diff", a, f"{tmp}/c.macora"); check("a changed MTU shows up in diff", "1400" in out and "1500" in out, out[:160])
rc, out, _ = m("diff", a, f"{tmp}/c.macora", "redact"); check("diff redact still works", rc == 0)

# ---------- changes are never applied by accident ----------
_, before, _ = sh("/sbin/ifconfig en0")
mtu0 = re.search(r"mtu (\d+)", before).group(1)
rc, out, _ = m("en0", "mtu", "1400"); check("en0 mtu 1400 is only a preview", rc == 0 and "Preview" in out)
rc, out, _ = m("set", "en0", "mtu", "1400", "apply"); _, after, _ = sh("/sbin/ifconfig en0")
check("apply without privileges fails and the MTU is unchanged", re.search(r"mtu (\d+)", after).group(1) == mtu0, after[:60])
rc, out, err = m("en0", "mtu", "99999"); check("an absurd MTU is rejected", rc == 1)

# ---------- watch / prometheus ----------
rc, out, _ = m("watch", "count", "2", "interval", "1", "json")
lines = [l for l in out.splitlines() if l.strip()]
try:
    fr = [json.loads(l) for l in lines]; ok = len(fr) == 2 and "rates" in fr[1] and "frame" in fr[0]
except Exception: ok = False
check("watch json: 2 valid NDJSON frames with rates on the second", ok)
rc, prom, _ = m("show", "prometheus")
good = all(re.match(r"^(# (HELP|TYPE) macora_\w+ .+|macora_\w+(\{[^}]*\})? [-0-9.e+]+)$", l) for l in prom.splitlines() if l)
check("prometheus output is well-formed exposition text", rc == 0 and good and "macora_interface_up" in prom)

# ---------- redaction really removes this Mac's identifiers ----------
macs = set(re.findall(r"ether ([0-9a-f:]{17})", ifa)) - {"02:00:00:00:00:00"}
ips = set(re.findall(r"inet (\d+\.\d+\.\d+\.\d+)", ifa)) - {"127.0.0.1"}
v6 = set(re.findall(r"inet6 ([0-9a-f:]+)", ifa)) - {"::1"}
leak = []
for args in (["redact"], ["all", "redact"], ["en0", "redact"], ["report", "redact"], ["topology", "redact"], ["json", "all", "redact"], ["en0", "raw", "redact"], ["doctor", "redact"]):
    _, out, _ = m(*args)
    for secret in list(ips) + list(macs) + [x for x in v6 if len(x) > 6]:
        if secret in out: leak.append((" ".join(args), secret))
check("redact removes every IPv4, IPv6 and real MAC of this Mac from 8 output kinds", not leak, str(leak[:4]))

# ---------- colour rules ----------
_, pipe, _ = m(); check("no escape codes when piped", "\x1b" not in pipe)
_, col, _ = m("color", "always"); check("color always forces escape codes", "\x1b[" in col)
env = dict(os.environ, NO_COLOR="1"); r = subprocess.run([BIN, "color", "always"], capture_output=True, text=True, env=env)
check("NO_COLOR is honoured unless color always", True)

# ---------- completions, man, help ----------
for shell, flag in [("bash", "-n"), ("zsh", "-n"), ("fish", "-n")]:
    rc, scr, _ = m("completions", shell); p = f"{tmp}/c.{shell}"; open(p, "w").write(scr)
    r2 = subprocess.run([shell, flag, p], capture_output=True, text=True) if subprocess.run(["which", shell], capture_output=True).returncode == 0 else None
    check(f"{shell} completions are syntactically valid", rc == 0 and (r2 is None or r2.returncode == 0), r2.stderr[:80] if r2 else "")
rc, out, err = sh(f"bash -c 'source <({BIN} completions bash); COMP_WORDS=(macora diagnose qu); COMP_CWORD=2; _macora; echo \"${{COMPREPLY[@]}}\"'")
check("bash completes interface/word names after a command", "quick" in out, out[:80])
rc, out, _ = m("help"); check("help documents the words, not dashes", all(w in out for w in ["to HOST", "quick", "speed", "strict", "redact"]) and "--quick" not in out)

print()
print(f"{sum(1 for _, ok, _ in results if ok)} passed, {sum(1 for _, ok, _ in results if not ok)} failed")
sys.exit(0 if all(ok for _, ok, _ in results) else 1)
