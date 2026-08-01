# Paired Fixture reset task

Ticket: [12 – Design the paired Fixture reset task](../issues/12-design-paired-fixture-reset-task.md)

## Purpose and boundary

The future `infra/azerothcore/realm reset-state --yes` establishes exactly the
isolated three-fixture set required before Pair protocol experiments:
`Miaztest`, `Miazpaira`, and `Miazpairb`. This task specifies the repository
and local-environment work that makes that reset reproducible and inspectable.
It does not implement the Shared-Host Multi-client Simulation, run a
Role-reversed Replication Proof, or treat database state as Remote Avatar
evidence.

## Target resources

| Resource | Reset-owned required state |
| --- | --- |
| Compose project | only `miazcore-reference-realm`, checked-in `infra/azerothcore/compose.yaml`, loopback auth/world ports only |
| State volume | recreated from empty state and contains exactly the three declared fixture accounts/Characters |
| Existing fixture | unchanged `MIAZTEST` / `Miaztest`, existing credential pair and `reference-character.pdump` |
| Pair A | `MIAZPAIRA` / `Miazpaira`, `reference-pair-a-character.pdump`, ignored `fixture-pair-a-account` and `fixture-pair-a-password` |
| Pair B | `MIAZPAIRB` / `Miazpairb`, `reference-pair-b-character.pdump`, ignored `fixture-pair-b-account` and `fixture-pair-b-password` |
| Lock/run record | canonical `.scratch/learning-client/.realm-test.lock` owner record and one ignored redacted run directory |

The four Pair secret files are regular, owner-only `0600` files under the
versioned repository-relative contract path `infra/azerothcore/secrets/`.
Their contents and any concrete resolved local path never enter a command line,
environment, or retained artifact. Pair Pdump files carry only
provenance-approved fixture data, never credentials.

## One-time HITL: Pair Pdump provenance

Before automation may accept the Pair Pdump files, a maintainer performs this
checklist in a clean, lock-held local Realm:

1. Start from an empty character state, create separate Pair A/B accounts and
   Characters through the controlled Worldserver workflow, not hand-written
   character SQL.
2. Place A on map `0`; place B exactly 3.0 metres east with identical north,
   elevation, and heading. Do not call either stored placement an Entry Anchor.
3. Export one Pdump per Character through the existing controlled workflow;
   record generator version, source commit, SHA-256, and review result in a
   non-secret provenance manifest checked into the repository.
4. Inspect that each dump contains only the expected Character fixture data and
   no account credentials, session material, local paths, or runtime logs.
5. Run one clean reset plus Placement Probe. Human acceptance is limited to the
   manifest review and the probe result; Remote Avatar visibility remains out
   of scope.

Any failed provenance review rejects the Pdump and returns to step 1. Existing
single-fixture artifacts are not rewritten to create the Pair.

## AFK reset implementation checklist

The later implementation must execute the following sequence without prompts
when passed `--yes`:

1. Acquire the canonical atomic lock with `mkdir`; on contention return `75`
   before Docker, secrets, or client processes are touched. Write only script,
   PID, UTC start, and run directory to the owner record.
2. Validate all three checked-in Pdump manifests/hashes and the Pair secret
   directory policy before destructive state changes. A present Pair secret
   file must be regular, non-symlinked, owner-only, and valid through the
   redacted configuration boundary; a missing Pair file is allowed only
   because this reset atomically creates/restores it in step 4.
3. Stop only the canonical Compose project; remove only the labelled state
   volume using the existing `realm` volume-label guard. Never reset
   `server-data`, bind a LAN port, or accept a project/address override.
4. Recreate the canonical Realm, atomically create/restore the exact distinct
   Pair credential files with `0600` permissions, and create/verify the two
   distinct Realm accounts with the required expansion state. Import the
   original fixture and then Pair A/B Pdump fixtures into those pre-existing
   account names. Finally recreate auth/world containers and wait for their
   Docker health checks before socket-level health.
5. Run the paired database health query and retain an allowlisted summary. It
   must prove exactly one distinct account/Character for each declared name;
   map `0`; Pair B east minus Pair A east within `0.001 m` of `3.0 m`; same
   north, elevation, and heading within `0.001` persisted units; `online=0`;
   and no transport attachment. The tight persistence tolerance accounts only
   for stored numeric representation; the Placement Probe separately proves
   the Fixture Pair Start Placement relation. It does
   not record account IDs, database rows, credentials, or values as replication
   success evidence.
6. Release the lock only after health succeeds. On any post-mutation failure,
   make one same-owner reset/health recovery attempt; preserve a redacted
   failure marker and the lock for human recovery if it fails.

## Placement Probe and cleanup

The separate Placement Probe is the first client exercise after a successful
reset. It acquires the same lock, starts the fixed Pair A/B profiles
concurrently, and proves each reaches `MovementReady` with distinct Realm GUIDs
and same-map Realm-observed Entry Anchors consistent with the stored 3.0-m
placement relation. It writes only redacted profile, GUID shorthand, map,
finite pose relation, phase, and failure-category evidence.

After the probe, request clean shutdown of both clients, wait through the
shared bounded offline settlement, run scoped `reset-state --yes` again, and
require final health. A client exit is not offline settlement. Any failure
reaps tracked children and follows the same one-recovery-or-retained-lock rule.

## Human recovery checklist

When an interrupted reset preserves the lock:

1. Verify the recorded PID is not an active holder and inspect the redacted run
   record; PID non-existence alone does not grant cleanup permission.
2. Restore canonical `realm health` without using an alternate Compose project
   or address override.
3. Confirm the labelled resource targets and that no Pair client remains.
4. Remove only `.scratch/learning-client/.realm-test.lock`, then restart the
   AFK reset from the beginning. Do not reuse partial Pair data or a temporary
   account.

## Verification required by the implementation ticket

1. Script tests cover lock contention, allowed labelled targets, secret modes,
   manifest mismatch, exactly-three-fixture health, one recovery attempt, and
   retained-lock failure.
2. A reset-scoped Placement Probe proves both profiles independently reach
   `MovementReady` with distinct GUIDs and the declared relation.
3. Redaction tests reject credentials, secret paths, raw database output, and
   packet/session material from all records.
4. A clean reset after the probe returns to the same three-fixture health
   state; it does not assert any Remote Avatar observation.
