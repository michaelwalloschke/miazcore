#!/usr/bin/env bash
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
probe="$root/scripts/placement-probe.sh"
tmp="$(mktemp -d "${TMPDIR:-/tmp}/miazcore-placement-probe-test.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT

fail() {
    echo "placement probe behavior test failed: $*" >&2
    exit 1
}

make_fixture() {
    local fixture="$1"
    mkdir -p "$fixture"/{.scratch/learning-client,artifacts/shared-host-replication,infra/azerothcore,target/debug/examples,bin}
    cat >"$fixture/infra/azerothcore/realm" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
printf '%s\n' "$1" >>"$root/realm-calls"
case "$1" in
  preflight|health|wait-character-offline) exit 0 ;;
  reset-state)
    test -n "${MIAZCORE_RESET_COMPLETION_FILE:-}" && printf 'reset-complete\n' >"$MIAZCORE_RESET_COMPLETION_FILE"
    exit 0 ;;
  *) exit 64 ;;
esac
EOF
    cat >"$fixture/bin/cargo" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
    cat >"$fixture/target/debug/examples/fixture_pair_ready" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
profile="$1"
case "${FAKE_CLIENT_MODE:-success}" in
  fail) exit 1 ;;
  hang) trap 'exit 0' TERM; while :; do sleep 1; done ;;
esac
touch "$MIAZCORE_PAIR_READY_DIR/$profile.ready"
peer=pair-a
[[ "$profile" == pair-a ]] && peer=pair-b
for _ in {1..100}; do
  [[ -f "$MIAZCORE_PAIR_READY_DIR/$peer.ready" ]] && break
  sleep 0.01
done
[[ -f "$MIAZCORE_PAIR_READY_DIR/$peer.ready" ]] || exit 1
if [[ "$profile" == pair-a ]]; then
  echo 'PAIR_READY profile=pair-a guid=0x2 map=0 east=-8949.950 north=-132.493 elevation=83.531 orientation=0.000 overlap=peer-release'
else
  echo 'PAIR_READY profile=pair-b guid=0x3 map=0 east=-8946.950 north=-132.493 elevation=83.531 orientation=0.000 overlap=peer-release'
fi
EOF
    chmod +x "$fixture/infra/azerothcore/realm" "$fixture/bin/cargo" "$fixture/target/debug/examples/fixture_pair_ready"
}

run_probe() {
    local fixture="$1"
    PATH="$fixture/bin:$PATH" \
      MIAZCORE_PLACEMENT_PROBE_ROOT="$fixture" \
      MIAZCORE_PLACEMENT_PROBE_COMMIT=behavior-test \
      "$probe"
}

success="$tmp/success"
make_fixture "$success"
run_probe "$success"
summary="$(find "$success/artifacts/shared-host-replication" -name summary.json -type f)"
[[ -n "$summary" ]] || fail "successful probe did not write summary"
rg -q '"status": "passed"' "$summary" || fail "successful probe was not passed"
rg -q '"final_realm_health": "passed"' "$summary" || fail "success lacks final health"
[[ ! -e "$success/.scratch/learning-client/.realm-test.lock" ]] || fail "success retained lock"
[[ "$(rg -c '^reset-state$' "$success/realm-calls")" == 2 ]] || fail "success did not reset twice"

held="$tmp/held"
make_fixture "$held"
mkdir "$held/.scratch/learning-client/.realm-test.lock"
if run_probe "$held"; then fail "held lock was accepted"; fi
[[ "$(cat "$held/realm-calls" 2>/dev/null || true)" == "" ]] || fail "held lock mutated realm"

failed="$tmp/failed"
make_fixture "$failed"
if FAKE_CLIENT_MODE=fail run_probe "$failed"; then fail "failed client was accepted"; fi
[[ "$(rg -c '^reset-state$' "$failed/realm-calls")" == 2 ]] || fail "failure did not run exactly one recovery"
[[ ! -e "$failed/.scratch/learning-client/.realm-test.lock" ]] || fail "recoverable failure retained lock"
test -n "$(find "$failed/artifacts/shared-host-replication" -name failure.json -type f)" || fail "failure artifact missing"

interrupted="$tmp/interrupted"
make_fixture "$interrupted"
PATH="$interrupted/bin:$PATH" MIAZCORE_PLACEMENT_PROBE_ROOT="$interrupted" MIAZCORE_PLACEMENT_PROBE_COMMIT=behavior-test FAKE_CLIENT_MODE=hang "$probe" &
pid=$!
for _ in {1..100}; do
  [[ -f "$interrupted/realm-calls" ]] && rg -q '^reset-state$' "$interrupted/realm-calls" && break
  sleep 0.01
done
kill -TERM "$pid"
wait "$pid" || true
[[ ! -e "$interrupted/.scratch/learning-client/.realm-test.lock" ]] || fail "interrupted probe retained lock after reaping"
[[ "$(rg -c '^reset-state$' "$interrupted/realm-calls")" == 2 ]] || fail "interrupted probe did not run one recovery"

realm_copy="$tmp/realm-secret"
mkdir -p "$realm_copy/secrets"
cp "$root/infra/azerothcore/realm" "$realm_copy/realm"
chmod +x "$realm_copy/realm"
for name in database-password database-root-password fixture-account fixture-password fixture-pair-a-account fixture-pair-a-password fixture-pair-b-account fixture-pair-b-password; do
  printf 'test\n' >"$realm_copy/secrets/$name"
  chmod 600 "$realm_copy/secrets/$name"
done
chmod 644 "$realm_copy/secrets/fixture-pair-a-password"
if "$realm_copy/realm" health >/dev/null 2>&1; then fail "insecure secret mode was accepted"; fi
chmod 600 "$realm_copy/secrets/fixture-pair-a-password"
mv "$realm_copy/secrets/fixture-pair-a-password" "$realm_copy/secrets/target"
ln -s target "$realm_copy/secrets/fixture-pair-a-password"
if "$realm_copy/realm" health >/dev/null 2>&1; then fail "symlink secret was accepted"; fi
