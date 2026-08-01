#!/usr/bin/env bash
set -euo pipefail
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
lock="$root/.scratch/learning-client/.realm-test.lock"
started_utc="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
run_id="$(date -u +%Y%m%dT%H%M%SZ)-placement-probe"
run="$root/artifacts/shared-host-replication/$run_id"
commit="$(git -C "$root" rev-parse HEAD)"
temporary_logs="$(mktemp -d "${TMPDIR:-/tmp}/miazcore-placement-probe.XXXXXX")"
mutation_attempted=false
recovery_attempted=false
retain_lock=false
succeeded=false
stage="initial-reset"
a=""
b=""

mkdir "$lock" 2>/dev/null || { echo "Placement Probe is already owned" >&2; exit 75; }
printf 'script=scripts/placement-probe.sh\npid=%s\nstarted_utc=%s\nrun=%s\n' \
    "$$" "$started_utc" "$run_id" >"$lock/owner"

recover_once() {
    [[ "$mutation_attempted" == true && "$recovery_attempted" == false ]] || return 0
    recovery_attempted=true
    echo "Placement Probe failed after a reset attempt; running one canonical recovery" >&2
    if MIAZCORE_SKIP_PULL=1 MIAZCORE_REALM_LOCK_HELD=1 ./infra/azerothcore/realm reset-state --yes \
        && ./infra/azerothcore/realm health; then
        echo "Placement Probe recovery succeeded" >&2
        return 0
    fi

    retain_lock=true
    printf '{"schema":"miazcore.fixture-pair-placement.v1","status":"recovery-failed","commit":"%s","run":"%s"}\n' \
        "$commit" "$run_id" >"$run/recovery-failed.json"
    echo "Placement Probe recovery failed; retaining $lock for operator inspection" >&2
    return 1
}

reap_children() {
    local child reaped=true
    for child in "$a" "$b"; do
        [[ -n "$child" ]] || continue
        kill -TERM "$child" 2>/dev/null || true
    done
    for child in "$a" "$b"; do
        [[ -n "$child" ]] || continue
        wait "$child" 2>/dev/null || reaped=false
    done
    a=""
    b=""
    "$reaped"
}

canonical_reset() {
    local marker="$1"
    unlink "$marker" 2>/dev/null || true
    if ! MIAZCORE_SKIP_PULL=1 MIAZCORE_REALM_LOCK_HELD=1 \
        MIAZCORE_RESET_COMPLETION_FILE="$marker" ./infra/azerothcore/realm reset-state --yes; then
        [[ "$(cat "$marker" 2>/dev/null || true)" == reset-complete ]] || return 1
        echo "reset command returned non-zero after canonical completion marker" >&2
    fi
}

cleanup() {
    local status=$?
    trap - EXIT INT TERM
    if ! reap_children; then
        retain_lock=true
        echo "Placement Probe could not reap Pair clients; retaining $lock" >&2
    fi
    if [[ "$succeeded" != true ]]; then
        printf '{"schema":"miazcore.fixture-pair-placement.v1","status":"failed","stage":"%s","commit":"%s","run":"%s"}\n' \
            "$stage" "$commit" "$run_id" >"$run/failure.json"
        recover_once || true
    fi
    rm -rf "$temporary_logs"
    if [[ "$retain_lock" != true ]]; then
        rm -f "$lock/owner"
        rmdir "$lock" 2>/dev/null || true
    fi
    exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
mkdir -p "$run"
cd "$root"
mutation_attempted=true
canonical_reset "$temporary_logs/initial-reset.marker"
stage="build-pair-client"
cargo build --locked -q -p learning_client --example fixture_pair_ready
stage="pair-movement-ready"
target/debug/examples/fixture_pair_ready pair-a >"$temporary_logs/pair-a" 2>&1 & a=$!
target/debug/examples/fixture_pair_ready pair-b >"$temporary_logs/pair-b" 2>&1 & b=$!
failed=false
wait "$a" || failed=true
wait "$b" || failed=true
a=""
b=""
[[ "$failed" == false ]] || { echo "Placement Probe failed: a Pair profile did not reach MovementReady" >&2; exit 1; }
stage="validate-pair-evidence"
python3 - "$run" "$temporary_logs" "$commit" <<'PY'
import json, pathlib, re, sys
p=pathlib.Path(sys.argv[1]); logs=pathlib.Path(sys.argv[2]); commit=sys.argv[3]; rows=[]
for token in ('pair-a','pair-b'):
 m=re.fullmatch(r'PAIR_READY profile=(pair-[ab]) guid=0x([0-9a-f]+) map=(\d+) east=(-?\d+\.\d+) north=(-?\d+\.\d+) elevation=(-?\d+\.\d+) orientation=(-?\d+\.\d+)\n?',(logs/token).read_text())
 if not m: raise SystemExit('Placement Probe failed: malformed ready evidence')
 q=m.groups(); rows.append({'profile':q[0],'guid':f'0x{q[1][-8:]}','map':int(q[2]),'east':float(q[3]),'north':float(q[4]),'elevation':float(q[5]),'orientation':float(q[6])})
a,b=rows
if a['guid']==b['guid'] or a['map']!=b['map'] or abs((b['east']-a['east'])-3)>0.001 or any(abs(a[k]-b[k])>0.001 for k in ('north','elevation','orientation')): raise SystemExit('Placement Probe failed: relation invariant')
(p/'profiles.json').write_text(json.dumps({'schema':'miazcore.fixture-pair-placement.v1','profiles':rows},indent=2)+'\n')
PY
stage="logout-settlement"
./infra/azerothcore/realm wait-character-offline
stage="final-reset"
canonical_reset "$temporary_logs/final-reset.marker"
stage="final-health"
./infra/azerothcore/realm health
python3 - "$run" "$commit" <<'PY'
import json, pathlib, sys
run, commit = pathlib.Path(sys.argv[1]), sys.argv[2]
profiles = json.loads((run / 'profiles.json').read_text())['profiles']
(run / 'summary.json').write_text(json.dumps({'schema':'miazcore.fixture-pair-placement.v1','status':'passed','commit':commit,'profiles':profiles,'final_realm_health':'passed'}, indent=2) + '\n')
(run / 'profiles.json').unlink()
PY
succeeded=true
