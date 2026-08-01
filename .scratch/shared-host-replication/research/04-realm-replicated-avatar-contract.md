# Realm-replicated Avatar contract

Ticket: [04 – Define the Realm-replicated Avatar contract](../issues/04-define-realm-replicated-avatar-contract.md)

## Decision

The engine-independent World-session boundary owns the Realm GUID identity,
the Remote Avatar Lifecycle, and the latest raw Realm-observed Pose. The Bevy
Diagnostic World owns the Remote Pose Projection and its time-smoothed Rendered
Pose. A Rendered Pose is never returned through the session snapshot and never
serves as Realm evidence.

The first Shared-Host Multi-client Simulation accepts exactly one
Realm-replicated Avatar per client. The model remains GUID-keyed so that this
limit is an explicit acceptance constraint rather than an identity shortcut.

## Engine-independent session contract

The future `client_session` public surface introduces these semantic values;
their names describe the contract, not a generated wire type:

```text
RemoteAvatarId
  non-zero Realm GUID
  equality and lifecycle key only
  GUID shorthand is display-only; Character name is neither required nor used

RemoteAvatarSnapshot
  id: RemoteAvatarId
  realm_observed_pose: WorldPose
  source_sequence: u64

RemoteAvatarRemovalSource
  DestroyObject
  OutOfRange

RemoteAvatarFaultCategory
  InvalidPose
  UnsupportedMovement
  InconsistentLifecycle

RemoteAvatarEvent
  { sequence, change }

RemoteAvatarChange
  Created { id, realm_observed_pose }
  Updated { id, realm_observed_pose }
  Removed { id, source: RemoteAvatarRemovalSource }
  Faulted { id, category: RemoteAvatarFaultCategory }
```

`ClientSnapshot` later contains `remote_avatar: Option<RemoteAvatarSnapshot>`.
It is the latest complete World-session truth, not an event history. The
existing bounded, lossless semantic-event FIFO carries every lifecycle or pose
transition in order. Every `RemoteAvatarEvent.sequence` is the same monotonic
semantic-event sequence used by the FIFO; `source_sequence` identifies the
exact event that produced the latest Snapshot without exposing a packet,
cipher, or transport handle.

Create initializes both the Snapshot's identity and its Realm-observed Pose.
Update replaces only the raw Realm-observed Pose for the same ID. Destroy or a
matching out-of-range observation removes the Snapshot and emits `Removed`.
After frame and container integrity have been established, a semantically
unusable record for the accepted ID instead clears the Snapshot and emits the
lossless `Faulted` event with a redacted category. `Faulted` is distinct from
an ordinary Realm removal and is defined by Ticket 08.
An update before Create, an update for another GUID, and a removal for an
absent GUID cannot create, merge, or relabel a Remote Avatar. The precise
redacted diagnostic and marker-fault outcome for unusable records is owned by
Ticket 08.

While the first acceptance slot is occupied, a different eligible GUID is a
valid but ignored acceptance-capacity outcome: it cannot replace the accepted
ID, create a second marker, or mutate the Snapshot. The later Remote Avatar
Fault Boundary owns malformed or unusable records, not this valid one-avatar
limit. This contract deliberately does not grow a multi-avatar collection or
an arbitrary-object registry.

## Presentation contract

The Bevy-only projection stores a private `RemoteAvatarPresentation` keyed by
the `RemoteAvatarId` from the latest Snapshot/Event. It has:

```text
id: RemoteAvatarId
realm_observed_pose: WorldPose     # copied verbatim from session evidence
rendered_pose: WorldPose           # presentation state only
```

- On `Created`, `rendered_pose` starts exactly at `realm_observed_pose`.
- On `Updated`, the projection may smooth from its current Rendered Pose toward
  the new Realm-observed Pose. It never extrapolates, simulates velocity, or
  creates a local prediction.
- On map change or a later-defined large correction, it snaps instead of
  smoothing. Ticket 09 measures timing and tolerance; Ticket 15 decides the
  presentation mechanics.
- On `Removed`, the Remote Avatar Marker is despawned and both presentation
  poses are discarded. There is no lingering marker or inferred final pose.

The Diagnostic World may display a GUID shorthand and a heading indicator. A
Character name, model, display ID, values-mask field, local Submitted Pose, or
controlled-character Predicted Pose must not affect Remote Avatar identity,
lifecycle, Realm-observed Pose, or Rendered Pose.

## Ordering and backpressure

The retained World worker is the sole writer of `RemoteAvatarEvent` and the
Remote Avatar portion of `ClientSnapshot`. It publishes the snapshot revision
after applying the same transition that it enqueues as an event. The Bevy
bridge consumes events in sequence order and may refresh the latest snapshot;
it never writes back into session state.

Event-FIFO saturation is a visible, fail-closed boundary just like other
semantic client events. Coalescing may be considered only by Ticket 14 after
it can prove that a lifecycle edge cannot be lost. In particular, Create and
Removed are lossless and Update cannot be silently reclassified as a local
Rendered-Pose transition.

## Explicit deferrals

- More than one accepted Remote Avatar, name-query caching, model/display
  metadata, NPCs, pets, and arbitrary objects.
- Teleport, map-transition, peer reconnect/crash, interpolation cadence,
  timeout, correction threshold, and Remote Avatar fault details.
- Remote movement prediction, collision, terrain, persistence, and any claim
  that an observed pose means a server-side movement acknowledgement.

## Verification required by later implementation

1. A scripted contract test proves Create → Update → Removed for one stable
   GUID, with each Update carrying a finite Realm-observed Pose.
2. It proves a second eligible GUID is ignored without replacing the accepted
   ID, and that stale or unmatched transitions do not create a marker.
3. A projection test proves Rendered Pose can change only toward an explicit
   Remote Avatar Realm-observed Pose, never from local movement truth.
4. A removal test proves both the Snapshot entry and Bevy marker disappear;
   no stale pose remains available after `Removed`.
5. Public formatting and sidecars contain only the GUID shorthand and semantic
   fields, never names unless a later name-query contract explicitly permits
   them, and never packet or credential material.
