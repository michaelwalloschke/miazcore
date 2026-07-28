#!/usr/bin/env bash
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
bundle="$(mktemp -d "${TMPDIR:-/tmp}/miazcore-acceptance.XXXXXX")"
cleanup() { rm -rf "$bundle"; }
trap cleanup EXIT INT TERM
mkdir -p "$bundle/artifacts"
candidate="0123456789abcdef0123456789abcdef01234567"
cat >"$bundle/artifacts/manual-attestation.json" <<JSON
{"schema":"miazcore.world-entry-manual-attestation.v1","candidate_sha":"$candidate","result":"PASS","host":"test host","checks":{"metal_and_diagnostic_world_visible":"PASS","phase_progression_and_pre_ready_input_gating":"PASS","orbit_zoom_focus_and_camera_relative_wasd":"PASS","smooth_heading_aligned_movement_without_height_drift":"PASS","rendered_submitted_realm_observed_diagnostics":"PASS","movement_proof_freeze_and_reconnect_evidence":"PASS","correction_and_visible_failure_presentation":"PASS","clean_disconnect_and_realm_health":"PASS"},"notes":"manual diagnostic review passed"}
JSON
printf '\211PNG\r\n\032\nmetal-placeholder\n' >"$bundle/artifacts/metal.png"
write_valid_sidecars() {
    printf '%s\n' '{"schema":"miazcore.render-proof.v1","phase":"Offline","network":"disabled","realm_id":1,"client_build":12340,"character":"Test","rendered_pose":{"space":"offline-display","east":2.4,"north":-1.6,"elevation":0.0},"submitted_pose":null,"realm_observed_pose":null}' >"$bundle/artifacts/metal.json"
    printf '%s\n' '{"schema":"miazcore.live-render-proof.v1","phase":"PersistedMovementCompared","network":"reference-realm","realm_id":1,"client_build":12340,"character":"Test","run_speed":7.0,"movement_publication":"bounded-ground","entry_anchor":{"map_id":0,"east":1.0,"north":2.0,"elevation":3.0,"orientation":0.0},"predicted_pose":{"map_id":0,"east":3.5,"north":2.0,"elevation":3.0,"orientation":1.0},"rendered_pose":{"map_id":0,"east":3.5,"north":2.0,"elevation":3.0,"orientation":1.0},"submitted_pose":{"map_id":0,"east":3.5,"north":2.0,"elevation":3.0,"orientation":1.0},"realm_observed_pose":{"map_id":0,"east":3.5,"north":2.0,"elevation":3.0,"orientation":1.0},"failure_context":null,"movement_proof":{"source":"fresh-reconnect-login-verify-world","expected":{"map_id":0,"east":3.5,"north":2.0,"elevation":3.0,"orientation":1.0},"observed":{"map_id":0,"east":3.5,"north":2.0,"elevation":3.0,"orientation":1.0},"delta_metres":0.0,"tolerance_metres":0.25,"passed":true}}' >"$bundle/artifacts/persisted-movement.json"
    printf '%s\n' '{"schema":"miazcore.live-render-proof.v1","phase":"PersistedMovementRejected","network":"reference-realm","realm_id":1,"client_build":12340,"character":"Test","run_speed":7.0,"movement_publication":"bounded-ground","entry_anchor":{"map_id":0,"east":1.0,"north":2.0,"elevation":3.0,"orientation":0.0},"predicted_pose":{"map_id":0,"east":1.0,"north":2.0,"elevation":3.0,"orientation":0.0},"rendered_pose":{"map_id":0,"east":1.0,"north":2.0,"elevation":3.0,"orientation":0.0},"submitted_pose":{"map_id":0,"east":1.0,"north":2.0,"elevation":3.0,"orientation":0.0},"realm_observed_pose":{"map_id":0,"east":1.0,"north":2.0,"elevation":3.0,"orientation":0.0},"failure_context":"movement proof requires a submitted stopped pose","movement_proof":null}' >"$bundle/artifacts/negative-short.json"
    printf '%s\n' '{"schema":"miazcore.persisted-movement-negative-probe.v1","phase":"ReconnectUnavailableRejected","network":"reference-realm","oracle":"client-reconnect-failure","database_derived_success":false}' >"$bundle/artifacts/negative-reconnect.json"
}
write_valid_sidecars
printf '{"schema":"miazcore.acceptance-commands.v1","commands":{"deterministic":["cargo"],"session":["cargo"],"bevy":["scripts/check.sh"],"metal":["scripts/render-smoke.sh"],"live-character":["scripts/live-character-selection.sh"],"live-proof":["scripts/persisted-movement-smoke.sh"],"live-negatives":["scripts/persisted-movement-negative-probes.sh"],"manual":["manual-attestation"]}}\n' >"$bundle/artifacts/commands.json"
printf '{"schema":"miazcore.acceptance-results.v1","results":{"deterministic":"PASS","session":"PASS","bevy":"PASS","metal":"PASS","live-character":"PASS","live-proof":"PASS","live-negatives":"PASS","manual":"PASS"}}\n' >"$bundle/artifacts/gate-results.json"
printf '{"schema":"miazcore.acceptance-versions.v1","versions":{"git":"test","rustc":"test","cargo":"test","python":"test","platform":"test"}}\n' >"$bundle/artifacts/versions.json"
for gate in deterministic session bevy metal live-character live-proof live-negatives; do
    printf 'PASS\n' >"$bundle/artifacts/$gate.result"
done
python3 - "$bundle" "$candidate" <<'PY'
import hashlib, json, pathlib, sys
root = pathlib.Path(sys.argv[1]) / "artifacts"
candidate = sys.argv[2]
gates = ("deterministic", "session", "bevy", "metal", "live-character", "live-proof", "live-negatives")
hashes = {gate: hashlib.sha256((root / f"{gate}.result").read_bytes()).hexdigest() for gate in gates}
hashes["manual"] = hashlib.sha256((root / "manual-attestation.json").read_bytes()).hexdigest()
evidence = ("metal.png", "metal.json", "persisted-movement.json", "negative-short.json", "negative-reconnect.json")
evidence_hashes = {name: hashlib.sha256((root / name).read_bytes()).hexdigest() for name in evidence}
(root / "execution.json").write_text(json.dumps({"schema": "miazcore.acceptance-execution.v1", "candidate_sha": candidate, "attempt_id": "test-attempt", "gate_result_hashes": hashes, "gate_evidence_hashes": evidence_hashes}) + "\n")
PY

python3 "$root/scripts/validate-acceptance-evidence.py" create "$bundle" "$candidate"
python3 "$root/scripts/validate-acceptance-evidence.py" validate "$bundle"
attempt="$bundle/attempt"
curated="$bundle/curated"
mkdir -p "$attempt/sidecars"
printf '%s\n' "$candidate" >"$attempt/candidate_sha"
for gate in deterministic session bevy metal live-character live-proof live-negatives; do
    printf 'PASS\n' >"$attempt/$gate.result"
done
cp "$bundle/artifacts/metal.png" "$attempt/sidecars/metal.png"
cp "$bundle/artifacts/metal.json" "$attempt/sidecars/metal.json"
cp "$bundle/artifacts/persisted-movement.json" "$attempt/sidecars/persisted-movement.json"
cp "$bundle/artifacts/negative-short.json" "$attempt/sidecars/negative-short.json"
cp "$bundle/artifacts/negative-reconnect.json" "$attempt/sidecars/negative-reconnect.json"
python3 "$root/scripts/validate-acceptance-evidence.py" curate "$curated" "$candidate" "$bundle/artifacts/manual-attestation.json" "$attempt"
python3 "$root/scripts/validate-acceptance-evidence.py" validate "$curated"
printf 'FAIL\n' >"$bundle/artifacts/live-proof.result"
if python3 "$root/scripts/validate-acceptance-evidence.py" validate "$bundle"; then
    echo "acceptance validator accepted a changed retained gate result" >&2
    exit 1
fi
printf 'PASS\n' >"$bundle/artifacts/live-proof.result"
python3 "$root/scripts/validate-acceptance-evidence.py" create "$bundle" "$candidate"
printf 'tampered\n' >>"$bundle/artifacts/metal.json"
if python3 "$root/scripts/validate-acceptance-evidence.py" validate "$bundle"; then
    echo "acceptance validator accepted a tampered artifact" >&2
    exit 1
fi
printf '{"phase":"Offline","opaque":"not allowed"}\n' >"$bundle/artifacts/metal.json"
if python3 "$root/scripts/validate-acceptance-evidence.py" create "$bundle" "$candidate"; then
    echo "acceptance validator accepted an unallowlisted curated field" >&2
    exit 1
fi
write_valid_sidecars
python3 "$root/scripts/validate-acceptance-evidence.py" create "$bundle" "$candidate"
mkdir "$bundle/artifacts/unexpected-directory"
if python3 "$root/scripts/validate-acceptance-evidence.py" create "$bundle" "$candidate"; then
    echo "acceptance validator accepted an unexpected artifact directory" >&2
    exit 1
fi
rmdir "$bundle/artifacts/unexpected-directory"
python3 - "$bundle/artifacts/persisted-movement.json" <<'PY'
import json, pathlib, sys
path = pathlib.Path(sys.argv[1])
data = json.loads(path.read_text())
del data["movement_proof"]["expected"]
path.write_text(json.dumps(data) + "\n")
PY
if python3 "$root/scripts/validate-acceptance-evidence.py" create "$bundle" "$candidate"; then
    echo "acceptance validator accepted incomplete movement proof evidence" >&2
    exit 1
fi
write_valid_sidecars
python3 "$root/scripts/validate-acceptance-evidence.py" create "$bundle" "$candidate"
printf 'stale sidecar\n' >"$bundle/artifacts/negative-short.json"
if python3 "$root/scripts/validate-acceptance-evidence.py" validate "$bundle"; then
    echo "acceptance validator accepted stale semantic evidence" >&2
    exit 1
fi
write_valid_sidecars
python3 "$root/scripts/validate-acceptance-evidence.py" create "$bundle" "$candidate"
rm "$bundle/artifacts/metal.png"
if python3 "$root/scripts/validate-acceptance-evidence.py" validate "$bundle"; then
    echo "acceptance validator accepted a deleted artifact" >&2
    exit 1
fi
printf '\211PNG\r\n\032\nmetal-placeholder\n' >"$bundle/artifacts/metal.png"
write_valid_sidecars
python3 "$root/scripts/validate-acceptance-evidence.py" create "$bundle" "$candidate"
python3 - "$bundle/artifacts/manual-attestation.json" <<'PY'
import json, pathlib, sys
path = pathlib.Path(sys.argv[1])
data = json.loads(path.read_text())
data["notes"] = "QUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVowMTIzNDU2Nzg5QUJDREVGR0g="
path.write_text(json.dumps(data) + "\n")
PY
if python3 "$root/scripts/validate-acceptance-evidence.py" validate "$bundle"; then
    echo "acceptance validator accepted encoded material in manual evidence" >&2
    exit 1
fi
cat >"$bundle/artifacts/manual-attestation.json" <<JSON
{"schema":"miazcore.world-entry-manual-attestation.v1","candidate_sha":"$candidate","result":"PASS","host":"test host","checks":{"metal_and_diagnostic_world_visible":"PASS","phase_progression_and_pre_ready_input_gating":"PASS","orbit_zoom_focus_and_camera_relative_wasd":"PASS","smooth_heading_aligned_movement_without_height_drift":"PASS","rendered_submitted_realm_observed_diagnostics":"PASS","movement_proof_freeze_and_reconnect_evidence":"PASS","correction_and_visible_failure_presentation":"PASS","clean_disconnect_and_realm_health":"PASS"},"notes":"manual diagnostic review passed"}
JSON
python3 "$root/scripts/validate-acceptance-evidence.py" create "$bundle" "$candidate"
python3 - "$bundle/artifacts/versions.json" <<'PY'
import json, pathlib, sys
path = pathlib.Path(sys.argv[1])
data = json.loads(path.read_text())
data["versions"]["cargo"] = "session key should never be curated"
path.write_text(json.dumps(data) + "\n")
PY
if python3 "$root/scripts/validate-acceptance-evidence.py" validate "$bundle"; then
    echo "acceptance validator accepted sensitive curated data" >&2
    exit 1
fi
