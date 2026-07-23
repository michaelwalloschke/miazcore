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
git diff --quiet && git diff --cached --quiet || {
    echo "acceptance requires a clean candidate checkout" >&2
    exit 65
}
candidate_sha="$(git rev-parse HEAD)"
attempt="artifacts/world-entry-acceptance/$(date -u +%Y%m%dT%H%M%SZ)-${candidate_sha:0:12}"
mkdir -p "$attempt/logs" "$attempt/artifacts"

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

run_gate deterministic cargo test --locked -p client_protocol --tests
run_gate session cargo test --locked -p client_session
run_gate bevy scripts/check.sh
run_gate metal scripts/render-smoke.sh
run_gate live-character scripts/live-character-selection.sh
run_gate live-proof scripts/persisted-movement-smoke.sh
run_gate live-negatives scripts/persisted-movement-negative-probes.sh

cp "$manual_attestation" "$attempt/artifacts/manual-attestation.json"
cp artifacts/render-smoke/offline-diagnostic-world.png "$attempt/artifacts/metal.png"
cp artifacts/render-smoke/offline-diagnostic-world.json "$attempt/artifacts/metal.json"
cp artifacts/persisted-movement-smoke.json "$attempt/artifacts/persisted-movement.json"
python3 scripts/validate-acceptance-evidence.py create "$attempt" "$candidate_sha"
python3 scripts/validate-acceptance-evidence.py validate "$attempt"
echo "World-entry Acceptance passed: $attempt"
