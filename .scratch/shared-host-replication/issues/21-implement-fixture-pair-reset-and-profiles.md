# Implement Fixture Pair reset and profiles

Type: implementation
Status: resolved
Blocked by: [Design the paired Fixture reset task](12-design-paired-fixture-reset-task.md), [Decide the Fixture Profile and secret contract](05-decide-fixture-profile-and-secret-contract.md)

## Objective

Implement the reset-owned Fixture Pair A/B infrastructure required for valid
Ticket 09 measurement: provenance-checked Pair Pdump fixtures, ignored `0600`
credential files, closed `--fixture-profile pair-a|pair-b` client selection,
paired reset health, and a lock-held Placement Probe.

## Scope

- Extend only the canonical local Reference Realm reset workflow and Learning
  Client configuration adapter; preserve the existing single fixture unchanged.
- Create/restore Pair secrets and accounts before importing their Pair Pdumps;
  retain only fixed repository-relative filenames, hashes, and redacted facts.
- Enforce exactly three reset fixtures, canonical loopback endpoints, lock
  discipline, one recovery attempt, and no CLI credential/endpoint overrides.
- Implement a redacted Placement Probe that proves independent Pair profiles,
  distinct GUIDs, `MovementReady`, and the 3.0-m stored/Entry-Anchor relation.

## Out of scope

- Remote Avatar decoder/presentation, Role-reversed Replication Proof,
  orchestrator role commands, LAN/Windows, general account management, or
  database state as multiplayer acceptance.

## Acceptance

1. Clean reset creates exactly `Miaztest`, `Miazpaira`, and `Miazpairb`, all
   offline, with Pair B 3.0 m east of Pair A within the established storage
   tolerance; final health succeeds.
2. Both closed profiles select distinct private credentials/Characters and
   reach concurrent `MovementReady` on loopback in the Placement Probe.
3. Script/unit tests cover locks, secrets, profile parsing/redaction, fixture
   health, probe success/failure, and cleanup/recovery.
4. Full checks and a reset-scoped live Placement Probe pass with no retained
   credential, session, payload, or raw database data.

## Required evidence

- Reviewed Pair Pdump provenance manifest and SHA-256 values.
- Redacted reset and Placement-Probe artifacts bound to the implementation
  commit, plus final canonical Realm health.

## Answer

Canonical reset now provisions and health-checks the three offline fixtures
with provenance-checked Pair Pdump inputs and owner-only private credentials.
Closed Pair A/B profiles reach `MovementReady` behind a peer-ready barrier, so
the live probe proves overlapping retained sessions before either client may
publish its redacted ready record. The probe has bounded child reaping,
preflight-before-mutation, a single recovery path, and a final-health-bound
summary.

## Comments

- 2026-08-01: Claimed. Pair Pdump generation remains provenance-gated; reset
  implementation must fail closed until both reviewed source fixtures exist.
- 2026-08-01: Closed profile loading is implemented behind the private Learning
  Client configuration adapter. Canonical reset-owned Pair provisioning,
  health, lock/recovery semantics, and the Placement Probe remain open work;
  Pair Pdump generation/review is their explicit human checkpoint.
- 2026-08-02: Resolved. Implementation commit `660fce8dc0276326598573da20fdb5b214b6a480`; redacted live evidence is
  `artifacts/shared-host-replication/20260801T222218Z-placement-probe/summary.json`
  (two distinct GUIDs, 3.0 m east relation, final health; peer-release is
  validated before this sanitized summary is written).
  `./infra/azerothcore/realm health` and `scripts/check.sh` passed after the
  run; the latter includes behavioral lock, failure/recovery, cleanup and
  secret-safety coverage.
