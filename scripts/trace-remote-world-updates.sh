#!/usr/bin/env bash
set -euo pipefail

# A reset-scoped, local-only research probe. The trace adapter receives only
# complete decrypted frames and persists an allowlisted semantic summary; this
# script never copies credentials, session keys, packet payloads, or logs into
# repository artifacts.
export MIAZCORE_SKIP_PULL=1
export COMPOSE_PROGRESS=plain

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
realm="$root/infra/azerothcore/realm"
compose=(docker compose --project-directory "$root/infra/azerothcore" --file "$root/infra/azerothcore/compose.yaml" --project-name miazcore-reference-realm)
workspace="$(mktemp -d "${TMPDIR:-/tmp}/miazcore-remote-trace.XXXXXX")"
run_id="$(date -u +%Y%m%dT%H%M%SZ)-$(git -C "$root" rev-parse --short=12 HEAD)"
artifact_dir="$root/artifacts/remote-world-trace/$run_id"
peer_secret_root="$workspace/peer-secrets"
peer_account="MIAZPEER$(openssl rand -hex 4 | tr '[:lower:]' '[:upper:]')"
transcript_file="$workspace/transcript.json"
result_file="$workspace/result.json"
reset_complete=0

# A trace is evidence, not a developer convenience.  Refuse a dirty checkout
# so the retained result is unambiguously tied to the commit recorded below.
if [[ -n "$(git -C "$root" status --porcelain)" ]]; then
    echo "refusing to retain remote-world evidence from a dirty worktree" >&2
    exit 2
fi

cleanup() {
    local status="$?"
    rm -rf "$workspace"
    if [[ "$reset_complete" -eq 1 ]]; then
        "$realm" reset-state --yes >/dev/null
    fi
    return "$status"
}
trap cleanup EXIT INT TERM

database_query() {
    "${compose[@]}" exec -T database bash -ceu '
      export MYSQL_PWD="$(cat /run/secrets/database_root_password)"
      mysql --user=root --batch --skip-column-names "$@"
    ' -- "$@"
}

if [[ "${MIAZCORE_REMOTE_TRACE_SKIP_INITIAL_RESET:-0}" != 1 ]]; then
    "$realm" reset-state --yes >/dev/null
fi
# The optional skip is only for a known-clean Realm between the commit and its
# canonical measurement. Either path must restore the single-fixture state on
# exit, including a failed experiment after peer provisioning.
reset_complete=1
mkdir -p "$peer_secret_root"
chmod 700 "$peer_secret_root"
printf '%s\n' "$peer_account" >"$peer_secret_root/account"
cp "$root/infra/azerothcore/secrets/fixture-password" "$peer_secret_root/password"
chmod 600 "$peer_secret_root/account" "$peer_secret_root/password"

"${compose[@]}" run --rm --no-deps \
    --env "MIAZCORE_PEER_ACCOUNT=$peer_account" fixture-provisioner bash -ceu '
      password="$(< /run/secrets/fixture_password)"
      commands="$(mktemp)"
      trap "rm -f \"$commands\"" EXIT
      printf "account create %s %s\\nserver shutdown 1\\n" "$MIAZCORE_PEER_ACCOUNT" "$password" >"$commands"
      /miazcore/with-secrets.sh /azerothcore/env/dist/bin/worldserver <"$commands" >/dev/null 2>&1
      printf "pdump load /miazcore/fixtures/reference-character.pdump %s Miazpeer\\nserver shutdown 1\\n" "$MIAZCORE_PEER_ACCOUNT" >"$commands"
      /miazcore/with-secrets.sh /azerothcore/env/dist/bin/worldserver <"$commands" >/dev/null 2>&1
    ' >/dev/null

"$realm" health >/dev/null
cargo run --locked -p client_session --example trace_remote_world_updates -- \
    "$peer_secret_root" "$transcript_file" "$result_file"

python3 - "$transcript_file" "$result_file" <<'PY'
import json, pathlib, sys

transcript = json.loads(pathlib.Path(sys.argv[1]).read_text())
result = json.loads(pathlib.Path(sys.argv[2]).read_text())
if transcript.get("schema") != "miazcore.remote-transcript.v1":
    raise SystemExit("remote trace has an unexpected schema")
if result.get("schema") != "miazcore.remote-trace-run.v1":
    raise SystemExit("remote trace run has an unexpected schema")
events = transcript.get("events")
if not isinstance(events, list) or not events:
    raise SystemExit("remote trace has no semantic events")
peer_guid = result["peer_guid"]
peer = [event for event in events if event.get("guid") == peer_guid]
kinds = [event.get("kind") for event in peer]
try:
    create = kinds.index("create-object2")
    movement = kinds.index("movement", create + 1)
    destroy = kinds.index("destroy", movement + 1)
except ValueError as error:
    raise SystemExit("remote trace lacks peer create, movement, or destroy") from error
if not create < movement < destroy:
    raise SystemExit("remote trace lifecycle order is invalid")
movement_opcodes = {event.get("opcode") for event in peer if event.get("kind") == "movement"}
allowed_movement_opcodes = {"0x00b5", "0x00b7", "0x00ee"}
if not movement_opcodes or not movement_opcodes <= allowed_movement_opcodes:
    raise SystemExit(
        "remote trace has no supported peer movement opcode; observed semantic opcodes: "
        + ", ".join(sorted(str(opcode) for opcode in movement_opcodes))
    )
for event in peer:
    if event.get("kind") in {"create", "movement"}:
        if event.get("map_id") != result["map_id"]:
            raise SystemExit("remote trace map identity drifted")
        if not all(isinstance(event.get(key), (int, float)) for key in ("east", "north", "elevation", "orientation")):
            raise SystemExit("remote trace pose is incomplete")
serialized = json.dumps(transcript, sort_keys=True).lower()
for forbidden in ("password", "account", "session", "cipher", "payload"):
    if forbidden in serialized:
        raise SystemExit(f"remote trace contains forbidden vocabulary: {forbidden}")
PY

commit="$(git -C "$root" rev-parse HEAD)"
python3 - "$result_file" "$commit" <<'PY'
import json, pathlib, sys

path = pathlib.Path(sys.argv[1])
result = json.loads(path.read_text())
result["commit"] = sys.argv[2]
path.write_text(json.dumps(result, sort_keys=True) + "\n")
PY

mkdir -p "$artifact_dir"
cp "$transcript_file" "$artifact_dir/transcript.json"
cp "$result_file" "$artifact_dir/run.json"
chmod 600 "$artifact_dir/transcript.json" "$artifact_dir/run.json"
database_query acore_characters --execute="SELECT COUNT(*) FROM characters WHERE name = 'Miazpeer';" | grep -qx '1'
echo "redacted remote-world trace retained: $artifact_dir"
