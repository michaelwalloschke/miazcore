#!/usr/bin/env bash
set -euo pipefail

miazcore_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
miazcore_lock_dir="$miazcore_root/.scratch/learning-client/.realm-test.lock"
cd "$miazcore_root"

miazcore_image="artifacts/persisted-movement-smoke.png"
miazcore_log="artifacts/persisted-movement-smoke.log"
mkdir -p artifacts

if ! mkdir "$miazcore_lock_dir" 2>/dev/null; then
    echo "Persisted Movement smoke is already owned by another process" >&2
    exit 75
fi
cleanup() { rmdir "$miazcore_lock_dir"; }
trap cleanup EXIT INT TERM

./infra/azerothcore/realm reset-state --yes
./infra/azerothcore/realm health
scripts/macos-compositor-proof.sh "$miazcore_image" --persisted-movement-external-proof-output "$miazcore_log"
rg -q 'AdapterInfo .*backend: Metal' "$miazcore_log"
rg -q 'rendered proof saved' "$miazcore_log"

python3 - "$miazcore_image" "${miazcore_image%.*}.json" <<'PY'
import json, pathlib, struct, subprocess, sys

image, sidecar_path = map(pathlib.Path, sys.argv[1:])
if image.stat().st_size < 100_000:
    raise SystemExit("persisted movement smoke failed: screenshot is implausibly small")
header = image.read_bytes()[:24]
if header[:8] != b"\x89PNG\r\n\x1a\n":
    raise SystemExit("persisted movement smoke failed: screenshot is not PNG")
width, height = struct.unpack(">II", header[16:24])
if width < 1024 or height < 720:
    raise SystemExit(f"persisted movement smoke failed: screenshot is only {width}x{height}")
bitmap = image.with_suffix(".bmp")
subprocess.run(["sips", "-s", "format", "bmp", str(image), "--out", str(bitmap)], check=True, stdout=subprocess.DEVNULL)
raw = bitmap.read_bytes(); bitmap.unlink()
pixels = raw[int.from_bytes(raw[10:14], "little"):]
if not any(any(channel > 8 for channel in pixel[:3]) for pixel in zip(*[iter(pixels)] * 4)):
    raise SystemExit("persisted movement smoke failed: screenshot is all black")

sidecar = json.loads(sidecar_path.read_text(encoding="utf-8"))
if sidecar.get("phase") != "PersistedMovementCompared":
    raise SystemExit("persisted movement smoke failed: fresh comparison did not complete")
proof = sidecar.get("movement_proof")
if not proof or proof.get("source") != "fresh-reconnect-login-verify-world" or not proof.get("passed"):
    raise SystemExit("persisted movement smoke failed: reconnect is not the sole successful oracle")
if proof.get("expected", {}).get("map_id") != proof.get("observed", {}).get("map_id"):
    raise SystemExit("persisted movement smoke failed: reconnect map differs from submitted oracle")
if proof.get("delta_metres") is None or proof["delta_metres"] > proof.get("tolerance_metres", 0.25):
    raise SystemExit("persisted movement smoke failed: reconnect pose exceeds tolerance")
anchor, submitted = sidecar["entry_anchor"], sidecar["submitted_pose"]
distance = ((submitted["east"] - anchor["east"]) ** 2 + (submitted["north"] - anchor["north"]) ** 2) ** .5
if not 2 <= distance <= 4:
    raise SystemExit("persisted movement smoke failed: canonical move was not between two and four metres")
print(f"persisted movement smoke passed: {width}x{height} Metal image and fresh reconnect proof")
PY

if rg -n -i '(password|session[_ -]?key|raw packet|credential)' "${miazcore_image%.*}.json"; then
    echo "persisted movement smoke failed: secret-bearing vocabulary in sidecar" >&2
    exit 1
fi
./infra/azerothcore/realm health
