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

# Fail closed: reset commands are not wrapped in a health-only success fallback.
require 'MIAZCORE_SKIP_PULL=1 MIAZCORE_REALM_LOCK_HELD=1 ./infra/azerothcore/realm reset-state --yes' "$probe"
if rg -q 'canonical_reset|reset command returned non-zero' "$probe"; then
    echo 'Placement Probe accepted a failed reset through health fallback' >&2
    exit 1
fi

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
