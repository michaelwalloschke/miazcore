#!/usr/bin/env python3
"""Create and validate a curated, redacted World-entry Acceptance bundle."""
import hashlib
import json
import math
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
RAW_BLOB = re.compile(r"(?:[0-9a-f]{64,}|[A-Za-z0-9+/]{64,}={0,2})", re.I)
MACHINE_GATES = (
    "deterministic", "session", "bevy", "metal", "live-character", "live-proof", "live-negatives",
)
GATES = (*MACHINE_GATES, "manual")
REQUIRED = {
    "commands.json", "execution.json", "gate-results.json", "manual-attestation.json", "metal.json", "metal.png",
    "negative-reconnect.json", "negative-short.json", "persisted-movement.json", "versions.json",
} | {f"{gate}.result" for gate in MACHINE_GATES}
EVIDENCE_BY_GATE = {
    "metal": ("metal.png", "metal.json"),
    "live-proof": ("persisted-movement.json",),
    "live-negatives": ("negative-short.json", "negative-reconnect.json"),
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
    entries = list(directory.iterdir())
    if any(not path.is_file() or path.is_symlink() for path in entries):
        raise SystemExit("bundle artifacts must contain only regular files")
    found = {path.name for path in entries}
    if found != REQUIRED:
        raise SystemExit(f"bundle artifacts must be exactly {sorted(REQUIRED)}")
    return sorted(directory / name for name in REQUIRED)


def read_json(path: pathlib.Path):
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise SystemExit(f"invalid curated JSON artifact {path.name}: {error}") from error


def assert_redacted(value, path: pathlib.Path, *, hash_values: bool = False) -> None:
    if isinstance(value, dict):
        for key, nested in value.items():
            if SENSITIVE.search(str(key)):
                raise SystemExit(f"sensitive field name in {path.name}")
            assert_redacted(nested, path, hash_values=hash_values or key in {"gate_result_hashes", "gate_evidence_hashes"})
    elif isinstance(value, list):
        for nested in value:
            assert_redacted(nested, path, hash_values=hash_values)
    elif isinstance(value, str) and SENSITIVE.search(value):
        raise SystemExit(f"sensitive vocabulary in {path.name}")
    elif isinstance(value, str):
        if any(ord(character) < 32 for character in value) or (not hash_values and RAW_BLOB.search(value)):
            raise SystemExit(f"raw or encoded material in {path.name}")


def allow_keys(value, allowed: set[str], path: pathlib.Path, label: str) -> None:
    if not isinstance(value, dict) or not set(value).issubset(allowed):
        raise SystemExit(f"unexpected fields in {path.name} {label}")


def require_keys(value, required: set[str], path: pathlib.Path, label: str) -> None:
    if not isinstance(value, dict) or set(value) != required:
        raise SystemExit(f"incomplete fields in {path.name} {label}")


def require_string(value, path: pathlib.Path, label: str) -> None:
    if not isinstance(value, str) or not value:
        raise SystemExit(f"invalid string in {path.name} {label}")


def require_integer(value, path: pathlib.Path, label: str) -> None:
    if isinstance(value, bool) or not isinstance(value, int):
        raise SystemExit(f"invalid integer in {path.name} {label}")


def require_number(value, path: pathlib.Path, label: str) -> None:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
        raise SystemExit(f"invalid number in {path.name} {label}")


def validate_pose(value, path: pathlib.Path, label: str, *, offline: bool = False) -> None:
    required = {"space", "east", "north", "elevation"} if offline else {"map_id", "east", "north", "elevation", "orientation"}
    require_keys(value, required, path, label)
    if offline:
        if value["space"] != "offline-display":
            raise SystemExit(f"invalid offline pose space in {path.name}")
    else:
        require_integer(value["map_id"], path, f"{label}.map_id")
    for coordinate in required - {"space", "map_id"}:
        require_number(value[coordinate], path, f"{label}.{coordinate}")


def validate_sidecars(artifacts: pathlib.Path) -> None:
    metal_path = artifacts / "metal.json"
    metal = read_json(metal_path)
    require_keys(metal, {"schema", "phase", "network", "realm_id", "client_build", "character", "rendered_pose", "submitted_pose", "realm_observed_pose"}, metal_path, "metal evidence")
    if metal["schema"] != "miazcore.render-proof.v1" or metal["phase"] != "Offline" or metal["network"] != "disabled" or metal["submitted_pose"] is not None or metal["realm_observed_pose"] is not None:
        raise SystemExit("metal semantic evidence is incomplete")
    require_integer(metal["realm_id"], metal_path, "realm_id")
    require_integer(metal["client_build"], metal_path, "client_build")
    require_string(metal["character"], metal_path, "character")
    validate_pose(metal["rendered_pose"], metal_path, "rendered_pose", offline=True)

    persisted_path = artifacts / "persisted-movement.json"
    persisted = read_json(persisted_path)
    movement_fields = {"schema", "phase", "network", "realm_id", "client_build", "character", "run_speed", "movement_publication", "entry_anchor", "predicted_pose", "rendered_pose", "submitted_pose", "realm_observed_pose", "failure_context", "movement_proof"}
    require_keys(persisted, movement_fields, persisted_path, "persisted movement evidence")
    if persisted["schema"] != "miazcore.live-render-proof.v1" or persisted["phase"] != "PersistedMovementCompared" or persisted["network"] != "reference-realm" or persisted["movement_publication"] != "bounded-ground" or persisted["failure_context"] is not None:
        raise SystemExit("persisted movement semantic evidence is incomplete")
    for name in ("entry_anchor", "predicted_pose", "rendered_pose", "submitted_pose", "realm_observed_pose"):
        validate_pose(persisted[name], persisted_path, name)
    require_integer(persisted["realm_id"], persisted_path, "realm_id")
    require_integer(persisted["client_build"], persisted_path, "client_build")
    require_string(persisted["character"], persisted_path, "character")
    require_number(persisted["run_speed"], persisted_path, "run_speed")
    proof = persisted["movement_proof"]
    require_keys(proof, {"source", "expected", "observed", "delta_metres", "tolerance_metres", "passed"}, persisted_path, "movement proof")
    if proof["source"] != "fresh-reconnect-login-verify-world" or proof["passed"] is not True:
        raise SystemExit("persisted movement semantic evidence is incomplete")
    validate_pose(proof["expected"], persisted_path, "movement proof expected")
    validate_pose(proof["observed"], persisted_path, "movement proof observed")
    require_number(proof["delta_metres"], persisted_path, "movement proof delta_metres")
    require_number(proof["tolerance_metres"], persisted_path, "movement proof tolerance_metres")
    if proof["delta_metres"] > proof["tolerance_metres"]:
        raise SystemExit("persisted movement proof exceeds its tolerance")

    short_path = artifacts / "negative-short.json"
    short = read_json(short_path)
    require_keys(short, movement_fields, short_path, "short negative evidence")
    if short["schema"] != "miazcore.live-render-proof.v1" or short["phase"] != "PersistedMovementRejected" or short["network"] != "reference-realm" or short["movement_publication"] != "bounded-ground":
        raise SystemExit("short negative semantic evidence is incomplete")
    for name in ("entry_anchor", "predicted_pose", "rendered_pose", "submitted_pose", "realm_observed_pose"):
        validate_pose(short[name], short_path, name)
    require_integer(short["realm_id"], short_path, "realm_id")
    require_integer(short["client_build"], short_path, "client_build")
    require_string(short["character"], short_path, "character")
    require_number(short["run_speed"], short_path, "run_speed")
    require_string(short["failure_context"], short_path, "failure_context")
    if short.get("movement_proof") is not None:
        raise SystemExit("short negative evidence must not contain a success proof")

    reconnect_path = artifacts / "negative-reconnect.json"
    reconnect = read_json(reconnect_path)
    require_keys(reconnect, {"schema", "phase", "network", "oracle", "database_derived_success"}, reconnect_path, "reconnect negative evidence")
    if reconnect["schema"] != "miazcore.persisted-movement-negative-probe.v1" or reconnect["phase"] != "ReconnectUnavailableRejected" or reconnect["network"] != "reference-realm" or reconnect["oracle"] != "client-reconnect-failure" or reconnect["database_derived_success"] is not False:
        raise SystemExit("reconnect negative semantic evidence is incomplete")


def recorded_execution(attempt: pathlib.Path, candidate: str, manual_attestation: pathlib.Path) -> dict:
    marker = attempt / "candidate_sha"
    if not marker.is_file() or marker.read_text(encoding="utf-8").strip() != candidate:
        raise SystemExit("attempt candidate does not match the accepted candidate")
    result_paths = {gate: attempt / f"{gate}.result" for gate in MACHINE_GATES}
    if any(not path.is_file() or path.read_text(encoding="utf-8") != "PASS\n" for path in result_paths.values()):
        raise SystemExit("attempt does not contain one PASS result for every machine gate")
    evidence_hashes = {}
    for gate, names in EVIDENCE_BY_GATE.items():
        for name in names:
            path = attempt / "sidecars" / name
            if not path.is_file():
                raise SystemExit(f"attempt is missing retained {gate} evidence: {name}")
            evidence_hashes[name] = sha256(path)
    return {
        "schema": "miazcore.acceptance-execution.v1",
        "candidate_sha": candidate,
        "attempt_id": attempt.name,
        "gate_result_hashes": {
            **{gate: sha256(path) for gate, path in result_paths.items()},
            "manual": sha256(manual_attestation),
        },
        "gate_evidence_hashes": evidence_hashes,
    }


def validate_bundle_content(artifacts: pathlib.Path, candidate: str) -> None:
    manual_path = artifacts / "manual-attestation.json"
    manual = read_json(manual_path)
    require_keys(
        manual,
        {"schema", "candidate_sha", "result", "host", "checks", "notes"},
        manual_path,
        "manual attestation",
    )
    if manual["schema"] != "miazcore.world-entry-manual-attestation.v1" or manual["candidate_sha"] != candidate or manual["result"] != "PASS":
        raise SystemExit("manual attestation must PASS for the exact candidate SHA")
    require_string(manual["host"], manual_path, "host")
    require_string(manual["notes"], manual_path, "notes")
    if set(manual.get("checks", {})) != MANUAL_CHECKS or set(manual["checks"].values()) != {"PASS"}:
        raise SystemExit("manual attestation must explicitly PASS every required check")

    commands_path = artifacts / "commands.json"
    commands = read_json(commands_path)
    allow_keys(commands, {"schema", "commands"}, commands_path, "commands")
    if commands.get("schema") != "miazcore.acceptance-commands.v1" or set(commands.get("commands", {})) != set(GATES):
        raise SystemExit("curated commands must describe every acceptance gate")
    if any(not isinstance(command, list) or not command or not all(isinstance(part, str) for part in command) for command in commands["commands"].values()):
        raise SystemExit("curated commands must be non-empty string arrays")

    results_path = artifacts / "gate-results.json"
    results = read_json(results_path)
    allow_keys(results, {"schema", "results"}, results_path, "gate results")
    if results.get("schema") != "miazcore.acceptance-results.v1" or results.get("results") != {gate: "PASS" for gate in GATES}:
        raise SystemExit("curated gate results must explicitly PASS every required gate")

    versions_path = artifacts / "versions.json"
    versions = read_json(versions_path)
    allow_keys(versions, {"schema", "versions"}, versions_path, "versions")
    if versions.get("schema") != "miazcore.acceptance-versions.v1" or set(versions.get("versions", {})) != {"git", "rustc", "cargo", "python", "platform"}:
        raise SystemExit("curated versions must record every required tool")
    if not all(isinstance(value, str) and value for value in versions["versions"].values()):
        raise SystemExit("curated versions must be non-empty strings")

    execution_path = artifacts / "execution.json"
    execution = read_json(execution_path)
    allow_keys(execution, {"schema", "candidate_sha", "attempt_id", "gate_result_hashes", "gate_evidence_hashes"}, execution_path, "execution")
    if execution.get("schema") != "miazcore.acceptance-execution.v1" or execution.get("candidate_sha") != candidate or not re.fullmatch(r"[0-9A-Za-z._-]+", execution.get("attempt_id", "")):
        raise SystemExit("curated execution provenance is malformed")
    hashes = execution.get("gate_result_hashes", {})
    if set(hashes) != set(GATES) or not all(isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) for value in hashes.values()):
        raise SystemExit("curated execution provenance must bind every gate result")
    for gate in MACHINE_GATES:
        result_path = artifacts / f"{gate}.result"
        if result_path.read_text(encoding="utf-8") != "PASS\n":
            raise SystemExit(f"curated {gate} result must explicitly PASS")
        if hashes[gate] != sha256(result_path):
            raise SystemExit(f"curated execution provenance does not bind {gate} result")
    if hashes["manual"] != sha256(manual_path):
        raise SystemExit("curated execution provenance does not bind manual attestation")
    evidence_hashes = execution.get("gate_evidence_hashes", {})
    expected_evidence = {name for names in EVIDENCE_BY_GATE.values() for name in names}
    if set(evidence_hashes) != expected_evidence or not all(isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) for value in evidence_hashes.values()):
        raise SystemExit("curated execution provenance must bind every semantic evidence artifact")
    for name in expected_evidence:
        if evidence_hashes[name] != sha256(artifacts / name):
            raise SystemExit(f"curated execution provenance does not bind {name}")

    validate_sidecars(artifacts)


def command_version(command: list[str]) -> str:
    try:
        return subprocess.check_output(command, text=True, stderr=subprocess.STDOUT).strip()
    except (OSError, subprocess.CalledProcessError) as error:
        raise SystemExit(f"could not record tool version {' '.join(command)}: {error}") from error


def curate(root: pathlib.Path, candidate: str, manual_attestation: pathlib.Path, attempt: pathlib.Path) -> None:
    artifacts = root / "artifacts"
    artifacts.mkdir(parents=True, exist_ok=False)
    sources = {
        "manual-attestation.json": manual_attestation,
        "metal.png": attempt / "sidecars" / "metal.png",
        "metal.json": attempt / "sidecars" / "metal.json",
        "persisted-movement.json": attempt / "sidecars" / "persisted-movement.json",
        "negative-short.json": attempt / "sidecars" / "negative-short.json",
        "negative-reconnect.json": attempt / "sidecars" / "negative-reconnect.json",
    }
    for name, source in sources.items():
        if not source.is_file():
            raise SystemExit(f"missing curated source artifact: {source}")
        shutil.copyfile(source, artifacts / name)
    execution = recorded_execution(attempt, candidate, manual_attestation)
    for gate in MACHINE_GATES:
        shutil.copyfile(attempt / f"{gate}.result", artifacts / f"{gate}.result")
    (artifacts / "execution.json").write_text(json.dumps(execution, indent=2) + "\n")
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
    validate_bundle_content(root / "artifacts", candidate)
    manifest = {
        "schema": "miazcore.world-entry-acceptance.v2",
        "candidate_sha": candidate,
        "results": read_json(root / "artifacts/gate-results.json")["results"],
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
    validate_bundle_content(root / "artifacts", manifest["candidate_sha"])
    png = root / "artifacts/metal.png"
    if not png.read_bytes().startswith(b"\x89PNG\r\n\x1a\n"):
        raise SystemExit("metal evidence is not a PNG")
    print(f"validated World-entry Acceptance bundle for {manifest['candidate_sha']}")


if __name__ == "__main__":
    if len(sys.argv) < 3 or sys.argv[1] not in {"create", "curate", "validate"}:
        raise SystemExit("usage: validate-acceptance-evidence.py {create|curate|validate} BUNDLE [CANDIDATE_SHA] [MANUAL_ATTESTATION] [ATTEMPT]")
    root = pathlib.Path(sys.argv[2])
    if sys.argv[1] == "create":
        if len(sys.argv) != 4:
            raise SystemExit("create requires a candidate SHA")
        create(root, sys.argv[3])
    elif sys.argv[1] == "curate":
        if len(sys.argv) != 6:
            raise SystemExit("curate requires a candidate SHA, manual attestation, and attempt")
        curate(root, sys.argv[3], pathlib.Path(sys.argv[4]), pathlib.Path(sys.argv[5]))
    else:
        if len(sys.argv) != 3:
            raise SystemExit("validate accepts only a bundle path")
        validate(root)
