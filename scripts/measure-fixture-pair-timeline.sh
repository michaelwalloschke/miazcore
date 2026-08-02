#!/usr/bin/env bash
set -euo pipefail

# Ticket 09's local-only research probe. It owns exactly one reset-scoped
# Fixture Pair study and retains only semantic observations from a controller
# clock shared with the observer transcript. Raw World traffic, credentials,
# accounts, session keys, and client logs never leave its temporary directory.
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
realm="$root/infra/azerothcore/realm"
lock="$root/.scratch/learning-client/.realm-test.lock"
sample_count="${MIAZCORE_TIMELINE_SAMPLE_COUNT:-3}"
run_id="$(date -u +%Y%m%dT%H%M%SZ)-fixture-pair-timeline"
run="$root/artifacts/shared-host-replication/$run_id"
workspace="$(mktemp -d "${TMPDIR:-/tmp}/miazcore-fixture-pair-timeline.XXXXXX")"
commit="$(git -C "$root" rev-parse HEAD)"
mutated=false
recovery_attempted=false
succeeded=false
stage="initializing"

[[ "$sample_count" =~ ^[1-9][0-9]*$ ]] || { echo "sample count must be a positive integer" >&2; exit 64; }
[[ -z "$(git -C "$root" status --porcelain)" ]] || {
    echo "refusing to retain Fixture Pair timing evidence from a dirty worktree" >&2
    exit 2
}
mkdir "$lock" 2>/dev/null || { echo "Fixture Pair timeline measurement is already owned" >&2; exit 75; }
printf 'script=scripts/measure-fixture-pair-timeline.sh\npid=%s\nrun=%s\n' "$$" "$run_id" >"$lock/owner"

recover_once() {
    [[ "$mutated" == true && "$recovery_attempted" == false ]] || return 0
    recovery_attempted=true
    echo "Fixture Pair timeline measurement failed; running one canonical recovery" >&2
    MIAZCORE_SKIP_PULL=1 MIAZCORE_REALM_LOCK_HELD=1 "$realm" reset-state --yes \
        && "$realm" health
}

cleanup() {
    local status=$?
    trap - EXIT INT TERM
    if [[ "$succeeded" != true ]]; then
        mkdir -p "$run"
        printf '{"schema":"miazcore.fixture-pair-timeline.v1","status":"failed","stage":"%s","commit":"%s"}\n' \
            "$stage" "$commit" >"$run/failure.json"
        recover_once || echo "Fixture Pair timeline recovery failed; retaining diagnostics" >&2
    fi
    rm -rf "$workspace"
    rm -f "$lock/owner"
    rmdir "$lock" 2>/dev/null || true
    exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

canonical_reset() {
    local marker="$1"
    rm -f "$marker"
    if ! MIAZCORE_SKIP_PULL=1 MIAZCORE_REALM_LOCK_HELD=1 \
        MIAZCORE_RESET_COMPLETION_FILE="$marker" "$realm" reset-state --yes; then
        [[ "$(cat "$marker" 2>/dev/null || true)" == reset-complete ]] || return 1
        echo "reset command returned non-zero after canonical completion marker" >&2
    fi
    "$realm" health
}

cd "$root"
stage="preflight"
"$realm" preflight
stage="build"
cargo build --locked -q -p learning_client --example measure_fixture_pair_timeline

for ((sample = 1; sample <= sample_count; sample++)); do
    stage="reset-sample-$sample"
    mutated=true
    canonical_reset "$workspace/reset-$sample.marker"
    stage="run-sample-$sample"
    target/debug/examples/measure_fixture_pair_timeline \
        "$workspace/transcript-$sample.json" "$workspace/run-$sample.json"
    stage="validate-sample-$sample"
    python3 - "$workspace/transcript-$sample.json" "$workspace/run-$sample.json" \
        "$workspace/sample-$sample.json" <<'PY'
import json, math, pathlib, sys

transcript_path, run_path, output_path = map(pathlib.Path, sys.argv[1:])
transcript = json.loads(transcript_path.read_text())
run = json.loads(run_path.read_text())
if transcript.get("schema") != "miazcore.remote-transcript.v1":
    raise SystemExit("Fixture Pair timeline has an unexpected observer transcript schema")
if run.get("schema") != "miazcore.fixture-pair-timeline-run.v1":
    raise SystemExit("Fixture Pair timeline has an unexpected controller schema")
timeline = run.get("timeline", {})
required_timeline = (
    "observer_ready_after_ms", "mover_ready_after_ms", "move_start_after_ms",
    "move_stop_after_ms", "logout_requested_after_ms", "mover_proof_complete_after_ms",
)
if any(not isinstance(timeline.get(key), int) or timeline[key] < 0 for key in required_timeline):
    raise SystemExit("Fixture Pair timeline has incomplete shared-clock controller values")
if not timeline["observer_ready_after_ms"] <= timeline["mover_ready_after_ms"] <= timeline["move_start_after_ms"] <= timeline["move_stop_after_ms"] <= timeline["logout_requested_after_ms"] <= timeline["mover_proof_complete_after_ms"]:
    raise SystemExit("Fixture Pair controller actions are not serially ordered")
mover_guid = run.get("mover_guid")
events = [event for event in transcript.get("events", []) if event.get("guid") == mover_guid]
if not events:
    raise SystemExit("Fixture Pair observer did not record the mover GUID")
for event in events:
    if not isinstance(event.get("received_after_ms"), int) or event["received_after_ms"] < 0:
        raise SystemExit("Fixture Pair observer event has no valid shared-clock timestamp")
if [event["received_after_ms"] for event in events] != sorted(event["received_after_ms"] for event in events):
    raise SystemExit("Fixture Pair observer timestamps are not monotonic")
kinds = [event.get("kind") for event in events]
try:
    create_index = kinds.index("create-object2")
    movement_indices = [i for i, event in enumerate(events) if event.get("kind") == "movement"]
    stop_index = next(i for i in movement_indices if events[i].get("opcode") == "0x00b7")
    destroy_index = kinds.index("destroy", stop_index + 1)
except (ValueError, StopIteration) as error:
    raise SystemExit("Fixture Pair observer lacks create, terminal stop, or destroy lifecycle") from error
if not create_index < movement_indices[0] <= stop_index < destroy_index:
    raise SystemExit("Fixture Pair observer lifecycle is not create -> move -> stop -> destroy")
allowed = {"0x00b5", "0x00b7", "0x00ee"}
if any(events[i].get("opcode") not in allowed for i in movement_indices):
    raise SystemExit("Fixture Pair observer saw an unsupported movement opcode")
map_id = run.get("map_id")
for index in [create_index, *movement_indices]:
    event = events[index]
    if event.get("map_id") != map_id or not all(isinstance(event.get(key), (int, float)) and math.isfinite(event[key]) for key in ("east", "north", "elevation", "orientation")):
        raise SystemExit("Fixture Pair observer pose is incomplete or on another map")
submitted = run.get("submitted_stop", {})
stop = events[stop_index]
distance = math.hypot(stop["east"] - submitted.get("east", math.inf), stop["north"] - submitted.get("north", math.inf))
if distance > 0.25 or abs(stop["elevation"] - submitted.get("elevation", math.inf)) > 0.25:
    raise SystemExit("Fixture Pair terminal observer pose exceeds the 0.25m comparison contract")
anchor = run.get("mover_anchor", {})
move_distance = math.hypot(submitted.get("east", math.inf) - anchor.get("east", math.inf), submitted.get("north", math.inf) - anchor.get("north", math.inf))
if not 2.0 <= move_distance <= 4.0:
    raise SystemExit("Fixture Pair submitted move is outside the two-to-four-metre contract")
movement_times = [events[i]["received_after_ms"] for i in movement_indices]
cadences = [right - left for left, right in zip(movement_times, movement_times[1:])]
deltas = []
for left, right in zip([events[create_index], *(events[i] for i in movement_indices)], [*(events[i] for i in movement_indices), events[stop_index]]):
    deltas.append(math.hypot(right["east"] - left["east"], right["north"] - left["north"]))
output = {
    "schema": "miazcore.fixture-pair-timeline-sample.v1",
    "observer_guid": run["observer_guid"],
    "mover_guid": mover_guid,
    "move_distance_m": round(move_distance, 3),
    "terminal_pose_delta_m": round(distance, 3),
    "first_remote_movement_after_command_ms": movement_times[0] - timeline["move_start_after_ms"],
    "terminal_stop_after_move_start_ms": stop["received_after_ms"] - timeline["move_start_after_ms"],
    "destroy_after_logout_request_ms": events[destroy_index]["received_after_ms"] - timeline["logout_requested_after_ms"],
    "max_remote_update_cadence_ms": max(cadences, default=0),
    "max_remote_pose_delta_m": round(max(deltas, default=0.0), 3),
    "event_count": len(events),
}
if any(value < 0 for key, value in output.items() if key.endswith("_ms")):
    raise SystemExit("Fixture Pair observer record predates its shared-clock controller action")
serialized = json.dumps({"run": run, "events": events}, sort_keys=True).lower()
for forbidden in ("password", "account", "session", "cipher", "payload"):
    if forbidden in serialized:
        raise SystemExit(f"Fixture Pair timeline contains forbidden vocabulary: {forbidden}")
output_path.write_text(json.dumps(output, sort_keys=True) + "\n")
PY
done

stage="calibrate"
python3 - "$workspace" "$run" "$commit" "$sample_count" <<'PY'
import json, math, pathlib, sys

workspace, run, commit, expected_count = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2]), sys.argv[3], int(sys.argv[4])
samples = [json.loads(path.read_text()) for path in sorted(workspace.glob("sample-*.json"))]
if len(samples) != expected_count:
    raise SystemExit("Fixture Pair timeline did not retain every requested sample")
max_cadence = max(sample["max_remote_update_cadence_ms"] for sample in samples)
max_delta = max(sample["max_remote_pose_delta_m"] for sample in samples)
# Both margins are derived only from the observed Realm cadence: a future
# observer gets one additional complete cadence interval to see a terminal
# state, while a projected pose snaps beyond two observed update deltas.
summary = {
    "schema": "miazcore.fixture-pair-timeline.v1",
    "status": "passed",
    "commit": commit,
    "sample_count": len(samples),
    "samples": samples,
    "calibrated_contract": {
        "remote_pose_tolerance_m": 0.25,
        "first_remote_pose_deadline_ms": max(sample["first_remote_movement_after_command_ms"] for sample in samples) + max_cadence,
        "terminal_remote_pose_deadline_ms": max(sample["terminal_stop_after_move_start_ms"] for sample in samples) + max_cadence,
        "clean_logout_removal_deadline_ms": max(sample["destroy_after_logout_request_ms"] for sample in samples) + max_cadence,
        "remote_projection_snap_distance_m": math.ceil((2 * max_delta) * 1000) / 1000,
        "observed_max_update_cadence_ms": max_cadence,
    },
    "final_realm_health": "pending",
}
run.mkdir(parents=True, exist_ok=True)
(run / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
PY

stage="final-reset"
canonical_reset "$workspace/final-reset.marker"
stage="finalize"
python3 - "$run/summary.json" <<'PY'
import json, pathlib, sys
path = pathlib.Path(sys.argv[1])
summary = json.loads(path.read_text())
summary["final_realm_health"] = "passed"
path.write_text(json.dumps(summary, indent=2) + "\n")
PY
succeeded=true
echo "redacted Fixture Pair timeline retained: $run/summary.json"
