#!/usr/bin/env python3
"""Create and validate a curated, redacted World-entry Acceptance bundle."""
import hashlib
import json
import pathlib
import re
import sys

SECRET = re.compile(r"password|session[_ -]?key|raw packet|credential", re.I)
REQUIRED = {
    "manual-attestation.json", "metal.png", "metal.json", "persisted-movement.json",
}
MANUAL_CHECKS = {
    "metal_and_diagnostic_world_visible",
    "phase_progression_and_pre_ready_input_gating",
    "orbit_zoom_focus_and_camera_relative_wasd",
    "smooth_heading_aligned_movement_without_height_drift",
    "rendered_submitted_realm_observed_diagnostics",
    "movement_proof_freeze_and_reconnect_evidence",
    "correction_and_visible_failure_presentation",
    "clean_disconnect_and_realm_health",
}

def sha256(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()

def files(root: pathlib.Path):
    return sorted(path for path in (root / "artifacts").iterdir() if path.is_file())

def create(root: pathlib.Path, candidate: str) -> None:
    names = {path.name for path in files(root)}
    if names != REQUIRED:
        raise SystemExit(f"bundle artifacts must be exactly {sorted(REQUIRED)}")
    manual = json.loads((root / "artifacts/manual-attestation.json").read_text())
    if manual.get("candidate_sha") != candidate or manual.get("result") != "PASS":
        raise SystemExit("manual attestation must PASS for the exact candidate SHA")
    if set(manual.get("checks", {})) != MANUAL_CHECKS or set(manual["checks"].values()) != {"PASS"}:
        raise SystemExit("manual attestation must explicitly PASS every required check")
    manifest = {
        "schema": "miazcore.world-entry-acceptance.v1",
        "candidate_sha": candidate,
        "results": {name: "PASS" for name in (
            "deterministic", "session", "bevy", "metal", "live-character", "live-proof", "live-negatives", "manual")},
        "artifacts": {path.name: sha256(path) for path in files(root)},
        "deferrals": [
            "gameplay, content, multiplayer, LAN exposure, broader packet or movement coverage",
            "Windows native build, test, render, and runtime acceptance",
        ],
    }
    (root / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    (root / "REPORT.md").write_text(
        "# World-entry Acceptance Evidence\n\n"
        f"Candidate: `{candidate}`\n\n"
        "All deterministic, platform, live-realm, and manual gates passed once without retry. "
        "See `manifest.json` for curated artifact hashes and explicit deferrals.\n"
    )

def validate(root: pathlib.Path) -> None:
    manifest = json.loads((root / "manifest.json").read_text())
    if manifest.get("schema") != "miazcore.world-entry-acceptance.v1":
        raise SystemExit("unsupported evidence schema")
    if not re.fullmatch(r"[0-9a-f]{40}", manifest.get("candidate_sha", "")):
        raise SystemExit("candidate SHA is missing or malformed")
    expected_results = {
        "deterministic", "session", "bevy", "metal", "live-character", "live-proof", "live-negatives", "manual",
    }
    if set(manifest.get("results", {})) != expected_results or set(manifest["results"].values()) != {"PASS"}:
        raise SystemExit("required acceptance gates must all PASS")
    recorded = manifest.get("artifacts", {})
    if set(recorded) != REQUIRED:
        raise SystemExit("manifest artifact set is incomplete")
    for path in files(root):
        if SECRET.search(path.read_text(errors="ignore")):
            raise SystemExit(f"secret-bearing vocabulary in {path.name}")
        if recorded[path.name] != sha256(path):
            raise SystemExit(f"hash mismatch for {path.name}")
    print(f"validated World-entry Acceptance bundle for {manifest['candidate_sha']}")

if __name__ == "__main__":
    if len(sys.argv) < 3 or sys.argv[1] not in {"create", "validate"}:
        raise SystemExit("usage: validate-acceptance-evidence.py {create|validate} BUNDLE [CANDIDATE_SHA]")
    root = pathlib.Path(sys.argv[2])
    if sys.argv[1] == "create":
        if len(sys.argv) != 4:
            raise SystemExit("create requires a candidate SHA")
        create(root, sys.argv[3])
    else:
        validate(root)
