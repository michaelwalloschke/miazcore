#!/usr/bin/env bash
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
bundle="$(mktemp -d "${TMPDIR:-/tmp}/miazcore-acceptance.XXXXXX")"
cleanup() { rm -rf "$bundle"; }
trap cleanup EXIT INT TERM
mkdir -p "$bundle/artifacts"
candidate="0123456789abcdef0123456789abcdef01234567"
cat >"$bundle/artifacts/manual-attestation.json" <<JSON
{"candidate_sha":"$candidate","result":"PASS","checks":{"metal_and_diagnostic_world_visible":"PASS","phase_progression_and_pre_ready_input_gating":"PASS","orbit_zoom_focus_and_camera_relative_wasd":"PASS","smooth_heading_aligned_movement_without_height_drift":"PASS","rendered_submitted_realm_observed_diagnostics":"PASS","movement_proof_freeze_and_reconnect_evidence":"PASS","correction_and_visible_failure_presentation":"PASS","clean_disconnect_and_realm_health":"PASS"}}
JSON
printf '\211PNG\r\n\032\nmetal-placeholder\n' >"$bundle/artifacts/metal.png"
printf '{"phase":"Offline"}\n' >"$bundle/artifacts/metal.json"
printf '{"phase":"PersistedMovementCompared","movement_proof":{"passed":true}}\n' >"$bundle/artifacts/persisted-movement.json"
printf '{"phase":"PersistedMovementRejected"}\n' >"$bundle/artifacts/negative-short.json"
printf '{"phase":"ReconnectUnavailableRejected"}\n' >"$bundle/artifacts/negative-reconnect.json"
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
printf '{"phase":"Offline"}\n' >"$bundle/artifacts/metal.json"
python3 "$root/scripts/validate-acceptance-evidence.py" create "$bundle" "$candidate"
printf 'stale sidecar\n' >"$bundle/artifacts/negative-short.json"
if python3 "$root/scripts/validate-acceptance-evidence.py" validate "$bundle"; then
    echo "acceptance validator accepted stale semantic evidence" >&2
    exit 1
fi
printf '{"phase":"PersistedMovementRejected"}\n' >"$bundle/artifacts/negative-short.json"
python3 "$root/scripts/validate-acceptance-evidence.py" create "$bundle" "$candidate"
rm "$bundle/artifacts/metal.png"
if python3 "$root/scripts/validate-acceptance-evidence.py" validate "$bundle"; then
    echo "acceptance validator accepted a deleted artifact" >&2
    exit 1
fi
printf '\211PNG\r\n\032\nmetal-placeholder\n' >"$bundle/artifacts/metal.png"
printf '{"phase":"Offline"}\n' >"$bundle/artifacts/metal.json"
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
