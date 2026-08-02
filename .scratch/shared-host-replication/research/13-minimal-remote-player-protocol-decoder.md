# Minimal remote-player protocol decoder

Ticket: [13 – Define the minimal remote-player protocol decoder](../issues/13-define-minimal-remote-protocol-decoder.md)

## Decision

The first production boundary is a protocol-owned, stateless
`RemotePlayerDecoder` in `client_protocol`. It receives only complete plaintext
`WorldServerFrame` values from the existing directional incremental World-frame
decoder and emits a bounded sequence of semantic `RemotePlayerRecord` values:

```text
PlayerCreate { guid, pose }
PlayerMovement { guid, pose, opcode }
OutOfRange { guid }
Destroy { guid }
```

`guid` is the sole identity. `pose` is a finite ordinary-ground
`AcoreMovementInfo`; it has no map field, display name, model, values-mask
field, raw payload, or cipher material. The retained session attaches the
already-authenticated `LOGIN_VERIFY_WORLD` map to a newly accepted record; it
must never derive or relabel a map from peer data. A later session boundary
chooses the one accepted GUID and converts these records into lifecycle events.
This decoder owns no accepted-avatar slot, object registry, presentation state,
socket, cipher, or credential.

The existing `decode_remote_world_trace` and `RemoteWorldTraceEvent` are the
research tracer to replace or narrow behind this same internal boundary. Their
current structural walker and `consume_update_mask` behavior are the migration
starting point, not an authorization to expose a general object decoder.

## Frame and alignment boundary

```text
encrypted byte stream
  -> IncrementalWorldServerDecoder produces one complete plaintext frame
  -> RemotePlayerDecoder structurally consumes that frame
  -> retained World session selects one GUID and publishes semantic events
```

The remote decoder never receives arbitrary byte chunks and never performs
header preview, header decryption, or payload-length recovery. Until the
incremental frame decoder yields a complete frame, no remote record exists and
an otherwise unsupported opcode is not skipped. This preserves cipher position
across fragmentation, coalescing, and ignored frames by construction.

At this boundary, these exact opcodes are relevant:

| Input | Accepted semantic output | Required consumption |
| --- | --- | --- |
| `SMSG_UPDATE_OBJECT` | create, movement, out-of-range records | Walk every declared block exactly. |
| `SMSG_COMPRESSED_UPDATE_OBJECT` | as above | Apply the existing bounded exact inflate before walking. |
| Server `MSG_MOVE_START_FORWARD`, `MSG_MOVE_HEARTBEAT`, `MSG_MOVE_STOP` | GUID-prefixed ordinary-ground movement | Decode the complete `MovementInfo` and require no trailing bytes. |
| `SMSG_DESTROY_OBJECT` | destroy | Decode full GUID plus death byte and require no trailing bytes. |
| Any other complete World frame | no record | Ignore only after framing; it has no decoder-side effect. |

For update containers, the walker must continue to consume `VALUES`,
`MOVEMENT`, `CREATE_OBJECT`, `CREATE_OBJECT2`, `OUT_OF_RANGE_OBJECTS`, and
`NEAR_OBJECTS` in wire order. It emits a create only for non-self
`CREATE_OBJECT2`, `OBJECT_TYPE_PLAYER`, with `LIVING | STATIONARY_POSITION`
and a finite ordinary-ground movement. It emits an in-container movement only
after a complete finite ordinary-ground movement block. All values masks and
their selected values are consumed but never interpreted or retained. A valid
non-player, self player, other GUID, near list, or unmatched removal is
therefore structurally harmless and leaves selection to Ticket 14.

## Metadata limit

The word “display” in this ticket is limited to the presentation-safe GUID
shorthand derived by Ticket 15. The decoder deliberately yields no display
metadata: neither names nor numeric display/model fields. Names need a later
bounded name-query exchange, and opaque update values remain out of scope.

## Error policy

Errors are classified at the first boundary capable of deciding them:

| Condition | Owner and outcome |
| --- | --- |
| Fragmented header/payload, EOF with pending encrypted bytes, frame-size limit, or cipher alignment problem | The incremental World-frame decoder fails the World session. The remote decoder is not called with a partial frame. |
| Inflate limit/failure, invalid update count/list count, malformed packed GUID, truncated field, unsupported update type, or unconsumed trailing bytes in a relevant container | `RemotePlayerDecoder` returns `ProtocolError`; the retained World session fails closed. No scan or recovery is allowed. |
| Complete, structurally consumable record with non-finite or unsupported movement | Emit an internal per-GUID unusable-record result, not a pose. Ticket 14 faults only the already accepted matching avatar with Ticket 08's redacted category; other GUIDs remain ignored. |
| Valid complete data outside this small contract | Consume exactly and emit no record. |

The unusable-record result contains only GUID and one stable category
(`invalid-pose` or `unsupported-movement`); it carries no raw packet content,
values, account data, path, opcode diagnostic, or cipher state. Lifecycle
inconsistency is not guessed by this stateless decoder: it is detected by the
GUID-owning session boundary and uses the existing `inconsistent-lifecycle`
fault category.

## Implementation seam

`client_protocol` exposes one deliberately narrow public workspace seam because
the retained worker lives in the separate `client_session` crate:

```rust
pub fn decode_remote_player_frame(
    frame: &WorldServerFrame,
) -> Result<Vec<RemotePlayerRecord>, ProtocolError>;
```

`RemotePlayerRecord` and its redacted unusable-record counterpart contain only
the fields defined above. No generic cursor, object field, value mask, frame
decoder, or credential type crosses this seam. The retained worker invokes this
function after every completed inbound frame and owns accepted-GUID lifecycle
state. `WorldServerFrame` has no public field constructor, so production callers
cannot provide a partial header or payload through this seam. On its first
accepted create the worker must copy the current map ID from its authenticated
entry state; a worker without that map context must fail before publishing a
Remote Avatar record. Subsequent updates retain that fixed map until a
separately supported map-transfer lifecycle exists. Thus neither the decoder
nor the session infers a peer map. This preserves the separation between
Realm-observed pose, local prediction, submitted pose, and the later
Bevy-rendered pose.

The implementation must not use “previous create seen” inside the protocol
decoder as a hidden registry. A movement before the session accepts that GUID
is decoded and then ignored; a create/movement/removal ordering fault is made
only once the session has selected its one peer.

## Fixture and test provenance

Decoder tests use deterministic, checked-in synthetic plaintext World-frame
bodies built from project-owned builders. They are not captures and contain no
credentials, session keys, encrypted frames, raw production payloads, player
names, or display/model values. The fixtures cite the pinned build-12340
AzerothCore source recorded in the Ticket 03 research, while the local semantic
transcript remains behavioral provenance only.

The test matrix must cover:

1. A valid non-self player `CreateObject2`, each supported movement opcode,
   in-container movement, out-of-range, and destroy.
2. Multiple blocks and GUIDs in one container, coalesced complete frames, and
   valid ignored NPC/self/values/near/other-GUID records followed by a valid
   peer record, proving exact consumption.
3. Fragmented encrypted frames and arbitrary chunk boundaries through the
   existing incremental frame decoder, proving that the semantic decoder is
   invoked only once per complete frame and cipher alignment survives ignored
   frames.
4. Compressed bodies and all bounded malformed/container failures as
   whole-session `ProtocolError` cases.
5. Non-finite and non-ordinary-ground records as per-GUID unusable results;
   Ticket 14 tests separately prove that only a matching accepted avatar is
   faulted.
6. No decoded name/model/value field, no object registry, and no remote-pose
   prediction claim.

## Deferred

- Name-query cache, numeric display/model interpretation, and generic object
  decoding.
- NPCs, pets, game objects, multi-avatar storage, combat, chat, and all
  unsupported update fields.
- Teleport, map transfer, transport, flight, swimming, falling, and other
  movement forms.
- Accepted-avatar lifecycle, queueing, diagnostics publication, smoothing,
  rendering, live proof orchestration, and Windows runtime acceptance.
