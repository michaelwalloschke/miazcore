#!/usr/bin/env bash
set -euo pipefail

# Two reset-scoped, live failure probes for Ticket 18. Neither probe reads the
# database to establish success; the only acceptance evidence is the client's
# explicitly rejected semantic operation or its reconnect failure.
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
lock_dir="$root/.scratch/learning-client/.realm-test.lock"
cd "$root"

mkdir -p artifacts
if ! mkdir "$lock_dir" 2>/dev/null; then
    echo "Persisted Movement negative probes are already owned by another process" >&2
    exit 75
fi

worldserver_stopped=false
client_pid=""
cleanup() {
    if [[ -n "$client_pid" ]]; then
        kill "$client_pid" 2>/dev/null || true
        wait "$client_pid" 2>/dev/null || true
    fi
    if [[ "$worldserver_stopped" == true ]]; then
        ./infra/azerothcore/realm start-worldserver || true
    fi
    rmdir "$lock_dir"
}
trap cleanup EXIT INT TERM

wait_for_realm_health() {
    # AzerothCore can retain the recently disconnected fixture character as
    # online for a short server-side cleanup interval. Keep this bounded and
    # fail closed if the realm does not return to its semantic fixture state.
    for _ in {1..60}; do
        if ./infra/azerothcore/realm health; then
            return 0
        fi
        sleep 1
    done
    echo "realm did not return to a healthy fixture state within 60 seconds" >&2
    return 1
}

short_image="artifacts/persisted-movement-short-negative.png"
short_log="artifacts/persisted-movement-short-negative.log"
./infra/azerothcore/realm reset-state --yes
wait_for_realm_health
scripts/macos-compositor-proof.sh "$short_image" \
    --persisted-movement-short-negative-external-proof-output "$short_log"

python3 - "${short_image%.*}.json" <<'PY'
import json, pathlib, sys
sidecar = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
if sidecar.get("phase") != "PersistedMovementRejected":
    raise SystemExit("short-move negative probe did not reach the rejected proof phase")
if sidecar.get("movement_proof") is not None:
    raise SystemExit("short-move negative probe unexpectedly created a success oracle")
if sidecar.get("submitted_pose") != sidecar.get("entry_anchor"):
    raise SystemExit("short-move negative probe unexpectedly left the entry anchor")
if sidecar.get("failure_context") != "movement proof requires a submitted stopped pose at least two metres from entry":
    raise SystemExit("short-move negative probe did not retain its explicit rejection")
print("short-move negative probe passed: zero-distance stopped pose was rejected before persistence")
PY
wait_for_realm_health

fault_output="artifacts/persisted-movement-reconnect-unavailable.png"
fault_log="artifacts/persisted-movement-reconnect-unavailable.log"
fault_stage="${fault_output%.*}.stage"
fault_ack="${fault_output%.*}.ack"
fault_sidecar="${fault_output%.*}.json"
rm -f "$fault_output" "$fault_stage" "$fault_ack" "$fault_sidecar"
./infra/azerothcore/realm reset-state --yes
wait_for_realm_health
cargo build --locked -p learning_client >"$fault_log" 2>&1
WGPU_BACKEND=metal RUST_LOG=info target/debug/learning_client \
    --persisted-movement-fault-injection-external-proof-output "$fault_output" >>"$fault_log" 2>&1 &
client_pid=$!

for _ in {1..360}; do
    [[ -f "$fault_stage" && "$(<"$fault_stage")" == "reconnecting" ]] && break
    kill -0 "$client_pid" 2>/dev/null || { cat "$fault_log" >&2; exit 1; }
    sleep 0.25
done
[[ -f "$fault_stage" ]] || { echo "timed out waiting for the reconnect proof stage" >&2; exit 1; }
[[ "$(<"$fault_stage")" == "reconnecting" ]] || { echo "unexpected proof stage" >&2; exit 1; }

./infra/azerothcore/realm stop-worldserver
worldserver_stopped=true
printf 'worldserver-stopped\n' >"$fault_ack"
for _ in {1..180}; do
    ! kill -0 "$client_pid" 2>/dev/null && break
    sleep 0.25
done
if kill -0 "$client_pid" 2>/dev/null; then
    echo "timed out waiting for the injected reconnect failure" >&2
    exit 1
fi
wait "$client_pid" || true
client_pid=""
rg -q 'movement proof failed before fresh reconnect comparison' "$fault_log"
rg -q 'world connection' "$fault_log"

./infra/azerothcore/realm start-worldserver
worldserver_stopped=false
python3 - "$fault_sidecar" <<'PY'
import json, pathlib, sys
pathlib.Path(sys.argv[1]).write_text(json.dumps({
    "schema": "miazcore.persisted-movement-negative-probe.v1",
    "phase": "ReconnectUnavailableRejected",
    "network": "reference-realm",
    "oracle": "client-reconnect-failure",
    "database_derived_success": False,
}, indent=2) + "\n", encoding="utf-8")
PY
wait_for_realm_health
echo "persisted movement negative probes passed: short eligibility rejection and injected reconnect failure"
