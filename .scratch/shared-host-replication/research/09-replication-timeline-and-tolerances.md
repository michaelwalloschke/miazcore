# Replication timeline and tolerances

Ticket: [09 – Measure replication timeline and tolerances](../issues/09-measure-replication-timeline-and-tolerances.md)

## Method and evidence

One reset-scoped local Reference Realm run used the project-owned fixture plus
a temporary second peer. The observer retained only semantic Remote World
events with monotonic receive offsets; the peer retained only its stopped
Submitted Pose and non-sensitive timeline milestones. The result is recorded
in [the selected evidence](09-replication-timeline-evidence.json), sourced
from the ignored local artifact named there and bound to commit
`00acda54885996a7ba7732b3bcd00b82a48bc6f7`.

This is a local acceptance calibration, not a latency or throughput benchmark.
It does not make a claim for a remote host, LAN, Windows, or general Realm
load.

## Observed local timeline

| Fact | Observed value |
| --- | ---: |
| Peer `MovementReady` to observer `CreateObject2` | 163 ms |
| Move intent start to first observer movement record | 218 ms |
| Largest gap among five received movement records | 112 ms |
| Input idle to received `MSG_MOVE_STOP` | 20 ms |
| `BeginMovementProof` to observer `SMSG_DESTROY_OBJECT` | 19.830 s |
| Controlled horizontal move | 3.835 m |
| Same-map stopped Submitted Pose versus observer final stop | 0.000 m |

The observed peer movement frames advanced in 0.697 m, 0.698 m, 0.697 m, and
0.349 m horizontal increments. The final remote `MSG_MOVE_STOP` pose exactly
matched the peer's stopped Submitted Pose in map, position, and orientation.

## Acceptance calibration

The later Role-reversed Replication Proof uses these local fail-closed bounds:

| Boundary | Limit | Outcome when exceeded |
| --- | --- | --- |
| Peer ready to Remote Avatar `Created` | 2 s | role turn fails before movement; no inferred marker |
| Movement intent start to first remote pose update | 1 s | role turn fails; no local transform substitute |
| Gap between remote movement records while the peer moves | 250 ms | observer faults the replication proof, not the World frame/cipher |
| Local input stop to remote final `MSG_MOVE_STOP` | 1 s | role turn fails; do not compare an intermediate pose |
| Proof start to matching remote `Removed` after clean logout | 25 s | role turn fails with a bounded lifecycle timeout; no stale success marker |
| Final same-map Remote Avatar pose against stopped Submitted Pose | 0.25 m | proof fails; this is evidence comparison, not smoothing error |

The limits deliberately leave a bounded local margin above the single measured
run (respectively 12.3x, 4.6x, 2.2x, 50x, and 1.26x for the time boundaries).
They are deadlines for a deterministic local acceptance gate, not estimates of
network quality. Any later measurement that exceeds a limit reopens this
calibration rather than silently widening it.

## Remote Pose Projection settlement

While the map remains the observer's authenticated map, smooth only a remote
pose delta smaller than `1.0 m`. Snap at a delta of `1.0 m` or more, and always
snap on a map change. The measured ordinary movement increments never exceeded
`0.698 m`, so their rendering remains smooth; a larger discontinuity cannot
be presented as ordinary local cadence. The 0.25 m proof comparison always
uses the raw Realm-observed final stop, never the smoothed Rendered Pose.

## Explicit limits of the measurement

- A single reset-scoped run calibrates only this repository's loopback
  Reference Realm. It does not establish variance, a service-level objective,
  or a peer-crash/reconnect policy.
- The next `CreateObject2` after Destroy was a fresh proof reconnect and is not
  used as a reconnect-success requirement.
- Timeout is a proof-level outcome after frame integrity was established. A
  malformed encrypted frame or unwalkable update container remains a
  whole-session failure under Ticket 08.

## Verification required by later implementation

1. A fake-clock role-turn test exercises every calibrated deadline on both the
   passing edge and one millisecond beyond it.
2. A live reset-scoped proof records the selected monotonic milestones,
   Realm-observed final stop, stopped Submitted Pose, and redacted lifecycle
   outcome for both role directions.
3. Projection tests prove `< 1.0 m` smooths, `>= 1.0 m` snaps, map changes
   snap, and the 0.25 m comparison cannot read Rendered Pose.
