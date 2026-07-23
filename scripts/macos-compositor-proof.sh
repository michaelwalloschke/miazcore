#!/usr/bin/env bash
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
output="$1"
proof_flag="$2"
log="$3"
ready="${output%.*}.ready"

command -v swift >/dev/null || { echo "macOS compositor proof requires Swift/CoreGraphics" >&2; exit 64; }
command -v screencapture >/dev/null || { echo "macOS compositor proof requires screencapture" >&2; exit 64; }

rm -f "$output" "$ready"
cargo build --locked -p learning_client >"$log" 2>&1
WGPU_BACKEND=metal RUST_LOG=info target/debug/learning_client "$proof_flag" "$output" >>"$log" 2>&1 &
client_pid=$!
cleanup() { kill "$client_pid" 2>/dev/null || true; wait "$client_pid" 2>/dev/null || true; }
trap cleanup EXIT INT TERM

for _ in {1..120}; do
  [[ -f "$ready" ]] && break
  kill -0 "$client_pid" 2>/dev/null || { cat "$log" >&2; exit 1; }
  sleep 0.25
done
[[ -f "$ready" ]] || { echo "timed out waiting for external compositor proof readiness" >&2; exit 1; }

window_id="$(swift "$root/scripts/macos-window-id.swift" "$client_pid" 'Miazcore — Diagnostic World')" || exit $?
screencapture -x -o -l"$window_id" "$output" || { echo "macOS Screen Recording permission is required" >&2; exit 1; }

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
