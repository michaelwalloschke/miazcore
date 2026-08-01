#!/usr/bin/env bash
set -euo pipefail
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
lock="$root/.scratch/learning-client/.realm-test.lock"
run="$root/artifacts/shared-host-replication/$(date -u +%Y%m%dT%H%M%SZ)-placement-probe"
mkdir "$lock" 2>/dev/null || { echo "Placement Probe is already owned" >&2; exit 75; }
cleanup() { rmdir "$lock" 2>/dev/null || true; }
trap cleanup EXIT INT TERM
mkdir -p "$run"
cd "$root"
MIAZCORE_REALM_LOCK_HELD=1 ./infra/azerothcore/realm reset-state --yes
cargo build --locked -q -p learning_client --example fixture_pair_ready
target/debug/examples/fixture_pair_ready pair-a >"$run/pair-a.tmp" 2>&1 & a=$!
target/debug/examples/fixture_pair_ready pair-b >"$run/pair-b.tmp" 2>&1 & b=$!
wait "$a"; wait "$b"
python3 - "$run" <<'PY'
import json, pathlib, re, sys
p=pathlib.Path(sys.argv[1]); rows=[]
for token in ('pair-a','pair-b'):
 m=re.fullmatch(r'PAIR_READY profile=(pair-[ab]) guid=0x([0-9a-f]+) map=(\d+) east=(-?\d+\.\d+) north=(-?\d+\.\d+) elevation=(-?\d+\.\d+) orientation=(-?\d+\.\d+)\n?',(p/f'{token}.tmp').read_text())
 if not m: raise SystemExit('Placement Probe failed: malformed ready evidence')
 q=m.groups(); rows.append({'profile':q[0],'guid':f'0x{q[1][-8:]}','map':int(q[2]),'east':float(q[3]),'north':float(q[4]),'elevation':float(q[5]),'orientation':float(q[6])})
a,b=rows
if a['guid']==b['guid'] or a['map']!=b['map'] or abs((b['east']-a['east'])-3)>0.001 or any(abs(a[k]-b[k])>0.001 for k in ('north','elevation','orientation')): raise SystemExit('Placement Probe failed: relation invariant')
(p/'pair-a.tmp').unlink(); (p/'pair-b.tmp').unlink()
(p/'summary.json').write_text(json.dumps({'schema':'miazcore.fixture-pair-placement.v1','profiles':rows},indent=2)+'\n')
PY
./infra/azerothcore/realm wait-character-offline
MIAZCORE_REALM_LOCK_HELD=1 ./infra/azerothcore/realm reset-state --yes
./infra/azerothcore/realm health
