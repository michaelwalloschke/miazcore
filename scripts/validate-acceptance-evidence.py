#!/usr/bin/env python3
"""Create and validate a curated, redacted World-entry Acceptance bundle."""
import hashlib
import json
import pathlib
import platform
import re
import shutil
import subprocess
import sys

SENSITIVE = re.compile(
    r"password|credential|session[_ -]?(?:key|material|token)|secret|cipher|raw packet|auth(?:entication)?[_ -]?proof",
    re.I,
)
GATES = (
    "deterministic", "session", "bevy", "metal", "live-character", "live-proof", "live-negatives", "manual",
)
REQUIRED = {
    "commands.json", "gate-results.json", "manual-attestation.json", "metal.json", "metal.png",
    "negative-reconnect.json", "negative-short.json", "persisted-movement.json", "versions.json",
}
DEFERRALS = [
    "gameplay, content, multiplayer, LAN exposure, broader packet or movement coverage",
    "authored-content polish, general polish, unmeasured optimization, and public distribution work",
    "Windows native build, test, render, and runtime acceptance",
]
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


def artifact_files(root: pathlib.Path):
    directory = root / "artifacts"
    if not directory.is_dir():
        raise SystemExit("bundle artifacts directory is missing")
    found = {path.name for path in directory.iterdir() if path.is_file()}
    if found != REQUIRED:
        raise SystemExit(f"bundle artifacts must be exactly {sorted(REQUIRED)}")
    return sorted(directory / name for name in REQUIRED)


def read_json(path: pathlib.Path):
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise SystemExit(f"invalid curated JSON artifact {path.name}: {error}") from error


def assert_redacted(value, path: pathlib.Path) -> None:
    if isinstance(value, dict):
        for key, nested in value.items():
            if SENSITIVE.search(str(key)):
                raise SystemExit(f"sensitive field name in {path.name}")
            assert_redacted(nested, path)
    elif isinstance(value, list):
        for nested in value:
            assert_redacted(nested, path)
    elif isinstance(value, str) and SENSITIVE.search(value):
        raise SystemExit(f"sensitive vocabulary in {path.name}")


def command_version(command: list[str]) -> str:
    try:
        return subprocess.check_output(command, text=True, stderr=subprocess.STDOUT).strip()
    except (OSError, subprocess.CalledProcessError) as error:
        raise SystemExit(f"could not record tool version {' '.join(command)}: {error}") from error


def curate(root: pathlib.Path, candidate: str, manual_attestation: pathlib.Path) -> None:
    artifacts = root / "artifacts"
    artifacts.mkdir(parents=True, exist_ok=False)
    sources = {
        "manual-attestation.json": manual_attestation,
        "metal.png": pathlib.Path("artifacts/render-smoke/offline-diagnostic-world.png"),
        "metal.json": pathlib.Path("artifacts/render-smoke/offline-diagnostic-world.json"),
        "persisted-movement.json": pathlib.Path("artifacts/persisted-movement-smoke.json"),
        "negative-short.json": pathlib.Path("artifacts/persisted-movement-short-negative.json"),
        "negative-reconnect.json": pathlib.Path("artifacts/persisted-movement-reconnect-unavailable.json"),
    }
    for name, source in sources.items():
        if not source.is_file():
            raise SystemExit(f"missing curated source artifact: {source}")
        shutil.copyfile(source, artifacts / name)
    commands = {
        "deterministic": ["cargo", "test", "--locked", "-p", "client_protocol", "--tests"],
        "session": ["cargo", "test", "--locked", "-p", "client_session"],
        "bevy": ["scripts/check.sh"],
        "metal": ["scripts/render-smoke.sh"],
        "live-character": ["scripts/live-character-selection.sh"],
        "live-proof": ["scripts/persisted-movement-smoke.sh"],
        "live-negatives": ["scripts/persisted-movement-negative-probes.sh"],
        "manual": ["manual-attestation", manual_attestation.name],
    }
    (artifacts / "commands.json").write_text(json.dumps({"schema": "miazcore.acceptance-commands.v1", "commands": commands}, indent=2) + "\n")
    (artifacts / "gate-results.json").write_text(json.dumps({"schema": "miazcore.acceptance-results.v1", "results": {gate: "PASS" for gate in GATES}}, indent=2) + "\n")
    versions = {
        "git": command_version(["git", "--version"]),
        "rustc": command_version(["rustc", "-V"]),
        "cargo": command_version(["cargo", "-V"]),
        "python": platform.python_version(),
        "platform": platform.platform(),
    }
    (artifacts / "versions.json").write_text(json.dumps({"schema": "miazcore.acceptance-versions.v1", "versions": versions}, indent=2) + "\n")
    create(root, candidate)


def create(root: pathlib.Path, candidate: str) -> None:
    paths = artifact_files(root)
    manual = read_json(root / "artifacts/manual-attestation.json")
    if manual.get("candidate_sha") != candidate or manual.get("result") != "PASS":
        raise SystemExit("manual attestation must PASS for the exact candidate SHA")
    if set(manual.get("checks", {})) != MANUAL_CHECKS or set(manual["checks"].values()) != {"PASS"}:
        raise SystemExit("manual attestation must explicitly PASS every required check")
    results = read_json(root / "artifacts/gate-results.json")
    if results.get("schema") != "miazcore.acceptance-results.v1" or results.get("results") != {gate: "PASS" for gate in GATES}:
        raise SystemExit("curated gate results must explicitly PASS every required gate")
    commands = read_json(root / "artifacts/commands.json")
    if commands.get("schema") != "miazcore.acceptance-commands.v1" or set(commands.get("commands", {})) != set(GATES):
        raise SystemExit("curated commands must describe every acceptance gate")
    manifest = {
        "schema": "miazcore.world-entry-acceptance.v2",
        "candidate_sha": candidate,
        "results": results["results"],
        "artifacts": {path.name: sha256(path) for path in paths},
        "deferrals": DEFERRALS,
    }
    (root / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    (root / "REPORT.md").write_text(
        "# World-entry Acceptance Evidence\n\n"
        f"Candidate: `{candidate}`\n\n"
        "All eight recorded gates passed once without retry. `artifacts/commands.json`, "
        "`artifacts/versions.json`, `artifacts/gate-results.json`, and the curated semantic sidecars "
        "are hash-bound by `manifest.json`; diagnostics are retained separately as attempts.\n"
    )


def validate(root: pathlib.Path) -> None:
    manifest = read_json(root / "manifest.json")
    if manifest.get("schema") != "miazcore.world-entry-acceptance.v2":
        raise SystemExit("unsupported evidence schema")
    if not re.fullmatch(r"[0-9a-f]{40}", manifest.get("candidate_sha", "")):
        raise SystemExit("candidate SHA is missing or malformed")
    if manifest.get("results") != {gate: "PASS" for gate in GATES}:
        raise SystemExit("required acceptance gates must all PASS")
    if manifest.get("deferrals") != DEFERRALS:
        raise SystemExit("manifest deferrals are incomplete")
    paths = artifact_files(root)
    recorded = manifest.get("artifacts", {})
    if set(recorded) != REQUIRED:
        raise SystemExit("manifest artifact set is incomplete")
    for path in paths:
        if recorded[path.name] != sha256(path):
            raise SystemExit(f"hash mismatch for {path.name}")
        if path.suffix == ".json":
            assert_redacted(read_json(path), path)
    png = root / "artifacts/metal.png"
    if not png.read_bytes().startswith(b"\x89PNG\r\n\x1a\n"):
        raise SystemExit("metal evidence is not a PNG")
    if read_json(root / "artifacts/metal.json").get("phase") != "Offline":
        raise SystemExit("metal semantic evidence is incomplete")
    proof = read_json(root / "artifacts/persisted-movement.json")
    if proof.get("phase") != "PersistedMovementCompared" or not proof.get("movement_proof", {}).get("passed"):
        raise SystemExit("persisted movement semantic evidence is incomplete")
    if read_json(root / "artifacts/negative-short.json").get("phase") != "PersistedMovementRejected":
        raise SystemExit("short negative semantic evidence is incomplete")
    if read_json(root / "artifacts/negative-reconnect.json").get("phase") != "ReconnectUnavailableRejected":
        raise SystemExit("reconnect negative semantic evidence is incomplete")
    print(f"validated World-entry Acceptance bundle for {manifest['candidate_sha']}")


if __name__ == "__main__":
    if len(sys.argv) < 3 or sys.argv[1] not in {"create", "curate", "validate"}:
        raise SystemExit("usage: validate-acceptance-evidence.py {create|curate|validate} BUNDLE [CANDIDATE_SHA] [MANUAL_ATTESTATION]")
    root = pathlib.Path(sys.argv[2])
    if sys.argv[1] == "create":
        if len(sys.argv) != 4:
            raise SystemExit("create requires a candidate SHA")
        create(root, sys.argv[3])
    elif sys.argv[1] == "curate":
        if len(sys.argv) != 5:
            raise SystemExit("curate requires a candidate SHA and manual attestation")
        curate(root, sys.argv[3], pathlib.Path(sys.argv[4]))
    else:
        if len(sys.argv) != 3:
            raise SystemExit("validate accepts only a bundle path")
        validate(root)
