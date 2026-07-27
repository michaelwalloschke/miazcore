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

python3 "$root/scripts/validate-acceptance-evidence.py" create "$bundle" "$candidate"
python3 "$root/scripts/validate-acceptance-evidence.py" validate "$bundle"
printf 'tampered\n' >>"$bundle/artifacts/metal.json"
if python3 "$root/scripts/validate-acceptance-evidence.py" validate "$bundle"; then
    echo "acceptance validator accepted a tampered artifact" >&2
    exit 1
fi
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
