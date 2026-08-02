# Implement proof-aware Pair client boundary

Type: implementation
Status: open
Blocked by: [Implement deterministic replication harness](26-implement-deterministic-replication-harness.md), [Implement Fixture Pair reset and profiles](21-implement-fixture-pair-reset-and-profiles.md)

## Objective

Let each closed Fixture Profile participate safely in one parent-owned
Role-reversed Replication Proof by producing atomic semantic sidecars and
obeying bounded proof controls.

## Entry gate

Ticket 26 is resolved; Ticket 21's Pair profiles, Placement Probe, canonical
Realm recovery, and three-fixture health are currently valid.

## Scope

- Add closed Pair proof admission, non-secret parent-owned runtime workspace,
  atomic command/sidecar behavior, revision acknowledgement, bounded role turns,
  manual review checkpoints, same-map projection-snap acknowledgement, and
  clean shutdown.
- Preserve the existing private credential boundary and Pair profile selection.

## Out of scope

- Parent lock/reset ownership, dual-window capture, final bundle curation,
  arbitrary remote control, retry, peer reconnect, LAN, or Windows runtime.

## Acceptance

1. Unknown profile, unowned control directory, caller credential/path/endpoint,
   wrong command, stale revision, duplicate revision, profile drift, or
   malformed sidecar fails before it can claim proof progress.
2. Accepted control history is atomic and redacted; sidecars contain only the
   closed semantic evidence vocabulary and never derive success from pixels,
   database state, or local poses.
3. Review checkpoints gate local movement correctly; the presentation snap
   writes no movement frame and does not mutate Realm-observed remote truth.
4. Clean shutdown reaches the existing offline semantics and preserves terminal
   sidecar evidence.
5. Client tests, fake integration tests, Pair readiness smoke, and workspace
   check pass.

## Required evidence

- Closed CLI/control/sidecar tests covering ordering, temporary-write rename,
  redaction, input gating, snap acknowledgement, and terminal cleanup.
