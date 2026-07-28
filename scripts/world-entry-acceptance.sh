#!/usr/bin/env bash
set -euo pipefail

# Runs the three machine-owned World-entry Acceptance gates once on one clean
# candidate. It deliberately does not retry failed commands: every invocation
# becomes a separately retained attempt under artifacts/.
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
manual_attestation=""
if [[ "${1:-}" == "--manual-attestation" ]]; then
    manual_attestation="${2:-}"
fi
[[ -n "$manual_attestation" && -f "$manual_attestation" ]] || {
    echo "usage: $0 --manual-attestation <completed-attestation.json>" >&2
    exit 64
}
cd "$root"
[[ -z "$(git status --porcelain --untracked-files=all)" ]] || {
    echo "acceptance requires a clean candidate checkout" >&2
    exit 65
}
candidate_sha="$(git rev-parse HEAD)"
attempt="artifacts/world-entry-attempts/$(date -u +%Y%m%dT%H%M%SZ)-${candidate_sha:0:12}"
bundle="artifacts/world-entry-acceptance/$(basename "$attempt")"
mkdir -p "$attempt/logs"
printf '%s\n' "$candidate_sha" >"$attempt/candidate_sha"

run_gate() {
    local name="$1"; shift
    local log="$attempt/logs/$name.log"
    if "$@" >"$log" 2>&1; then
        printf 'PASS\n' >"$attempt/$name.result"
    else
        printf 'FAIL\n' >"$attempt/$name.result"
        echo "World-entry Acceptance $name gate failed; retained: $log" >&2
        exit 1
    fi
}

# Kept for compatibility with diagnostic callers that explicitly require a
# pseudoterminal.  The acceptance path intentionally launches compositor
# proofs directly: on this host an intervening `script(1)` process can expose
# a black ScreenCaptureKit surface even while the client renders correctly.
run_gui_gate() {
    local name="$1"; shift
    local log="$attempt/logs/$name.log"
    command -v script >/dev/null || {
        echo "World-entry Acceptance requires script(1) for the macOS GUI gate" >&2
        exit 64
    }
    if script -q "$log" "$@"; then
        printf 'PASS\n' >"$attempt/$name.result"
    else
        printf 'FAIL\n' >"$attempt/$name.result"
        echo "World-entry Acceptance $name gate failed; retained: $log" >&2
        exit 1
    fi
}

retain_sidecars() {
    local gate="$1"; shift
    local destination="$attempt/sidecars"
    mkdir -p "$destination"
    for source in "$@"; do
        [[ -f "$source" ]] || {
            echo "World-entry Acceptance $gate gate did not retain required semantic evidence: $source" >&2
            exit 1
        }
        cp "$source" "$destination/$(basename "$source")"
    done
}

run_gate deterministic cargo test --locked -p client_protocol --tests
run_gate session cargo test --locked -p client_session
run_gate bevy scripts/check.sh
run_gate metal scripts/render-smoke.sh
retain_sidecars metal artifacts/render-smoke/offline-diagnostic-world.png artifacts/render-smoke/offline-diagnostic-world.json
mv "$attempt/sidecars/offline-diagnostic-world.png" "$attempt/sidecars/metal.png"
mv "$attempt/sidecars/offline-diagnostic-world.json" "$attempt/sidecars/metal.json"
run_gate live-character scripts/live-character-selection.sh
# ScreenCaptureKit captures these proofs only when the client remains a direct
# child of the permission-bearing desktop process.
run_gate live-proof scripts/persisted-movement-smoke.sh
retain_sidecars live-proof artifacts/persisted-movement-smoke.json
mv "$attempt/sidecars/persisted-movement-smoke.json" "$attempt/sidecars/persisted-movement.json"
run_gate live-negatives scripts/persisted-movement-negative-probes.sh
retain_sidecars live-negatives artifacts/persisted-movement-short-negative.json artifacts/persisted-movement-reconnect-unavailable.json
mv "$attempt/sidecars/persisted-movement-short-negative.json" "$attempt/sidecars/negative-short.json"
mv "$attempt/sidecars/persisted-movement-reconnect-unavailable.json" "$attempt/sidecars/negative-reconnect.json"

python3 scripts/validate-acceptance-evidence.py curate "$bundle" "$candidate_sha" "$manual_attestation" "$attempt"
python3 scripts/validate-acceptance-evidence.py validate "$bundle"
echo "World-entry Acceptance passed: $bundle (diagnostic attempt retained: $attempt)"
