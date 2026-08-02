# Remote Avatar session-event boundary

Ticket: [14 – Design the Remote Avatar session-event boundary](../issues/14-design-remote-session-event-boundary.md)

## Decision

The retained World worker is the sole owner and writer of one accepted remote
avatar. It consumes `RemotePlayerRecord` values from Ticket 13 and publishes
their semantic effect through the existing `WorkerBoundary`: the existing
lossless `ClientEvent` FIFO, its single monotonic `sequence`, and the
`ClientSnapshot` protected by the existing snapshot lock. No remote-specific
channel, socket handle, protocol cursor, generated type, or ECS value crosses
the public `client_session` API.

The public session vocabulary is:

```text
RemoteAvatarId                 non-zero Realm GUID; identity only
RemoteAvatarSnapshot {
  id: RemoteAvatarId,
  realm_observed_pose: WorldPose,
  source_sequence: u64,
}
RemoteAvatarRemovalSource      DestroyObject | OutOfRange
RemoteAvatarFaultCategory      InvalidPose | UnsupportedMovement | InconsistentLifecycle
RemoteAvatarChange {
  Created { id, realm_observed_pose },
  Updated { id, realm_observed_pose },
  Removed { id, source },
  Faulted { id, category },
}
ClientEventKind::RemoteAvatar { change: RemoteAvatarChange }
ClientSnapshot {
  remote_avatar: Option<RemoteAvatarSnapshot>,
  remote_avatar_invalidated_through: u64,
  ..
}
```

`RemoteAvatarId` rejects GUID zero at construction and offers a bounded
display-only hexadecimal shorthand. It has no name, model, account, or
credential-bearing field. `WorldPose` is assembled by the session from the
decoder's finite movement and the session's authenticated entry-map context;
the remote decoder never supplies a peer map. A missing entry map is a
whole-session safety failure before any Remote Avatar event can be published.
`remote_avatar_invalidated_through` is a session-owned monotonic semantic
sequence fence: it initializes to zero, changes only when a Remote Avatar
transition cannot be enqueued because the FIFO is saturated, and is then set
to the last successfully committed global event sequence in the same snapshot
revision that clears the Remote Avatar state. It never decreases or derives
from Bevy state.

## Acceptance state machine

The worker holds a private `accepted_remote_id: Option<RemoteAvatarId>`.
It is not a collection and is never exposed through a transport API.

| Decoded record | No accepted ID | Matching accepted ID | Different ID while occupied |
| --- | --- | --- | --- |
| eligible create | accept; publish `Created`; set snapshot | lifecycle fault; clear and publish `Faulted(InconsistentLifecycle)` | consume and ignore (one-avatar capacity) |
| ordinary-ground movement | consume and ignore | publish `Updated`; replace only raw Realm-observed pose | consume and ignore |
| `OutOfRange` / `Destroy` | consume and ignore | clear; publish `Removed` with exact source | consume and ignore |
| unusable record | consume and ignore | clear; publish the matching redacted `Faulted` category | consume and ignore |

Clearing on `Removed` or `Faulted` clears both `accepted_remote_id` and
`ClientSnapshot.remote_avatar`. A later valid create is eligible as a new
ordinary lifecycle; this does not claim peer reconnect support. A movement
before accepted create, an unmatched removal, or a second eligible GUID cannot
replace, merge, relabel, or mutate the selected Remote Avatar.

The decoder's structurally invalid frame/container errors never reach this
table: Ticket 08 retains their whole-session failure boundary. The only
per-avatar input is the safe, complete unusable-record result from Ticket 13.

## Atomic publication and ordering

For each non-ignored transition, the worker prepares the next event sequence,
the complete `RemoteAvatarChange`, and the next snapshot value under the
snapshot lock. It then attempts the FIFO write. Only a successful enqueue
commits the transition: increment the worker sequence, install the prepared
snapshot, and increment `snapshot_revision` before releasing the worker turn.
The event carries all change data, so a consumer does not need to read a
snapshot to interpret the event; a subsequent snapshot read has the same or a
newer revision.

The snapshot is latest-state only. On `Created` and `Updated`, its
`source_sequence` equals the exact `ClientEvent.sequence` that carried that
change. `Removed` and `Faulted` have no snapshot after their event. The existing
FIFO is therefore the only ordered history, and the snapshot can never be used
to recreate a removed/faulted marker or invent an intermediate pose.

Ticket 14 deliberately chooses **no update coalescing**. `Created`, `Updated`,
`Removed`, and `Faulted` are all lossless FIFO events at the fixed capacity of
64. If the FIFO is full, publication fails through the existing
`EventBackpressure` boundary: clear any remote snapshot and accepted ID, mark
the World session failed with the existing redacted backpressure diagnostic,
stop further projection, and require normal recovery. The worker must not drop,
overwrite, or reclassify a remote pose update to keep running. A future
coalescing design requires a separate proof that it preserves every lifecycle
edge and source-sequence meaning.

FIFO saturation cannot enqueue the terminal event that would ordinarily remove
a marker. Therefore the failure transition also sets
`remote_avatar_invalidated_through` to the last successfully committed global
semantic-event sequence, in the same snapshot revision that clears
`remote_avatar`. It is an invalidation fence, not a history or a wire fact.
Ticket 15's bridge must read the latest snapshot once at the start of every
render update, even when no event arrived. When the phase is failed/offline or
the fence advances, it clears all private remote presentation state and ignores
every queued `RemoteAvatar` event at or below that fence. A fresh post-recovery
Remote Avatar event has a strictly larger sequence and is eligible normally.
This required pull-side guard prevents queued pre-failure `Created`/`Updated`
events from resurrecting a stale marker after saturation.

## Separation from local and presentation truth

Remote transitions can mutate only `remote_avatar`, its private accepted ID,
and the bounded Remote Avatar event stream. They must not mutate
`entry_anchor`, `predicted_pose`, `submitted_pose`, `submitted_pose_is_stopped`,
`realm_observed_pose` (which remains controlled-character truth), correction
targets, movement proofs, local movement intent, or the local run speed.

Ticket 15's Bevy bridge receives these events in FIFO order and must refresh
the latest `remote_avatar` snapshot and invalidation fence once per render
update. It creates its own private rendered pose from the remote
Realm-observed pose. It never writes to this boundary, never feeds the local
prediction path, and cannot represent a Remote Avatar in a `ClientSnapshot`
once a removal, fault, or saturated-session failure is committed.

Diagnostics and later sidecars receive only the event category/source,
GUID shorthand, sequence, and finite pose where a `Created` or `Updated` event
already contains it. They must reject names, raw opcodes, payloads, values
masks, account/credential data, transport paths, session keys, and rendered or
locally submitted poses as remote evidence.

## Verification contract

Deterministic boundary tests, with a fake event receiver and no Docker or Bevy,
must prove:

1. Create → Update → Destroy and Create → Update → OutOfRange each yield
   ordered, consecutive event sequences; `source_sequence` points to the
   Create/Update event and snapshot is absent after removal.
2. A second eligible GUID, stale movement, unmatched removal, and safe
   out-of-scope record leave both the accepted snapshot and event queue
   unchanged.
3. Duplicate create, and each usable decoder fault category for the accepted
   GUID, atomically clear the remote snapshot and emit only `Faulted`; the same
   record for another GUID is ignored.
4. A missing authenticated map, malformed/container decoder error, and FIFO
   saturation fail closed and cannot leave a stale accepted ID or snapshot. In
   particular, fill the FIFO after a queued `Created`, saturate on a remote
   update, then prove the presentation fence clears the marker and rejects the
   queued pre-failure create/update events.
5. Local prediction, Submitted Pose, controlled-character Realm-observed Pose,
   and remote snapshot stay distinct under every remote transition; debug
   formatting and semantic sidecars contain no forbidden raw or secret fields.

## Deferred

- Bevy marker entity ownership, interpolation/snap mechanics, label layout,
  capture, and manual two-window presentation evidence (Ticket 15).
- Any coalescing, multiple accepted avatars, name queries, model/display data,
  general objects, or generic subscriptions.
- Teleport/map transfer, peer reconnect/crash policies, remote prediction,
  persistence, and Windows runtime acceptance.
