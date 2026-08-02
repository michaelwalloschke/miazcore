# Measure replication timeline and tolerances

Type: wayfinder:research
Status: resolved
Blocked by: [Design Fixture Pair provisioning](02-design-fixture-pair-provisioning.md), [Trace the remote-player World-Update boundary](03-trace-remote-player-world-update-boundary.md)

## Question

What measured local-Realm timing and update cadence support a deterministic
serial two-to-four-metre Replicated Move, same-map observer comparison within
0.25 m, and bounded Remote Avatar removal after clean logout?

## Answer criteria

Establish evidence-backed deadlines and settle where smoothing snaps. Do not
borrow thresholds from wall-clock intuition or turn this into a latency or
performance benchmark.

## Answer

[Fixture Pair replication timeline and tolerances](../research/09-replication-timeline-and-tolerances.md)
records a shared-clock, reset-scoped three-sample Pair A/B study and locks the
local serial role-turn deadlines, pose tolerance, and data-derived snap
threshold. Its passed artifact ends with a canonical Realm health check.

## Comments

- 2026-08-01: Initial temporary-peer trace is not accepted as Ticket 09
  evidence. Its observer transcript and controller used independent monotonic
  origins, it did not use reset-owned Fixture Pair A/B profiles, and its
  create-pose validator matched the wrong semantic event name. The ticket
  remains claimed until a shared-clock, Fixture-Pair measurement is available.
