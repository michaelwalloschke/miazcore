#!/usr/bin/env bash
set -euo pipefail

miazcore_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
miazcore_output_dir="${1:-$miazcore_root/artifacts/render-smoke}"
miazcore_image="$miazcore_output_dir/offline-diagnostic-world.png"
miazcore_sidecar="$miazcore_output_dir/offline-diagnostic-world.json"
miazcore_log="$miazcore_output_dir/renderer.log"

mkdir -p "$miazcore_output_dir"
rm -f "$miazcore_image" "$miazcore_sidecar" "$miazcore_log" "${miazcore_image%.*}.ready"

cd "$miazcore_root"
scripts/macos-compositor-proof.sh "$miazcore_image" --external-proof-output "$miazcore_log"

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
    raise SystemExit("render smoke failed: screenshot is implausibly small")
with image_path.open("rb") as image_file:
    header = image_file.read(24)
if header[:8] != b"\x89PNG\r\n\x1a\n":
    raise SystemExit("render smoke failed: screenshot is not PNG")
width, height = struct.unpack(">II", header[16:24])
if width < 1024 or height < 720:
    raise SystemExit(f"render smoke failed: screenshot is only {width}x{height}")
bitmap_path = image_path.with_suffix(".bmp")
import subprocess
subprocess.run(["sips", "-s", "format", "bmp", str(image_path), "--out", str(bitmap_path)], check=True, stdout=subprocess.DEVNULL)
bitmap = bitmap_path.read_bytes()
bitmap_path.unlink()
offset = int.from_bytes(bitmap[10:14], "little")
bits_per_pixel = int.from_bytes(bitmap[28:30], "little")
if bits_per_pixel != 32 or not any(any(pixel[channel] > 8 for channel in range(3)) for pixel in zip(*[iter(bitmap[offset:])]*4)):
    raise SystemExit("render smoke failed: screenshot is all black")

sidecar = json.loads(sidecar_path.read_text())
expected = {
    "schema": "miazcore.render-proof.v1",
    "phase": "Offline",
    "network": "disabled",
    "realm_id": 1,
    "client_build": 12340,
    "character": "Miaztest",
    "submitted_pose": None,
    "realm_observed_pose": None,
}
for key, value in expected.items():
    if sidecar.get(key) != value:
        raise SystemExit(
            f"render smoke failed: sidecar {key!r} is {sidecar.get(key)!r}"
        )
if sidecar["rendered_pose"] != {
    "space": "offline-display",
    "east": 2.4,
    "north": -1.6,
    "elevation": 0.0,
}:
    raise SystemExit("render smoke failed: scripted Rendered Pose is wrong")

print(f"render smoke: {width}x{height} Metal screenshot and semantic sidecar passed")
PY

if rg -n -i '(password|session[_ -]?key|raw packet|credential)' "$miazcore_sidecar"; then
  echo "render smoke failed: secret-bearing vocabulary in sidecar" >&2
  exit 1
fi

shasum -a 256 "$miazcore_image" "$miazcore_sidecar"
