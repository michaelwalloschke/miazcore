#!/usr/bin/env bash
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
probe="$root/scripts/placement-probe.sh"
realm="$root/infra/azerothcore/realm"

bash -n "$probe" "$realm"

require() {
    rg -q --fixed-strings "$1" "$2" || {
        echo "missing contract: $1" >&2
        exit 1
    }
}

# Fail closed: a reset status is tolerated only with a fresh completion marker
# written by `realm` after its own final health boundary.
require 'canonical_reset()' "$probe"
require 'MIAZCORE_RESET_COMPLETION_FILE="$marker"' "$probe"
require '== reset-complete' "$probe"
require 'MIAZCORE_RESET_COMPLETION_FILE' "$realm"
require "printf 'reset-complete" "$realm"

# Lock ownership, child reaping, and one retained-lock recovery are all explicit.
require 'started_utc=' "$probe"
require 'reap_children()' "$probe"
require 'Placement Probe could not reap Pair clients; retaining' "$probe"
require 'recovery_attempted' "$probe"
require 'recovery-failed.json' "$probe"

# Only final health may publish a passed artifact; temporary profiles are removed.
require 'stage="final-health"' "$probe"
require "'final_realm_health':'passed'" "$probe"
require "(run / 'profiles.json').unlink()" "$probe"

# The canonical realm health remains the exact three-fixture boundary.
require 'fixture_count" == 3' "$realm"
require 'distinct_accounts" == 3' "$realm"
require 'Pair Pdump provenance checksum failed' "$realm"
