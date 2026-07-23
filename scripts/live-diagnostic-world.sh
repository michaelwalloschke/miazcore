#!/usr/bin/env bash
set -euo pipefail

miazcore_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
miazcore_lock_dir="$miazcore_root/.scratch/learning-client/.realm-test.lock"
miazcore_output_dir="${1:-$miazcore_root/artifacts/live-diagnostic-world}"
miazcore_image="$miazcore_output_dir/live-diagnostic-world.png"
miazcore_sidecar="$miazcore_output_dir/live-diagnostic-world.json"
miazcore_log="$miazcore_output_dir/renderer.log"

if ! mkdir "$miazcore_lock_dir" 2>/dev/null; then
    echo "Live Diagnostic World gate is already owned by another process" >&2
    exit 75
fi
cleanup() {
    rmdir "$miazcore_lock_dir"
}
trap cleanup EXIT INT TERM

mkdir -p "$miazcore_output_dir"
rm -f "$miazcore_image" "$miazcore_sidecar" "$miazcore_log" "${miazcore_image%.*}.ready"

cd "$miazcore_root"
./infra/azerothcore/realm reset-state --yes
./infra/azerothcore/realm health
scripts/macos-compositor-proof.sh "$miazcore_image" --live-external-proof-output "$miazcore_log"

rg -q 'AdapterInfo .*backend: Metal' "$miazcore_log"
rg -q 'rendered proof saved' "$miazcore_log"

python3 - "$miazcore_image" "$miazcore_sidecar" <<'PY'
import json
import pathlib
import struct
import sys

image_path = pathlib.Path(sys.argv[1])
sidecar_path = pathlib.Path(sys.argv[2])

if image_path.stat().st_size < 100_000:
    raise SystemExit("live Diagnostic World gate failed: screenshot is implausibly small")
with image_path.open("rb") as image_file:
    header = image_file.read(24)
if header[:8] != b"\x89PNG\r\n\x1a\n":
    raise SystemExit("live Diagnostic World gate failed: screenshot is not PNG")
width, height = struct.unpack(">II", header[16:24])
if width < 1024 or height < 720:
    raise SystemExit(f"live Diagnostic World gate failed: screenshot is only {width}x{height}")
bitmap_path = image_path.with_suffix(".bmp")
import subprocess
subprocess.run(["sips", "-s", "format", "bmp", str(image_path), "--out", str(bitmap_path)], check=True, stdout=subprocess.DEVNULL)
bitmap = bitmap_path.read_bytes()
bitmap_path.unlink()
offset = int.from_bytes(bitmap[10:14], "little")
bits_per_pixel = int.from_bytes(bitmap[28:30], "little")
if bits_per_pixel != 32 or not any(any(pixel[channel] > 8 for channel in range(3)) for pixel in zip(*[iter(bitmap[offset:])]*4)):
    raise SystemExit("live Diagnostic World gate failed: screenshot is all black")

sidecar = json.loads(sidecar_path.read_text())
anchor = sidecar.get("entry_anchor")
expected = {
    "schema": "miazcore.live-render-proof.v1",
    "phase": "MovementReady",
    "network": "reference-realm",
    "realm_id": 1,
    "client_build": 12340,
    "character": "Miaztest",
    "movement_publication": "disabled",
    "submitted_pose": anchor,
}
for key, value in expected.items():
    if sidecar.get(key) != value:
        raise SystemExit(
            f"live Diagnostic World gate failed: sidecar {key!r} is {sidecar.get(key)!r}"
        )
if sidecar.get("run_speed", 0) <= 0:
    raise SystemExit("live Diagnostic World gate failed: run speed is not positive")
rendered = sidecar.get("rendered_pose")
observed = sidecar.get("realm_observed_pose")
for label, pose in (("entry anchor", anchor), ("rendered pose", rendered), ("realm-observed pose", observed)):
    if not isinstance(pose, dict) or pose.get("map_id") != 0:
        raise SystemExit(f"live Diagnostic World gate failed: {label} is missing map 0")
    if abs(pose.get("east", 0) + 8949.95) > 0.01 or abs(pose.get("north", 0) + 132.493) > 0.01:
        raise SystemExit(f"live Diagnostic World gate failed: {label} does not match the Entry Anchor")
if rendered != anchor or sidecar.get("submitted_pose") != anchor or observed != anchor:
    raise SystemExit("live Diagnostic World gate failed: entry pose truths diverged before movement")

print(f"live Diagnostic World: {width}x{height} Metal screenshot and MovementReady sidecar passed")
PY

if rg -n -i '(password|session[_ -]?key|raw packet|credential)' "$miazcore_sidecar"; then
    echo "live Diagnostic World gate failed: secret-bearing vocabulary in sidecar" >&2
    exit 1
fi

./infra/azerothcore/realm health
shasum -a 256 "$miazcore_image" "$miazcore_sidecar"
