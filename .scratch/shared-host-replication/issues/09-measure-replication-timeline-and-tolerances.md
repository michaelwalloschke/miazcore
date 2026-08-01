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

[Replication timeline and tolerances](../research/09-replication-timeline-and-tolerances.md)
records the redacted local measurement and establishes bounded lifecycle,
movement-cadence, final-pose-comparison, and Remote Pose Projection limits.

## Comments

- 2026-08-01: A reset-scoped loopback run observed a 3.835 m move, exact
  stopped Submitted/remote-stop equality, and remote Destroy 19.830 s after
  proof start; the resulting local limits fail closed rather than infer state.
