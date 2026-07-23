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
printf 'metal-placeholder\n' >"$bundle/artifacts/metal.png"
printf '{"phase":"MovementReady"}\n' >"$bundle/artifacts/metal.json"
printf '{"phase":"PersistedMovementCompared"}\n' >"$bundle/artifacts/persisted-movement.json"

python3 "$root/scripts/validate-acceptance-evidence.py" create "$bundle" "$candidate"
python3 "$root/scripts/validate-acceptance-evidence.py" validate "$bundle"
printf 'tampered\n' >>"$bundle/artifacts/metal.json"
if python3 "$root/scripts/validate-acceptance-evidence.py" validate "$bundle"; then
    echo "acceptance validator accepted a tampered artifact" >&2
    exit 1
fi
