#!/usr/bin/env bash
set -euo pipefail

miazcore_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
miazcore_lock_dir="$miazcore_root/.scratch/learning-client/.realm-test.lock"
cd "$miazcore_root"

miazcore_image="artifacts/live-movement-smoke.png"
miazcore_log="artifacts/live-movement-smoke.log"
mkdir -p artifacts

if ! mkdir "$miazcore_lock_dir" 2>/dev/null; then
    echo "Live Movement smoke is already owned by another process" >&2
    exit 75
fi
cleanup() {
    rmdir "$miazcore_lock_dir"
}
trap cleanup EXIT INT TERM

./infra/azerothcore/realm reset-state --yes
./infra/azerothcore/realm health

scripts/macos-compositor-proof.sh "$miazcore_image" --live-movement-external-proof-output "$miazcore_log"
rg -q 'AdapterInfo .*backend: Metal' "$miazcore_log"
rg -q 'rendered proof saved' "$miazcore_log"

python3 - "$miazcore_image" "${miazcore_image%.*}.json" <<'PY'
import json, pathlib, struct, subprocess, sys

image_path = pathlib.Path(sys.argv[1])
if image_path.stat().st_size < 100_000:
    raise SystemExit("live movement smoke failed: screenshot is implausibly small")
with image_path.open("rb") as image_file:
    header = image_file.read(24)
if header[:8] != b"\x89PNG\r\n\x1a\n":
    raise SystemExit("live movement smoke failed: screenshot is not PNG")
width, height = struct.unpack(">II", header[16:24])
if width < 1024 or height < 720:
    raise SystemExit(f"live movement smoke failed: screenshot is only {width}x{height}")
bitmap_path = image_path.with_suffix(".bmp")
subprocess.run(["sips", "-s", "format", "bmp", str(image_path), "--out", str(bitmap_path)], check=True, stdout=subprocess.DEVNULL)
bitmap = bitmap_path.read_bytes()
bitmap_path.unlink()
offset = int.from_bytes(bitmap[10:14], "little")
if not any(any(pixel[channel] > 8 for channel in range(3)) for pixel in zip(*[iter(bitmap[offset:])]*4)):
    raise SystemExit("live movement smoke failed: screenshot is all black")

sidecar = json.load(open(sys.argv[2], encoding="utf-8"))
if sidecar.get("phase") != "MovementReady":
    raise SystemExit("live movement smoke failed: session did not reach MovementReady")
if sidecar.get("movement_publication") != "bounded-ground":
    raise SystemExit("live movement smoke failed: bounded movement was not declared")
anchor = sidecar["entry_anchor"]
submitted = sidecar["submitted_pose"]
observed = sidecar["realm_observed_pose"]
if submitted == anchor:
    raise SystemExit("live movement smoke failed: no completed movement write was projected")
if observed != anchor:
    raise SystemExit("live movement smoke failed: realm-observed pose was relabelled")
predicted = sidecar.get("predicted_pose")
if predicted != submitted:
    raise SystemExit("live movement smoke failed: submitted pose is not the latest predicted pose")
if abs(submitted["east"] - anchor["east"]) > 5.0 or abs(submitted["north"] - anchor["north"]) > 5.0:
    raise SystemExit("live movement smoke failed: submitted pose exceeded the five-metre bounded envelope")
print(f"live movement smoke passed: {width}x{height} Metal image and bounded client-written pose remain distinct from realm observation")
PY

if rg -n -i '(password|session[_ -]?key|raw packet|credential)' "${miazcore_image%.*}.json"; then
    echo "live movement smoke failed: secret-bearing vocabulary in sidecar" >&2
    exit 1
fi

./infra/azerothcore/realm health
