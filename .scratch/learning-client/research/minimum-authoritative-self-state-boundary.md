# Minimum authoritative self-state boundary

Research date: 2026-07-22

## Answer in one sentence

Before movement is enabled, the World-entry Slice must bounded-inflate AzerothCore's compressed object update, structurally walk its update blocks, confirm exactly one selected player `CreateObject2` carrying `SELF | LIVING`, decode that block's AzerothCore-layout movement snapshot and nine absolute speeds, retain its run speed and control flags while skipping the update-field values generically, then keep run speed current through the force-run-speed change/ack pair; all movement-bearing packets use a project-owned AzerothCore `MovementInfo` codec rather than the incompatible generated `wow_world_messages` representation.

## Source baseline and compatibility authority

This decision retains the same pins as the broader protocol trace:

| Source | Pin | Role |
| --- | --- | --- |
| [AzerothCore `azerothcore-wotlk`](https://github.com/azerothcore/azerothcore-wotlk/tree/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f) | `a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f` | Authoritative wire behavior of the Reference Realm |
| [`wow_messages`](https://github.com/gtker/wow_messages/tree/e1c9e15a8b94fce76cbd433cf49bd67c376a99d7) | `e1c9e15a8b94fce76cbd433cf49bd67c376a99d7`; `wow_world_messages` 0.3.0 | Independent schema evidence and candidate dependency, except where it disagrees with AzerothCore |

AzerothCore's GPL-covered implementation is evidence, not code to copy into the Learning Client. The client owns an independent byte-level implementation of the facts below. The MIT/Apache-licensed `wow_messages` crate may still be used for unaffected messages behind the protocol crate, but AzerothCore wins every compatibility disagreement.

## The minimum bootstrap invariant

A normal Reference Realm player inherits `UPDATEFLAG_LIVING | UPDATEFLAG_STATIONARY_POSITION` from `Unit` ([unit construction](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Entities/Unit/Unit.cpp#L333-L365)). When AzerothCore builds the update for the player themselves, it adds `UPDATEFLAG_SELF`, selects `UPDATETYPE_CREATE_OBJECT2` for a player, writes the packed GUID and player type, then appends the movement block and values block ([create construction](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Entities/Object/Object.cpp#L178-L232), [flag values](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Entities/Object/Updates/UpdateData.h#L26-L49)). For the controlled, non-vehicle fixture, the self block invariant is therefore:

```text
update_type  = 3                 # CreateObject2
guid         = selected character GUID
object_type  = 4                 # Player
update_flags = 0x0061            # SELF | LIVING | STATIONARY_POSITION
```

The numeric player type is fixed by AzerothCore's client object type enum ([`TYPEID_PLAYER`](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Entities/Object/ObjectGuid.h#L30-L40)).

The `STATIONARY_POSITION` bit does not add a second position here because AzerothCore takes the `LIVING` branch first. The self block's living movement data is the authoritative initial control snapshot. `SMSG_LOGIN_VERIFY_WORLD` remains the world-entry gate and supplies the map plus Entry Anchor; the later self block must identify the same selected GUID and its pose must agree with the entry pose within the verification tolerance before input is enabled. Its server timestamp is diagnostic ordering evidence, not a replacement for client time synchronization.

AzerothCore sets its server-side mover to the player before adding the player to the map, then sends visibility—including the self create—before time synchronization and the explicit no-flight state ([login initialization order](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Entities/Player/Player.cpp#L11754-L11838)). There is no server `SET_ACTIVE_MOVER` packet to await. `CMSG_SET_ACTIVE_MOVER` is only a client assertion checked against the mover AzerothCore already chose, so it is not an authority signal and is omitted from the minimum slice.

## Object-update decoder boundary

The protocol crate must accept both `SMSG_UPDATE_OBJECT` (`0x0A9`) and `SMSG_COMPRESSED_UPDATE_OBJECT` (`0x1F6`). The compressed form is mandatory in practice: AzerothCore changes every `SMSG_UPDATE_OBJECT` larger than 100 bytes into the compressed opcode, prefixing the zlib stream with the original body size ([compression threshold](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Server/WorldSocket.h#L32-L44), [wire transformation](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Server/WorldSocket.cpp#L98-L118)). A player create with its values mask exceeds that threshold.

The decompressor must:

- reject a declared size above the server-frame/project cap before allocating;
- stream into an output limited to that declared size;
- require the actual inflated size to equal the declaration;
- reject truncated, trailing, nested, or second-stream data; and
- pass the inflated body directly to the same update-object parser—the stream contains the original body, not another world header or opcode.

Do not call the pinned generated compressed parser directly on untrusted bytes. It allocates from the wire-declared `u32`, reads zlib data to completion with `unwrap`, and does not enforce the declared output length ([generated decompressor](https://github.com/gtker/wow_messages/blob/e1c9e15a8b94fce76cbd433cf49bd67c376a99d7/wow_world_messages/src/world/wrath/smsg_compressed_update_object.rs#L24-L38)).

After decompression, implement a selective structural cursor rather than a player update-field model:

1. Read the `u32` update-block count.
2. Consume every block in order using update types `VALUES`, `MOVEMENT`, `CREATE_OBJECT`, `CREATE_OBJECT2`, `OUT_OF_RANGE_OBJECTS`, and `NEAR_OBJECTS`.
3. For create/movement blocks, structurally consume all flag-selected movement and spline fields needed to find the next block. Materialize semantic data only for the selected GUID's `SELF | LIVING` player block.
4. Consume a values section as `u8 mask_word_count`, that many `u32` mask words, then one opaque `u32` value for every set bit. Discard those values. AzerothCore constructs exactly that mask-plus-set-values representation ([values construction](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Entities/Object/Object.cpp#L486-L516)).
5. Require exactly one self block, the selected GUID, player object type, `LIVING`, finite pose/speeds, and complete body consumption. Reject duplicates, mismatches, malformed masks, unsupported flags, or leftover bytes with opcode and byte-offset diagnostics.

This deliberately parses the update container and movement structure but does not name, store, or expose `OBJECT_FIELD_*`, `UNIT_FIELD_*`, or `PLAYER_FIELD_*` values.

## AzerothCore `MovementInfo` wire type

All movement-bearing messages share one project-owned protocol type. Keep the two flag words separate; do not merge them into a synthetic wider flag set:

```text
u32 flags
u16 flags2
u32 timestamp_ms
f32 x
f32 y
f32 z
f32 orientation

if flags has ONTRANSPORT:
    PackedGuid transport_guid
    f32 transport_x, transport_y, transport_z, transport_orientation
    u32 transport_time
    i8  transport_seat
    if flags2 has INTERPOLATED_MOVEMENT: u32 transport_time2

if flags has SWIMMING or FLYING, or flags2 has ALWAYS_ALLOW_PITCHING:
    f32 pitch

u32 fall_time_ms

if flags has FALLING:
    f32 jump_z_speed
    f32 jump_sin_angle
    f32 jump_cos_angle
    f32 jump_xy_speed

if flags has SPLINE_ELEVATION:
    f32 spline_elevation
```

This is the exact order AzerothCore writes for server movement snapshots and reads from client movement packets ([server writer](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Entities/Unit/Unit.cpp#L15832-L15882), [server reader](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Server/WorldSession.cpp#L1067-L1101)). The controlled slice accepts only a finite ordinary-ground self state: no transport, flight, swimming, fall, spline, root, hover, water-walk, or other special movement/control state. The structural decoder still consumes conditionals correctly so a rejected state produces a semantic diagnostic rather than corrupting the packet cursor.

For outgoing ground movement and control acknowledgements, set `flags2 = 0`, omit every conditional section, and use `fall_time_ms = 0`. Ordinary forward movement sets only `FORWARD`; stopped movement uses no movement flags.

## Authoritative ground speed and control packets

Immediately after `MovementInfo`, a living create block contains nine absolute `f32` speeds in this wire order:

```text
walk, run, run_back, swim, swim_back,
flight, flight_back, turn_rate, pitch_rate
```

AzerothCore writes the current absolute values, not speed multipliers ([living update](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Entities/Object/Object.cpp#L332-L365)); the normal player defaults happen to be walk `2.5` and run `7.0`, but aura and state modifiers can change them ([player base speeds and calculation](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Entities/Unit/Unit.cpp#L92-L103)). Decode all nine to preserve framing. The World-entry domain retains only `run_speed` as its current forward-ground limit and must never hard-code `7.0`; walking, backward, swim, flight, turn, and pitch behavior remain out of this slice.

The minimum speed/control behavior is:

| Packet | Treatment |
| --- | --- |
| Self `CreateObject2` living block | Required before input. Confirms selected/controlled GUID, corroborates the entry pose, supplies current movement flags and absolute run speed. |
| `SMSG_FORCE_RUN_SPEED_CHANGE` | Parse packed active GUID, server order counter, required zero byte, and new absolute run speed. Require the controlled GUID and a finite positive speed, update the current run speed, then send `CMSG_FORCE_RUN_SPEED_CHANGE_ACK` with the same counter, current AzerothCore-layout `MovementInfo`, and the exact new speed. AzerothCore's packet writer and ACK validation are the authority ([writer](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Entities/Unit/Unit.cpp#L11306-L11321), [ACK handler](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Handlers/MovementHandler.cpp#L684-L783)). |
| `SMSG_MOVE_UNSET_CAN_FLY` | Retain the already-decided behavior: require the controlled GUID, record the order counter, and send `CMSG_MOVE_SET_CAN_FLY_ACK` with `applied = false` and the current AzerothCore-layout ground movement state. |
| `SMSG_TIME_SYNC_REQ` | Retain the already-decided counter response. Movement remains disabled until the initial response and no-flight ACK have been sent. |
| Walk/back/swim/flight/turn/pitch speed changes; root/unroot, hover, water-walk, feather-fall, collision-height, vehicle, and transport control | Not implemented for the controlled World-entry happy path. If received for the selected mover, stop movement and surface `UnsupportedSelfControlState` instead of guessing or silently acknowledging. |
| `CMSG_SET_ACTIVE_MOVER` | Omit. It supplies no server observation and AzerothCore already chose the mover before the self create. |

Movement input becomes available only after `SMSG_LOGIN_VERIFY_WORLD`, the valid self living block, a finite positive run speed, the first completed time-sync response, and the no-flight acknowledgement. The packet loop continues to accept run-speed changes while entered or moving.

## Handling the `wow_world_messages` discrepancies

The pinned generated Wrath `MovementInfo` and living `MovementBlock` schemas declare `fall_time` as `f32` and serialize falling data as `z_speed, cos_angle, sin_angle, xy_speed` ([standalone movement codec](https://github.com/gtker/wow_messages/blob/e1c9e15a8b94fce76cbd433cf49bd67c376a99d7/wow_world_messages/src/world/wrath/movement_info.rs#L31-L49), [standalone writer](https://github.com/gtker/wow_messages/blob/e1c9e15a8b94fce76cbd433cf49bd67c376a99d7/wow_world_messages/src/world/wrath/movement_info.rs#L116-L130), [living update writer](https://github.com/gtker/wow_messages/blob/e1c9e15a8b94fce76cbd433cf49bd67c376a99d7/wow_world_messages/src/world/wrath/movement_block.rs#L198-L212)). AzerothCore requires `u32 fall_time_ms` and `z_speed, sin_angle, cos_angle, xy_speed`.

The fields happen to occupy the same number of bytes, but bit-reinterpreting a timer as a float and swapping semantic names throughout the session/domain layer is too fragile. Therefore:

- `client_protocol` owns `AcoreMovementInfo` and its reader/writer;
- every movement message, self living block, can-fly ACK, and run-speed ACK uses that type;
- no generated `wow_world_messages::MovementInfo` or living movement value crosses the protocol boundary;
- do not fork the dependency for the first slice; an upstream fix can replace the local codec only after identical golden and Reference Realm tests pass; and
- treat the pinned generated update-flag names at `0x0008`/`0x0010` as non-authoritative too: `wow_messages` calls them `LOW_GUID`/`HIGH_GUID`, while AzerothCore calls them `UNKNOWN`/`LOWGUID` ([generated names](https://github.com/gtker/wow_messages/blob/e1c9e15a8b94fce76cbd433cf49bd67c376a99d7/wow_world_base/src/inner/wrath/update_flag.rs#L1-L15), [AzerothCore names](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Entities/Object/Updates/UpdateData.h#L36-L49)). Their payload widths currently match, but the selective decoder uses AzerothCore names.

## Required proof before implementation claims this boundary

The verification contract should require:

1. Golden decode/encode tests with a non-zero `u32` fall timer whose bits would be nonsensical as a numeric float.
2. A falling sample with distinct sine and cosine values to catch field-order reversal in both directions.
3. Uncompressed and compressed self-create fixtures covering `CreateObject2`, `0x0061`, selected GUID/player type, base ground `MovementInfo`, nine speed slots, opaque values-mask skipping, exact decompressed length, and complete cursor consumption.
4. Malformed compressed inputs covering oversized declaration, truncated stream, output overrun/underrun, trailing data, duplicate self blocks, bad mask counts, and leftover bytes without panics or unbounded allocation.
5. A scripted `SMSG_FORCE_RUN_SPEED_CHANGE`/ACK round trip proving the counter, run-only zero byte, current movement snapshot, and exact speed.
6. A real Reference Realm login assertion that the selected character produces exactly one self living create, the self GUID matches enumeration, the pose agrees with `SMSG_LOGIN_VERIFY_WORLD`, the run speed is positive, and input remains gated until time/no-flight synchronization completes.

## Consequences for the next decisions

- The networked movement contract can use server-provided `run_speed` as the prediction/submission limit and can distinguish bootstrap realm evidence from ordinary unacknowledged Submitted Pose.
- The architecture needs a protocol-level selective update decoder and `AcoreMovementInfo`, but it does not need a game-object registry or update-field domain model.
- The verification contract now has exact golden-layout, decompression, malformed-input, speed-change, and live-realm assertions to adopt.
- Special locomotion/control states remain deliberate failures of the controlled slice, not silently ignored future features.
