#!/usr/bin/env bash
set -euo pipefail

miazcore_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
lock_dir="$miazcore_root/.scratch/learning-client/.realm-test.lock"

if ! mkdir "$lock_dir" 2>/dev/null; then
    echo "character selection gate is already owned by another process" >&2
    exit 75
fi
cleanup() {
    rmdir "$lock_dir"
}
trap cleanup EXIT INT TERM

cd "$miazcore_root"
./infra/azerothcore/realm health
cargo run --locked -p client_session --example select_fixture_character -- success
cargo run --locked -p client_session --example select_fixture_character -- nonexistent-account
cargo run --locked -p client_session --example select_fixture_character -- absent-character
./infra/azerothcore/realm health
