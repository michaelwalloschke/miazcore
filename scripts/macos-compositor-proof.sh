#!/usr/bin/env bash
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
output="$1"
proof_flag="$2"
log="$3"
ready="${output%.*}.ready"
captured="${output%.*}.captured"

command -v swift >/dev/null || { echo "macOS compositor proof requires Swift/ScreenCaptureKit" >&2; exit 64; }

capture_candidate="${output%.*}.capture.png"
rm -f "$output" "$ready" "$captured" "$capture_candidate"
cargo build --locked -p learning_client >"$log" 2>&1
WGPU_BACKEND=metal RUST_LOG=info target/debug/learning_client "$proof_flag" "$output" >>"$log" 2>&1 &
client_pid=$!
cleanup() { kill "$client_pid" 2>/dev/null || true; wait "$client_pid" 2>/dev/null || true; }
trap cleanup EXIT INT TERM

# A normal AzerothCore saving logout completes after 20 seconds. Keep the
# compositor deadline above that lifecycle plus reconnect and render settlement.
for _ in {1..240}; do
  [[ -f "$ready" ]] && break
  kill -0 "$client_pid" 2>/dev/null || { cat "$log" >&2; exit 1; }
  sleep 0.25
done
[[ -f "$ready" ]] || { echo "timed out waiting for external compositor proof readiness" >&2; exit 1; }

# The client continues rendering until this adapter acknowledges a plausible
# candidate. ScreenCaptureKit captures the exact identified window surface,
# instead of asking the legacy compositor for an opportunistic frame.
sleep 3
swift "$root/scripts/macos-window-capture.swift" "$client_pid" 'Miazcore — Diagnostic World' "$capture_candidate" || {
  echo "macOS Screen Recording permission and an exact Diagnostic World window are required" >&2
  exit 1
}
python3 "$root/scripts/validate-compositor-png.py" "$capture_candidate"
mv "$capture_candidate" "$output"
: >"$captured"

for _ in {1..40}; do
  ! kill -0 "$client_pid" 2>/dev/null && break
  sleep 0.25
done
if kill -0 "$client_pid" 2>/dev/null; then
  echo "timed out waiting for Learning Client to exit after compositor capture" >&2
  exit 1
fi
wait "$client_pid"
trap - EXIT INT TERM
