# Remote-player World-Update boundary

Research date: 2026-07-29

## Answer in one sentence

For the build-12340 Reference Realm, a visible peer is established by a player
`CreateObject2` block in `SMSG_{COMPRESSED_}UPDATE_OBJECT`, ordinary pose and
heading changes arrive as GUID-prefixed `MSG_MOVE_*` movement packets, and the
peer is removed by `SMSG_DESTROY_OBJECT` or an out-of-range GUID list.

## Provenance and retained evidence

| Evidence | Pin / observation | Use |
| --- | --- | --- |
| [AzerothCore source](https://github.com/azerothcore/azerothcore-wotlk/tree/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f) | `a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f`, also locked in [`infra/azerothcore/artifacts.lock`](../../../infra/azerothcore/artifacts.lock) | Compatibility authority for the local Reference Realm. |
| [Reference Realm multi-session behavior](01-reference-realm-multi-session-behavior.md) | Local reset-scoped measurement reached two concurrent `MovementReady` sessions with distinct GUIDs on map `0`. | Establishes the local same-map precondition. |
| Existing World-entry parser | [`crates/client_protocol/src/world_entry.rs`](../../../crates/client_protocol/src/world_entry.rs) | Already frames encrypted World messages, bounded-inflates update containers, and structurally consumes all six update-block kinds. |

No raw authenticated traffic, session keys, credentials, unredacted runtime
logs, or packet payloads are retained by this research. The transcript below
is a semantic source-and-local-environment analysis, not a replay fixture. A
future live experiment may retain only allowlisted semantic facts such as
opcode family, lifecycle kind, GUID relation, map id, finite pose, and elapsed
time.

## Semantic transcript

### Appearance

AzerothCore writes a visible-object create as update type, packed GUID, object
type, movement block, and values block
([writer](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Entities/Object/Object.cpp#L178-L232)).
For a normal player, `STATIONARY_POSITION` selects `UPDATETYPE_CREATE_OBJECT2`;
the player object type is `4`. `SELF` is added only when target and object are
the same player, so a peer is a non-self player create
([definitions](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Entities/Object/Updates/UpdateData.h#L26-L49)).

```text
SMSG_UPDATE_OBJECT or SMSG_COMPRESSED_UPDATE_OBJECT
  CreateObject2
    remote_guid: packed GUID                 # identity key
    object_type: Player (4)
    update_flags: LIVING | STATIONARY_POSITION (not SELF)
    movement: finite MovementInfo            # x, y, z, orientation, timestamp
    nine speeds: structurally consumed only
    values mask + values: structurally consumed only
```

The living movement information uses the established AzerothCore layout:
flags, flags2, timestamp, position, orientation, then flag-selected fields
([writer](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Entities/Unit/Unit.cpp#L15832-L15882)).
This creates the initial Realm-observed pose and heading. It does not carry a
map id: visibility means the peer is on the observer's currently entered map.

### Movement

The server receives a player's `MSG_MOVE_*` payload, synchronizes it, and
broadcasts the same opcode with `MovementInfo` to other visible clients
([movement handler](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Handlers/MovementHandler.cpp#L363-L429)).
It can also broadcast `MSG_MOVE_HEARTBEAT` with packed GUID and movement data
([heartbeat construction](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Entities/Unit/Unit.cpp#L2153-L2160)).

```text
MSG_MOVE_START_FORWARD | MSG_MOVE_HEARTBEAT | MSG_MOVE_STOP
  remote_guid: packed GUID
  movement: finite ordinary-ground MovementInfo
```

Only a packet whose actor GUID was already created can update the accepted
Remote Avatar. The timestamp is ordering evidence, never a local prediction
clock. A complete valid packet for another GUID is irrelevant but remains
properly framed and decoded.

### Removal

`SMSG_DESTROY_OBJECT` contains the full GUID and a one-byte death indicator
([writer](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Entities/Object/Object.cpp#L263-L286)).
It is the source-defined destruction record that the later local clean-logout
probe must observe. Visibility loss can instead occur in an
`OUT_OF_RANGE_OBJECTS` block, whose count and packed GUIDs precede the other
update blocks
([packet construction](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Entities/Object/Updates/UpdateData.cpp#L45-L66)).
Both remove only a matching marker; neither implies a new map, final pose,
disconnect reason, or persistence.

### Display metadata is not in this first boundary

The create's values mask may expose public numeric fields such as
`UNIT_FIELD_DISPLAYID`, but player names are returned only through the separate
`CMSG_NAME_QUERY` / `SMSG_NAME_QUERY_RESPONSE` exchange
([handler](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Handlers/QueryHandler.cpp#L32-L70)).
This ticket therefore authorizes neither a name nor a model decoder from opaque
values. The initial marker labels itself with a GUID shorthand. A later,
bounded name-query cache is a separate slice.

## Minimal decoding contract for Ticket 13

1. Feed every byte through the incremental encrypted World-frame decoder. Only
   after a complete frame is available may an opcode be ignored; never skip
   encrypted bytes or a partial header.
2. For `SMSG_UPDATE_OBJECT` and `SMSG_COMPRESSED_UPDATE_OBJECT`, retain bounded
   decompression and structurally walk `VALUES`, `MOVEMENT`, `CREATE_OBJECT`,
   `CREATE_OBJECT2`, `OUT_OF_RANGE_OBJECTS`, and `NEAR_OBJECTS` in order.
3. Materialize a create only for a non-self `Player` with finite,
   ordinary-ground living movement. The packed GUID is the sole identity key;
   consume all mask-selected values without exposing them.
4. Materialize movement only for an already tracked GUID after a complete,
   finite ordinary-ground `MovementInfo`. Ignore valid movement for another
   GUID after decoding it completely.
5. Remove a matching GUID on `SMSG_DESTROY_OBJECT` or a complete out-of-range
   list. Ignore unmatched removals.
6. Header decryption, frame length, and bounded decompression failures are
   session-framing failures. Once a complete frame has been recovered,
   unsupported or unusable data belonging to one remote GUID is a Remote Avatar
   fault for Ticket 08: remove that marker with a redacted diagnostic while
   preserving the already-established frame alignment. A structurally malformed
   update container that cannot be walked to its declared end remains a
   session-framing failure.

The project's existing `consume_update_mask` pattern is the safe model:
unrelated NPCs, pets, game objects, values updates, and near lists are fully
consumed so the next block remains aligned, but no general object registry is
created.

## Deferred boundaries

- General object, NPC, pet, combat, chat, group, inventory, quest, terrain,
  collision, animation, and Blizzard-asset support.
- Name-query protocol and numeric display/model field interpretation.
- Teleport/map-change, remote reconnect, peer-crash handling, and more than
  one accepted Remote Avatar per observer.
- Local prediction, reconciliation, or persistence claims for remote players.

## Required follow-up proof

The remaining completion step for this ticket is a reset-scoped Fixture Pair
probe that retains only allowlisted semantic events proving the order:

```text
remote CreateObject2 -> remote Start/Heartbeat/Stop -> remote DestroyObject
```

It must prove that `DestroyObject` actually follows the controlled clean
logout. Ticket 09 then uses the same capture boundary to set update-cadence and
removal timing tolerances for the exact local Fixture Pair.
