#!/usr/bin/env bash
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
output="$1"
proof_flag="$2"
log="$3"
ready="${output%.*}.ready"
captured="${output%.*}.captured"

command -v swift >/dev/null || { echo "macOS compositor proof requires Swift/CoreGraphics" >&2; exit 64; }
command -v screencapture >/dev/null || { echo "macOS compositor proof requires screencapture" >&2; exit 64; }

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

window_id=""
for _ in {1..20}; do
  if window_id="$(swift "$root/scripts/macos-window-id.swift" "$client_pid" 'Miazcore — Diagnostic World' 2>/dev/null)"; then
    break
  fi
  kill -0 "$client_pid" 2>/dev/null || { cat "$log" >&2; exit 1; }
  sleep 0.25
done
[[ -n "$window_id" ]] || {
  echo "exact Diagnostic World window was not found within 5 seconds of proof readiness" >&2
  exit 1
}
# The client continues rendering until this adapter acknowledges a plausible
# candidate. This is capture settlement within one proof, never a gate retry.
for attempt in {1..20}; do
  screencapture -x -o -l"$window_id" "$capture_candidate" || {
    echo "macOS Screen Recording permission is required" >&2
    exit 1
  }
  if python3 "$root/scripts/validate-compositor-png.py" "$capture_candidate"; then
    mv "$capture_candidate" "$output"
    : >"$captured"
    break
  fi
  sleep 0.5
done
rm -f "$capture_candidate"
[[ -f "$captured" ]] || {
  echo "macOS compositor did not present a plausible Bevy frame within 10 seconds" >&2
  exit 1
}

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
