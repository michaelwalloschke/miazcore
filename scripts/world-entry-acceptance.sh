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

# macOS presents a Bevy/Metal window differently when the process has no
# terminal. `script` keeps a pseudoterminal for the actual GUI gate while also
# retaining its transcript as the canonical attempt log.
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

run_gate deterministic cargo test --locked -p client_protocol --tests
run_gate session cargo test --locked -p client_session
run_gate bevy scripts/check.sh
run_gui_gate metal scripts/render-smoke.sh
run_gate live-character scripts/live-character-selection.sh
run_gui_gate live-proof scripts/persisted-movement-smoke.sh
run_gui_gate live-negatives scripts/persisted-movement-negative-probes.sh

python3 scripts/validate-acceptance-evidence.py curate "$bundle" "$candidate_sha" "$manual_attestation"
python3 scripts/validate-acceptance-evidence.py validate "$bundle"
echo "World-entry Acceptance passed: $bundle (diagnostic attempt retained: $attempt)"
