#!/usr/bin/env bash
set -euo pipefail

# The locked images are already part of the local Reference Realm prerequisite.
# A measurement must not turn a reset-scoped local proof into a network pull.
export MIAZCORE_SKIP_PULL=1

# Research-only, reset-scoped measurement for Ticket 01. It creates an ignored
# temporary peer account from the existing fixture password and records only
# sanitized Realm facts. A failed run deliberately leaves the Realm untouched:
# otherwise the expensive reset/import would hide the actual failing stage.
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
realm="$root/infra/azerothcore/realm"
# Keep the one-off fixture provisioner and database queries inside the exact
# project that `infra/azerothcore/realm` reset/start commands manage.
compose=(docker compose --project-directory "$root/infra/azerothcore" --file "$root/infra/azerothcore/compose.yaml" --project-name miazcore-reference-realm)
workspace="$(mktemp -d "${TMPDIR:-/tmp}/miazcore-multi-session.XXXXXX")"
run_id="$(date -u +%Y%m%dT%H%M%SZ)-$(git -C "$root" rev-parse --short=12 HEAD)"
artifact_dir="$root/artifacts/multi-session-research/$run_id"
peer_secret_root="$workspace/peer-secrets"
peer_account="MIAZPEER$(openssl rand -hex 4 | tr '[:lower:]' '[:upper:]')"
ready_file="$workspace/ready.json"
primary_stop="$workspace/primary-stop"
peer_stop="$workspace/peer-stop"
duplicate_file="$workspace/duplicate.json"
measurement_file="$workspace/measurement.json"
client_log="$workspace/client.log"
realm_log="$workspace/realm.log"
provision_log="$workspace/provision.log"
client_pid=""
stage="initialization"

sanitize_log() {
    # The temporary account name is not a credential, but retaining it makes
    # diagnostic artifacts needlessly correlateable between runs. The command
    # output is otherwise intentionally limited to project-owned diagnostics.
    sed -E 's/MIAZPEER[[:xdigit:]]+/TEMPORARY_PEER/g' "$1"
}

append_failure_summary() {
    local log="$1"
    # Do not retain arbitrary runtime logs: they could grow a new secret or
    # packet format later. These allowlisted, already-redacted failure/status
    # forms are sufficient to identify the stage without copying traffic.
    grep -E 'failed|Failed|timed out|did not reach|ERROR|error:|health:' "$log" \
        | head -n 80 \
        | sanitize_log /dev/stdin || true
}

preserve_failure_diagnostics() {
    mkdir -p "$artifact_dir"
    {
        printf 'schema=miazcore.multi-session-research-failure.v1\n'
        printf 'stage=%s\n' "$stage"
        printf 'exit_status=%s\n' "$1"
        printf 'run_id=%s\n' "$run_id"
        printf 'commit=%s\n' "$(git -C "$root" rev-parse HEAD)"
        printf '\n[realm-health]\n'
        "$realm" health || true
        printf '\n[compose-ps]\n'
        "${compose[@]}" ps --all || true
        for log in "$realm_log" "$provision_log" "$client_log"; do
            if [[ -f "$log" ]]; then
                printf '\n[%s failure-summary]\n' "$(basename "$log")"
                append_failure_summary "$log"
            fi
        done
    } >"$artifact_dir/diagnostics.txt" 2>&1
    chmod 600 "$artifact_dir/diagnostics.txt"
    echo "multi-session research failed during: $stage" >&2
    echo "sanitized diagnostics retained: $artifact_dir/diagnostics.txt" >&2
}

cleanup() {
    local status="$?"
    if [[ -n "$client_pid" ]]; then
        kill "$client_pid" 2>/dev/null || true
        wait "$client_pid" 2>/dev/null || true
    fi
    if [[ "$status" -ne 0 ]]; then
        preserve_failure_diagnostics "$status"
    fi
    # The workspace contains the temporary credential copy. Never retain it,
    # even on failure; the retained diagnostic is sanitized above.
    rm -rf "$workspace"
    return "$status"
}
trap cleanup EXIT INT TERM

wait_for_file() {
    local path="$1" description="$2"
    for _ in {1..300}; do
        [[ -f "$path" ]] && return 0
        sleep 0.1
    done
    echo "multi-session research timed out waiting for $description" >&2
    return 1
}

run_realm() {
    if ! "$realm" "$@" >"$realm_log" 2>&1; then
        echo "multi-session research Realm operation failed: $1" >&2
        return 1
    fi
}

database_query() {
    "${compose[@]}" exec -T database bash -ceu '
      export MYSQL_PWD="$(cat /run/secrets/database_root_password)"
      mysql --user=root --batch --skip-column-names "$@"
    ' -- "$@"
}

online_state() {
    database_query acore_characters --execute="
      SELECT GROUP_CONCAT(CONCAT(name, ':', online) ORDER BY name SEPARATOR ',')
      FROM characters WHERE name IN ('Miaztest', 'Miazpeer');"
}

wait_for_online_state() {
    local expected="$1" description="$2" attempts="${3:-240}"
    for ((attempt = 0; attempt < attempts; attempt++)); do
        [[ "$(online_state)" == "$expected" ]] && return 0
        sleep 1
    done
    echo "multi-session research timed out waiting for $description" >&2
    return 1
}

measure_logout_settlement() {
    local primary_started="$1" peer_started="$2" output="$3"
    local primary_seconds="" peer_seconds="" observed now
    # A common bounded poll preserves both independent timing measurements
    # without keeping either client session alive while the other persists.
    for ((attempt = 0; attempt < 240; attempt++)); do
        observed="$(online_state)"
        now="$(date +%s)"
        [[ -n "$primary_seconds" || "$observed" != *'Miaztest:0'* ]] || primary_seconds="$((now - primary_started))"
        [[ -n "$peer_seconds" || "$observed" != *'Miazpeer:0'* ]] || peer_seconds="$((now - peer_started))"
        if [[ -n "$primary_seconds" && -n "$peer_seconds" ]]; then
            python3 - "$output" "$primary_seconds" "$peer_seconds" "$observed" <<'PY'
import json, pathlib, sys
path, primary, peer, final_state = sys.argv[1:]
pathlib.Path(path).write_text(json.dumps({
    "primary_seconds": int(primary),
    "peer_seconds": int(peer),
    "final_online_state": final_state,
}, sort_keys=True) + "\n")
PY
            return 0
        fi
        sleep 1
    done
    echo "multi-session research timed out waiting for both logout settlements; last state: $observed" >&2
    return 1
}

stage="resetting the Reference Realm"
if [[ "${MIAZCORE_MEASUREMENT_SKIP_INITIAL_RESET:-0}" != 1 ]]; then
    run_realm reset-state --yes
fi
mkdir -p "$peer_secret_root"
chmod 700 "$peer_secret_root"
printf '%s\n' "$peer_account" >"$peer_secret_root/account"
cp "$root/infra/azerothcore/secrets/fixture-password" "$peer_secret_root/password"
chmod 600 "$peer_secret_root/account" "$peer_secret_root/password"

stage="provisioning the disposable measurement peer"
"${compose[@]}" run --rm --no-deps \
    --env "MIAZCORE_PEER_ACCOUNT=$peer_account" fixture-provisioner bash -ceu '
      password="$(< /run/secrets/fixture_password)"
      commands="$(mktemp)"
      trap "rm -f \"$commands\"" EXIT
      printf "account create %s %s\\nserver shutdown 1\\n" "$MIAZCORE_PEER_ACCOUNT" "$password" >"$commands"
      /miazcore/with-secrets.sh /azerothcore/env/dist/bin/worldserver <"$commands" >/dev/null 2>&1
      printf "pdump load /miazcore/fixtures/reference-character.pdump %s Miazpeer\\nserver shutdown 1\\n" "$MIAZCORE_PEER_ACCOUNT" >"$commands"
      /miazcore/with-secrets.sh /azerothcore/env/dist/bin/worldserver <"$commands" >/dev/null 2>&1
    ' >"$provision_log" 2>&1

stage="checking the Reference Realm after peer provisioning"
# Authentication reads the newly created account through authserver. The
# already-running Worldserver receives that authenticated account id during
# world authentication, so no server restart is needed here; avoiding one
# keeps the successful two-session observation isolated from orchestration
# churn.
run_realm health

stage="bringing both independent sessions to MovementReady"
cargo run --locked -p client_session --example measure_multi_session -- \
    pair "$peer_secret_root" "$ready_file" "$primary_stop" "$peer_stop" >"$client_log" 2>&1 &
client_pid=$!
stage="observing concurrent online state"
wait_for_file "$ready_file" "both sessions to reach MovementReady"
wait_for_online_state 'Miazpeer:1,Miaztest:1' 'both Characters online'
ready_json="$(<"$ready_file")"
positions="$(database_query acore_characters --execute="
  SELECT CONCAT(name, ':', map, ':', ROUND(position_x, 3), ':', ROUND(position_y, 3), ':', ROUND(position_z, 3))
  FROM characters WHERE name IN ('Miaztest', 'Miazpeer') ORDER BY name;")"
stage="observing primary clean logout"
printf 'stop\n' >"$primary_stop"
wait_for_file "${primary_stop}.observed" "primary clean disconnect"
primary_started="$(date +%s)"
online_after_primary_disconnect="$(online_state)"
stage="observing peer clean logout"
printf 'stop\n' >"$peer_stop"
wait_for_file "${peer_stop}.observed" "peer clean disconnect"
peer_started="$(date +%s)"
wait "$client_pid"
client_pid=""
settlement_file="$workspace/logout-settlement.json"
stage="measuring asynchronous logout settlement"
measure_logout_settlement "$primary_started" "$peer_started" "$settlement_file"
logout_settlement="$(<"$settlement_file")"

stage="measuring duplicate fixture behavior"
cargo run --locked -p client_session --example measure_multi_session -- duplicate "$duplicate_file" >>"$client_log" 2>&1
duplicate_json="$(<"$duplicate_file")"
wait_for_online_state 'Miazpeer:0,Miaztest:0' 'duplicate session cleanup'

python3 - "$measurement_file" "$ready_json" "$positions" "$online_after_primary_disconnect" "$logout_settlement" "$duplicate_json" <<'PY'
import json, pathlib, sys
output, ready, positions, after_primary, settlement, duplicate = map(str, sys.argv[1:])
ready_data = json.loads(ready)
duplicate_data = json.loads(duplicate)
settlement_data = json.loads(settlement)
records = []
for line in positions.splitlines():
    name, map_id, east, north, elevation = line.split(":")
    records.append({"name": name, "map_id": int(map_id), "east": float(east), "north": float(north), "elevation": float(elevation)})
pathlib.Path(output).write_text(json.dumps({
    "schema": "miazcore.multi-session-research.v1",
    "pair_ready": ready_data,
    "server_positions": records,
    "online_state_after_primary_disconnect": after_primary,
    "logout_settlement": settlement_data,
    "duplicate_fixture_outcome": duplicate_data,
}, indent=2) + "\n")
PY

stage="validating the redacted measurement"
python3 - "$measurement_file" <<'PY'
import json, pathlib, sys
data = json.loads(pathlib.Path(sys.argv[1]).read_text())
if data["pair_ready"]["phase"] != "both-ready" or not data["pair_ready"]["same_map"]:
    raise SystemExit("multi-session research did not establish a same-map pair")
if len(data["server_positions"]) != 2:
    raise SystemExit("multi-session research did not observe both Characters")
settlement = data["logout_settlement"]
if settlement["final_online_state"] != "Miazpeer:0,Miaztest:0":
    raise SystemExit("multi-session research did not observe both Characters offline")
if max(settlement["primary_seconds"], settlement["peer_seconds"]) > 240:
    raise SystemExit("multi-session research exceeded the logout settlement budget")
print(json.dumps(data, indent=2))
PY

stage="retaining redacted successful measurement"
mkdir -p "$artifact_dir"
cp "$measurement_file" "$artifact_dir/measurement.json"
chmod 600 "$artifact_dir/measurement.json"
echo "redacted measurement retained: $artifact_dir/measurement.json"

# The temporary peer has served its one-off research purpose. Restore the
# standard single-fixture Realm only after the evidence is safely retained;
# failures above intentionally never enter this destructive success path.
stage="restoring the standard Reference Realm after successful measurement"
run_realm reset-state --yes
