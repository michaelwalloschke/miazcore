# Define the Realm-replicated Avatar contract

Type: wayfinder:grilling
Status: resolved
Blocked by: [Trace the remote-player World-Update boundary](03-trace-remote-player-world-update-boundary.md)

## Question

What engine-independent event, snapshot, and identity contract represents one
Realm-replicated Avatar while preserving GUID identity, raw Realm-observed Pose,
time-smoothed Rendered Pose, and the Create/Update/Destroy lifecycle?

## Decision boundaries

The contract may be GUID-keyed but the first acceptance supports exactly one
remote Avatar per client. It must not predict remote movement or relabel a
Rendered Pose as Realm-observed evidence.

## Answer

[Realm-replicated Avatar contract](../research/04-realm-replicated-avatar-contract.md)
defines GUID-keyed lifecycle and Realm-observed Snapshot truth at the
engine-independent session boundary, with time-smoothed Rendered Pose owned
solely by the Bevy projection.

## Comments

- 2026-07-29: Decision accepted: retain only Remote Avatar identity, lifecycle,
  and Realm-observed Pose in the session; keep Rendered Pose presentation-only.
