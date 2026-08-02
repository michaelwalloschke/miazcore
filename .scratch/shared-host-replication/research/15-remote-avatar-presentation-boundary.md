# Remote Avatar presentation boundary

Ticket: [15 – Design the Remote Avatar presentation boundary](../issues/15-design-remote-avatar-presentation-boundary.md)

## Decision

`client_bevy` owns all Remote Avatar rendering and keeps it in a private
`RemoteAvatarPresentation` resource. It receives only `ClientSnapshot` plus
the semantic `ClientEventKind::RemoteAvatar` changes defined by Ticket 14. It
never sees decoder records, sockets, credentials, raw payloads, local movement
intent, Submitted Pose, or the controlled character's Realm-observed Pose.

```text
RemoteAvatarPresentation {
  state: Absent | Present { id, realm_observed_pose, rendered_pose,
                            projection: Smooth | Snap }
                | Fault { id, category },
                | MapContextUnavailable { id },
  last_event_sequence: u64,
  invalidated_through: u64,
}
```

This is a presentation-only resource. `realm_observed_pose` is copied from an
accepted `Created`/`Updated` event or hydration snapshot, while `rendered_pose`
is private mutable Bevy state. Neither the Bevy copy nor `rendered_pose` is
written to `ClientSnapshot`, the session worker, a movement packet, or a
semantic sidecar. The session-originated `Created`/`Updated`
Realm-observed-pose event data remains exportable by the later proof/orchestrator
sidecar path; Bevy cannot alter or substitute it. Neither value feeds local
prediction/reconciliation systems.

## Ingress and invalidation order

`SessionBridgePlugin` continues to refresh `DiagnosticView.snapshot` in the
`Ingress` set before all presentation systems. It additionally places every
drained Remote Avatar event for the current frame into a private
`RemoteAvatarIngress` batch before it trims the existing eight-entry diagnostic
tail. The batch is consumed once in `Presentation`; it is not public Bevy
events, retained history, or a second session queue. Its maximum one-frame
size is the session event capacity (64), and any larger input is an invariant
failure that clears remote presentation rather than discarding an event.

At the start of every render update, before consuming that batch, the Remote
Avatar system reads the refreshed snapshot and applies Ticket 14's mandatory
fence rule:

1. `ClientSnapshot.remote_avatar_invalidated_through` is the session-owned
   monotonic fence (initially zero; it advances only on remote FIFO-saturation
   failure). If the session is `Failed` or `Offline`, despawn the remote entity
   tree, clear all observed/rendered poses, set `Absent`, discard the entire
   current Remote Avatar batch, and skip hydration for that render update. If
   the session remains live but that field advanced, perform the same clear,
   record the fence, and discard batch events whose sequence is at or below it.
2. Apply remaining Remote Avatar events in strictly increasing sequence order.
   A repeated/out-of-order sequence is ignored and recorded only as a bounded
   local presentation diagnostic; it cannot mutate a marker.
3. If no event establishes newer truth but `snapshot.remote_avatar` has a
   `source_sequence` greater than `last_event_sequence`, hydrate `Present`
   exactly at the snapshot's Realm-observed pose. This handles bridge startup
   without inventing intermediate movement.

The fence is the only overflow fallback. It prevents queued pre-failure
`Created`/`Updated` events from recreating a marker after the session has
already cleared its snapshot. A fresh recovered session has a strictly larger
event sequence and may establish a new marker normally.

## Marker lifecycle and visual language

There is at most one `RemoteAvatarRoot` entity tree. `Created` or snapshot
hydration spawns it at the exact Realm-observed pose with:

- a warm-amber faceted diamond on a short pedestal;
- a separate amber forward arrow, rotated only from the Remote Avatar heading;
- a ground-facing `REMOTE 0x…` GUID-shorthand label; and
- no model, name, numeric display ID, NPC/object representation, or WoW asset.

The existing cool-cyan capsule, ring, submitted marker, observed marker, and
camera retain their controlled-character meaning unchanged. Shape, separate
label, and heading arrow distinguish the remote marker without depending only
on colour. The Remote Avatar root uses its own components and queries, so it
cannot be mistaken for `RenderedAvatar` or one of the local diagnostic markers.

`Removed`, `Faulted`, a failed/offline snapshot, map-context unavailability,
or a fence advance recursively despawns the complete Remote Avatar tree and
clears both Remote Avatar pose rows. `Faulted` then keeps the bounded
`Fault { id, category }` inspector state in red until a later `Created` or an
ordinary session reset; it never leaves a world marker. An unexpected map
mismatch is hidden rather than projected and is visibly labelled `PROJECTION
SNAP / MAP CONTEXT UNAVAILABLE`, not misreported as a Realm fault. Ticket 14's
session boundary remains responsible for actual Remote Avatar fault categories.
It enters `MapContextUnavailable { id }`, which retains neither pose. On a
later render update, a matching-map `snapshot.remote_avatar` may hydrate the
marker exactly. A matching-ID, matching-map `Updated` event also immediately
rehydrates the root exactly at its Realm-observed pose (`Snap`) before later
updates can smooth it. Otherwise the state persists until a later `Created`, a
fault/removal, or an ordinary session reset.

## Projection mechanics

All Remote Avatar scene coordinates use the authenticated local entry anchor
and the existing world-pose-to-scene mapping. A remote pose whose map differs
from that anchor is never rendered. A `Created` always sets both observed and
rendered poses exactly to the received pose (`Snap`).

For an `Updated` event on the same accepted GUID:

| Condition | Result |
| --- | --- |
| map differs or entry anchor is absent | hide the marker and show `PROJECTION SNAP / MAP CONTEXT UNAVAILABLE` |
| same map and planar observed-to-rendered delta is `>= 1.628 m` | snap rendered pose to observed and display `PROJECTION SNAP` |
| same map and delta `< 1.628 m` | retain observed as the target and smoothly move rendered toward it at `8.0 m/s`, clamped not to overshoot; display `PROJECTION SMOOTH` |

`1.628 m` is the Ticket 09 calibrated boundary. The 8.0 m/s rate is a fixed
deterministic visual convergence rate, not a network, latency, velocity, or
prediction claim. A newer update retargets from the current rendered pose; it
does not extrapolate from timestamps or prior velocity. For each render delta
`dt`, let `d` be the planar rendered-to-observed distance and set
`blend = 1` when `d == 0`, otherwise `min(8 * max(dt, 0) / d, 1)`. Position
advances by its delta times `blend`; heading advances by the shortest
normalized angular delta times that exact same `blend`, then normalizes again.
Thus a change across the `-pi`/`pi` boundary cannot spin the marker the long
way around, and the short-arc test has a deterministic oracle. The renderer
may only converge to the latest explicit Realm-observed pose.

## Inspector, events, and diagnostics

The existing right inspector gains a separate `REMOTE AVATAR` block; the local
`IDENTITY & POSES` rows remain unchanged.

| Presentation state | Inspector | World |
| --- | --- | --- |
| `Absent` | `REMOTE AVATAR / ABSENT`; no remote pose values | no remote entity |
| `Present` | GUID shorthand; `PRESENT`; exact `REALM-OBSERVED`; private `RENDERED`; `PROJECTION SMOOTH` or `SNAP` | one diamond/pedestal/arrow/label tree |
| `Fault` | red `REMOTE AVATAR FAULT`; GUID shorthand and redacted category only; no pose values | no remote entity |
| `MapContextUnavailable` | `REMOTE AVATAR / PROJECTION SNAP / MAP CONTEXT UNAVAILABLE`; GUID shorthand and no pose values | no remote entity |

The existing short semantic-event tail formats remote events as sequence plus
`Created`/`Updated`/`Removed`/`Faulted`, GUID shorthand, and removal/fault
category. It never formats remote packet opcodes, raw fields, names, account
data, credentials, paths, cipher material, locally submitted poses, or the
private rendered pose. A capacity outcome for another GUID is intentionally
not a presentation event.

## Verification contract

Pure presentation tests and a headless Bevy schedule test must prove:

1. `Created` makes one root at the exact observed pose; a sub-`1.628 m`
   update smooths without overshoot or prediction; an update exactly at the
   boundary snaps; and heading chooses the short arc.
2. `Removed`, `Faulted`, map-context unavailability, and ordinary session reset
   despawn the root and clear both pose values. A fault retains only its
   redacted state/category. A later matching-map update from
   `MapContextUnavailable` rehydrates exactly, while a mismatched update does
   not.
3. A saturated-session snapshot/fence clears the root before queued old events
   are considered; those events cannot resurrect it, while a strictly newer
   recovered create can. A normal `Failed`/`Offline` snapshot with a queued
   pre-failure create/update likewise discards the whole batch and cannot
   recreate a marker.
4. A single ingress drain containing more than eight Remote Avatar transitions
   reaches projection in sequence even though the visible diagnostic tail
   retains only eight entries.
5. Local capsule/camera/prediction/Submitted Pose/controlled-character
   Realm-observed Pose stay unchanged under every remote event. Inspector and
   event text expose only approved semantic fields.

Visual/capture verification belongs to the later role-reversed live proof. It
must show two independent windows, each with one cyan local capsule and at
most one amber Remote Avatar marker, but it remains additional to semantic
sidecar evidence.

## Deferred

- Models, terrain, animation, generic player lists, arbitrary object rendering,
  name queries, numeric display/model fields, and authored gameplay UI.
- More than one Remote Avatar, remote prediction, map transfer, peer reconnect
  policy, capture orchestration, and the manual two-window acceptance step.
- Windows render/runtime acceptance.
