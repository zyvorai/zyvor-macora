#!/bin/bash
# Read-only smoke test of the macora CLI. Never applies a configuration change.
# Usage: scripts/smoke-cli.sh [path/to/macora]
set -uo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
bin="${1:-$root/target/release/macora}"
fixture="$root/tests/fixtures/macos.ifconfig"
[[ -x "$bin" ]] || { echo "binary not found: $bin (run cargo build --release)" >&2; exit 2; }
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
pass=0; fail=0

run() { # run <expected-exit> <args...>
  local want="$1"; shift
  local out; out="$("$bin" "$@" 2>&1)"; local rc=$?
  if [[ $rc -eq $want ]]; then pass=$((pass+1)); printf 'ok    macora %s\n' "$*"
  else fail=$((fail+1)); printf 'FAIL  macora %s (exit %s, wanted %s)\n%s\n' "$*" "$rc" "$want" "$out" >&2; fi
}
run_ok_or_fail() { # exit 0 or 1 both fine (environment-dependent), anything else is a crash
  local out; out="$("$bin" "$@" 2>&1)"; local rc=$?
  if [[ $rc -le 1 ]]; then pass=$((pass+1)); printf 'ok    macora %s (exit %s)\n' "$*" "$rc"
  else fail=$((fail+1)); printf 'FAIL  macora %s (exit %s)\n%s\n' "$*" "$rc" "$out" >&2; fi
}
json() { # json <args...>: output must parse
  if "$bin" "$@" 2>/dev/null | python3 -m json.tool >/dev/null; then pass=$((pass+1)); printf 'ok    macora %s (valid JSON)\n' "$*"
  else fail=$((fail+1)); printf 'FAIL  macora %s (invalid JSON)\n' "$*" >&2; fi
}

run 0 --version
run 0 --help
run 0
run 0 --all
run 0 --all --compact --redact
run 0 --from "$fixture" --all
run 0 --from "$fixture" --interface en0 --ipv6
run 0 explain en0 --from "$fixture"
run 0 routes
run 0 dns
run 0 path
run 0 services
run_ok_or_fail wifi            # no Wi-Fi hardware on VMs
run 0 neighbors
run 0 sockets
run 0 topology
run 0 report --redact
run 0 report --format html --redact
run 0 report --format md
run 0 show --format prometheus
run_ok_or_fail diagnose        # exits 1 when a layer fails, e.g. a VM gateway that drops ping
diag_json="$("$bin" diagnose --json 2>/dev/null || true)"
if printf '%s' "$diag_json" | python3 -c 'import sys,json; d=json.load(sys.stdin); assert d["steps"] and d["verdict"]'; then pass=$((pass+1)); echo "ok    macora diagnose --json (structured)"; else fail=$((fail+1)); echo "FAIL  macora diagnose --json" >&2; fi
run 0 ping 127.0.0.1 --count 2
run 0 trace 127.0.0.1
run 0 completions bash
run 0 completions zsh
run 0 completions fish
run 0 doctor
run 0 subnet 192.168.1.8/24
run 0 resolve localhost
run 0 set en0 mtu 1400 --from "$fixture"        # preview only, no --apply
run 0 watch --count 2 --interval 1
json --json
json --json --all
json --from "$fixture" --json
if "$bin" watch --count 2 --interval 1 --json 2>/dev/null | python3 -c 'import sys,json; n=[json.loads(l) for l in sys.stdin]; assert len(n)==2'; then
  pass=$((pass+1)); echo 'ok    macora watch --json (2 NDJSON frames)'
else fail=$((fail+1)); echo 'FAIL  macora watch --json' >&2; fi
run 1 --apply
run 1 ping 'a;b'
run 1 completions powershell
run 1 --format yaml
run 1 definitely-not-a-command
run 1 subnet not-a-cidr

"$bin" snapshot "$tmp/a.macora" >/dev/null 2>&1 && "$bin" snapshot "$tmp/b.macora" >/dev/null 2>&1 \
  && run 0 diff "$tmp/a.macora" "$tmp/b.macora" --redact \
  || { fail=$((fail+1)); echo 'FAIL  snapshot/diff' >&2; }
[[ "$(stat -f %Lp "$tmp/a.macora" 2>/dev/null || stat -c %a "$tmp/a.macora")" == 600 ]] \
  && { pass=$((pass+1)); echo 'ok    snapshot mode 0600'; } || { fail=$((fail+1)); echo 'FAIL  snapshot mode' >&2; }
run 1 snapshot "$tmp/a.macora"                    # must refuse to overwrite

printf '\n%s passed, %s failed\n' "$pass" "$fail"
[[ $fail -eq 0 ]]
