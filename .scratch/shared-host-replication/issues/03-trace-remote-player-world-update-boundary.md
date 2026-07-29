# Trace the remote-player World-Update boundary

Type: wayfinder:research
Status: resolved
Blocked by: [Measure Reference Realm multi-session behavior](01-measure-reference-realm-multi-session-behavior.md)

## Question

Which build-12340 World-Update records from the local Reference Realm establish
a remote player's GUID identity, display metadata, map/pose/heading updates,
and clean removal, and which decoder boundaries can safely ignore unrelated
objects without disturbing encrypted framing?

## Answer criteria

Produce a project-owned, provenance-recorded transcript analysis and a precise
minimal decoding contract. Keep raw authenticated traffic out of repository
artifacts and defer arbitrary object, NPC, combat, and teleport support.

## Answer

[Remote-player World-Update boundary](../research/03-remote-player-world-update-boundary.md)
records the pinned source trace, local two-session precondition, and the
frame-first structural-decoding contract. It establishes that player names are
a separate query protocol, so arbitrary update-field decoding remains deferred.

## Comments

- 2026-07-29: Resolved by the reset-scoped semantic trace harness and a
  checked-in reviewed evidence record. Both retain only peer GUID, lifecycle,
  movement opcode, map, and finite pose: CreateObject2 → Heartbeat/Stop →
  Destroy after the controlled logout proof. No raw authenticated traffic is
  retained.
