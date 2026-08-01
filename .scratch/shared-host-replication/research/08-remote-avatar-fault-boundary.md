# Remote Avatar fault boundary

Ticket: [08 – Define the Remote Avatar fault boundary](../issues/08-define-remote-avatar-fault-boundary.md)

## Decision

The retained World worker has two deliberately separate failure boundaries. It
fails the entire World session whenever it cannot establish encrypted-frame or
update-container integrity. Once it has recovered a complete, bounded,
structurally aligned World frame, an unusable record for one accepted Remote
Avatar removes only that marker and emits a redacted Remote Avatar fault.

The boundary never invents a remote pose, retains a stale marker silently, or
tries to recover cipher state. A peer crash or reconnect policy is not implied
by a marker fault and remains deferred.

## Session-integrity failures

The worker stops remote projection and enters the existing recoverable session
failure boundary when any of these occurs:

| Failure class | Examples | Required outcome |
| --- | --- | --- |
| Encrypted-frame integrity | incomplete or malformed encrypted header at EOF, impossible frame length, incremental-decoder overflow, cipher/header alignment failure | fail the whole World session; do not process later bytes or retain any new remote truth |
| Container integrity | bounded decompression failure, declared decompressed-size violation, invalid update-block count or length, or a block stream that cannot be consumed exactly to its declared end | fail the whole World session; do not skip forward or guess the next block |
| Self/control safety | a malformed or unsupported self-control record at an existing World-session safety boundary | retain the existing whole-session fail-closed behavior; it is not a Remote Avatar marker fault |

The decoder commits cipher progress only for recovered bytes according to its
existing incremental contract. This decision explicitly forbids resynchronizing
by scanning encrypted bytes, discarding a partial payload, or continuing after
an unwalkable update container.

## Per-Remote-Avatar faults

After a frame and its enclosing update container have been fully and safely
decoded, the following facts concern only one currently accepted
`RemoteAvatarId`:

| Record fact | Required outcome |
| --- | --- |
| A create or movement record for the accepted GUID lacks a complete pose, has non-finite position/orientation, or uses a movement form outside ordinary-ground support | remove the marker and snapshot; append one redacted `RemoteAvatarFault` diagnostic |
| An update for the accepted GUID is internally inconsistent with the accepted lifecycle or identity | remove the marker and snapshot; append one redacted fault |
| A removal record for the accepted GUID is structurally valid | emit the ordinary `Removed` event; this is not a fault |
| A per-GUID record cannot be semantically materialized but its remaining declared structure is still fully consumable | consume it, fault only the affected accepted marker if it is that GUID, then continue with the next complete frame |

Fault removal atomically clears the latest Remote Avatar Snapshot and both
presentation poses, then publishes the lossless, ordered `Faulted { id,
category }` event from the Realm-replicated Avatar contract. It is not a
`Removed` event, so a valid Realm removal remains distinguishable. The
diagnostic exposes only its stable category (`invalid-pose`,
`unsupported-movement`, or `inconsistent-lifecycle`) plus GUID shorthand. It
must never include packet bytes, opcodes, values masks, accounts, credential
material, paths, or cipher/session material.

No stale pose, marker, or rendered interpolation remains after the fault. A
later well-formed `Created` event is eligible to establish a new marker through
the normal lifecycle contract; this does not create a peer-reconnect claim.

## Valid but ignored data

The following is neither a session failure nor a Remote Avatar fault after
complete structural consumption:

- A supported record for a different GUID while the one accepted-avatar slot is
  occupied. It is the already-defined capacity outcome and cannot replace or
  mutate the accepted marker.
- A remote movement record before that GUID has become the accepted avatar, an
  unmatched removal, or an update for an absent GUID. It cannot create or
  relabel a marker.
- A complete supported record for an NPC, pet, game object, values update, near
  list, or other out-of-scope object. It is consumed solely to preserve stream
  alignment; no general object registry is created.

## Boundary order

```text
encrypted bytes
  -> complete framed message?       no: retain pending bytes / fail at limit or EOF
  -> bounded, exact container walk? no: World session failure
  -> record has accepted GUID?      no: consume and ignore as applicable
  -> semantic Remote Avatar usable? yes: Created / Updated / Removed
                                     no: remove only that marker + redacted fault
```

`RemoteAvatarFault` is a semantic diagnostic, not a wire type. `Faulted` is
the ordered lifecycle event carrying its category. The public Snapshot remains
`None` after marker removal; presentation uses the existing `FAULT` panel only
while a bounded diagnostic is available.

## Explicit deferrals

- Peer crash, reconnect, timeout, and reconnect/replacement policy.
- Support for teleport, map transfer, transport, flight, swimming, falling, or
  other movement forms beyond the ordinary-ground first acceptance boundary.
- A generalized bad-record quarantine, multi-avatar collection, arbitrary
  object model, name query, or raw packet diagnostics.

## Verification required by later implementation

1. Fragmented/corrupted encrypted headers, frame limits, inflate limits, and
   unwalkable update containers fail the retained World session without cipher
   recovery or a later-frame claim.
2. Scripted accepted-avatar records with non-finite pose, unsupported movement,
   or lifecycle inconsistency clear exactly that marker, poses, and projection,
   while producing only redacted category/GUID-shorthand diagnostics.
3. A valid second GUID, unmatched removal, absent-GUID movement, and supported
   out-of-scope object are fully consumed and leave the accepted marker and
   session alive.
4. A valid `Removed` event is distinguishable from a lossless `Faulted` event,
   and a later valid create can establish a marker without claiming
   peer-reconnect support.
