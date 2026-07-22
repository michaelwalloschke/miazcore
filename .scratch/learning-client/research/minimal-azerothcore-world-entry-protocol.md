# Minimal AzerothCore world-entry protocol

Research date: 2026-07-21

## Answer in one sentence

The World-entry Slice needs two TCP state machines: authenticate on the login socket with the 3.3.5a SRP6 exchange and obtain the realm endpoint, then authenticate a fresh world socket, enable encrypted packet headers, enumerate and log in the configured character, consume the login bootstrap, answer time and movement-control synchronization, and emit ground movement; ordinary accepted movement has no direct sender acknowledgement, so acceptance must be proven by a server-completed save followed by a reconnect whose `SMSG_LOGIN_VERIFY_WORLD` reports the moved position.

## Source baseline

The trace is pinned so later implementation can distinguish stable protocol facts from upstream drift:

| Source | Pin | Role | License |
| --- | --- | --- | --- |
| [AzerothCore `azerothcore-wotlk`](https://github.com/azerothcore/azerothcore-wotlk/tree/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f) | `a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f` | Authoritative behavior of the Reference Realm | [GNU GPL v2](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/LICENSE#L1-L7); source headers permit v2 or later |
| [`wow_messages`](https://github.com/gtker/wow_messages/tree/e1c9e15a8b94fce76cbd433cf49bd67c376a99d7) | `e1c9e15a8b94fce76cbd433cf49bd67c376a99d7`; `wow_login_messages` 0.5.0, `wow_world_messages` 0.3.0 | Independent client-side packet-layout cross-check | [MIT OR Apache-2.0](https://github.com/gtker/wow_messages/blob/e1c9e15a8b94fce76cbd433cf49bd67c376a99d7/Cargo.toml#L15-L19) |
| [`wow_srp`](https://github.com/gtker/wow_srp/tree/25ffab6433e1ee5eee629200cf42b592c1f36121) | `25ffab6433e1ee5eee629200cf42b592c1f36121`; 0.7.0 | Independent SRP6 and Wrath header-crypto cross-check | [MIT OR Apache-2.0](https://github.com/gtker/wow_srp/blob/25ffab6433e1ee5eee629200cf42b592c1f36121/Cargo.toml#L1-L12) |

AzerothCore is the compatibility authority where sources disagree. Protocol facts may be independently implemented, but GPL-covered implementation code must not be copied or adapted into the Learning Client without a deliberate licensing decision. Depending on or adapting the MIT/Apache-licensed Rust crates still requires choosing a license path and retaining the applicable notices. This is an engineering boundary, not legal advice.

## End-to-end state machine

| Phase | Direction | Message | Minimum result |
| --- | --- | --- | --- |
| Login challenge | client -> auth | `CMD_AUTH_LOGON_CHALLENGE_Client` `0x00` | Declare protocol 8, version 3.3.5 build 12340, platform/OS/locale, and normalized account name. |
| Login challenge | auth -> client | `CMD_AUTH_LOGON_CHALLENGE_Server` `0x00` | Receive SRP6 `B`, `g`, `N`, salt, CRC salt, and security flags. |
| Login proof | client -> auth | `CMD_AUTH_LOGON_PROOF_Client` `0x01` | Send `A`, `M1`, zero CRC proof for the controlled realm, zero telemetry keys, and no second factor. |
| Login proof | auth -> client | `CMD_AUTH_LOGON_PROOF_Server` `0x01` | Require success, verify `M2`, retain the 40-byte session key in memory. |
| Realm discovery | client -> auth | `CMD_REALM_LIST_Client` `0x10` | Send the four-byte zero request body. |
| Realm discovery | auth -> client | `CMD_REALM_LIST_Server` `0x10` | Select the configured online build-12340 realm and retain address plus realm ID. |
| World challenge | world -> client | `SMSG_AUTH_CHALLENGE` `0x1EC` | Receive the server seed from the fresh world TCP connection. |
| World proof | client -> world | `CMSG_AUTH_SESSION` `0x1ED` | Prove knowledge of the login session key; include a minimal addon block. |
| Header crypto | both | no packet | Initialize directional HMAC-SHA1-derived RC4-drop1024 streams; only subsequent world headers are encrypted. |
| World auth | world -> client | `SMSG_AUTH_RESPONSE` `0x1EE` | Decrypt and require `AUTH_OK`; queue form is not part of the controlled happy path. |
| Character discovery | client -> world | `CMSG_CHAR_ENUM` `0x037` | Empty body. This step is mandatory even if the character GUID was provisioned elsewhere. |
| Character discovery | world -> client | `SMSG_CHAR_ENUM` `0x03B` | Select the configured character and retain its GUID and coarse location. |
| Character login | client -> world | `CMSG_PLAYER_LOGIN` `0x03D` | Send the selected full 64-bit GUID. |
| World entry | world -> client | `SMSG_LOGIN_VERIFY_WORLD` `0x236` | Treat map plus `x/y/z/orientation` as the first definitive entered-world signal. |
| Control sync | world -> client | `SMSG_MOVE_UNSET_CAN_FLY` `0x344` | Retain packed player GUID and server order counter for the normal no-flight test character. |
| Control sync | client -> world | `CMSG_MOVE_SET_CAN_FLY_ACK` `0x345` | Return full GUID, counter, current ground `MovementInfo`, and `applied = false`. |
| Clock sync | world -> client | `SMSG_TIME_SYNC_REQ` `0x390` | Retain the server counter. |
| Clock sync | client -> world | `CMSG_TIME_SYNC_RESP` `0x391` | Echo the counter with monotonic client milliseconds. |
| Ground move | client -> world | `MSG_MOVE_START_FORWARD` `0x0B5` | Send the active mover GUID and current ground `MovementInfo` with forward flag. |
| Ground move | client -> world | `MSG_MOVE_HEARTBEAT` `0x0EE` | Send plausible timestamped position progress while forward remains set. |
| Ground move | client -> world | `MSG_MOVE_STOP` `0x0B7` | Send the final position with no movement flags. |

The login protocol structs and accepted command values are visible in AzerothCore's [auth packet declarations](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/apps/authserver/Server/AuthSession.cpp#L37-L102). The independent client layouts agree for the [initial challenge](https://github.com/gtker/wow_messages/blob/e1c9e15a8b94fce76cbd433cf49bd67c376a99d7/wow_login_messages/src/logon/all/cmd_auth_logon_challenge_client.rs#L10-L41), [login proof](https://github.com/gtker/wow_messages/blob/e1c9e15a8b94fce76cbd433cf49bd67c376a99d7/wow_login_messages/src/logon/version_8/cmd_auth_logon_proof_client.rs#L8-L37), and [realm-list response](https://github.com/gtker/wow_messages/blob/e1c9e15a8b94fce76cbd433cf49bd67c376a99d7/wow_login_messages/src/logon/version_8/cmd_realm_list_server.rs#L7-L20).

## Login authentication details

### Challenge request

Use protocol version `8`, semantic version `3.3.5a`, build `12340`, game name `\0WoW`, platform `x86`, and OS `OSX` for the first macOS client. The four-character platform, OS, and locale fields have the legacy wire byte order; use a validated codec rather than serializing display strings directly. Normalize the account name and password to the protocol's uppercase form before both transmission and hashing. The controlled account must have no TOTP secret, IP/country lock conflict, or ban.

AzerothCore records the build, OS, and locale from this packet and returns the SRP6 challenge only for an accepted build in [the challenge handler](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/apps/authserver/Server/AuthSession.cpp#L284-L315) and [challenge response](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/apps/authserver/Server/AuthSession.cpp#L388-L425). A configured TOTP secret changes the security flags and proof payload, so it is deliberately excluded.

### SRP6 proof

All SRP arrays are 32-byte, zero-padded little-endian values. For normalized username `U`, normalized password `P`, server public key `B`, salt `s`, generator `g = 7`, modulus `N`, and a random 32-byte private `a`:

```text
x  = SHA1(s || SHA1(U || ":" || P))
A  = g^a mod N
u  = SHA1(A || B)
S  = (B - 3 * g^x)^(a + u*x) mod N
K  = WoW-SHA1-interleave(S)             # 40 bytes
M1 = SHA1((SHA1(N) xor SHA1(g)) || SHA1(U) || s || A || B || K)
M2 = SHA1(A || M1 || K)
```

Generate fresh cryptographic randomness, reject invalid public values, send `A` and `M1`, and verify the returned `M2` before accepting the session key. The current server constants and verification are in AzerothCore's [SRP6 implementation](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/common/Cryptography/Authentication/SRP6.cpp#L26-L29) and [proof verification](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/common/Cryptography/Authentication/SRP6.cpp#L50-L111). `wow_srp` independently exposes the client transition and server-proof verification in [its typestate client](https://github.com/gtker/wow_srp/blob/25ffab6433e1ee5eee629200cf42b592c1f36121/src/client.rs#L133-L269).

With `StrictVersionCheck = 0`, the 20-byte CRC proof may be zero, but the build itself is still validated. Build `12340` is present in AzerothCore's [`build_info`](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/data/sql/base/db_auth/build_info.sql#L48-L55), while executable-integrity proof is disabled by the [default auth setting](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/apps/authserver/authserver.conf.dist#L158-L164). The successful proof stores the session key and returns `M2` in the [server proof path](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/apps/authserver/Server/AuthSession.cpp#L473-L555).

### Realm list

Although a single realm endpoint could be duplicated in client configuration, the realm-list round trip belongs in the slice: it proves auth completion, exposes realm availability/build flags, supplies the Docker-published world address, and gives the realm ID required by `CMSG_AUTH_SESSION`. AzerothCore constructs those fields in the [realm-list response](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/apps/authserver/Server/AuthSession.cpp#L739-L836).

## World authentication and framing

The world socket is a separate TCP connection. The server first sends an unencrypted 40-byte `SMSG_AUTH_CHALLENGE` body: `u32 1`, a 4-byte server seed, and 32 unused random bytes ([server construction](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Server/WorldSocket.cpp#L223-L231)).

`CMSG_AUTH_SESSION` is also unencrypted. Its body is:

```text
u32 build = 12340
u32 login_server_id = 0
cstring normalized_account
u32 login_server_type = 0
u32 client_seed                  # fresh random value
u32 region_id = 0
u32 battlegroup_id = 0
u32 realm_id                     # from realm list
u64 dos_response = 0
u8  digest[20]
u8  addon_info[]
```

The digest is `SHA1(account || LE32(0) || LE32(client_seed) || LE32(server_seed) || K)`. AzerothCore's [reader and verifier](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Server/WorldSocket.cpp#L530-L630), the independent [packet schema](https://github.com/gtker/wow_messages/blob/e1c9e15a8b94fce76cbd433cf49bd67c376a99d7/wow_world_messages/src/world/wrath/cmsg_auth_session.rs#L4-L37), and the independent [digest calculation](https://github.com/gtker/wow_srp/blob/25ffab6433e1ee5eee629200cf42b592c1f36121/src/vanilla_header/internal.rs#L6-L22) agree.

Do not omit `addon_info` entirely: AzerothCore intentionally treats an empty remainder as malformed. The minimum controlled payload is four zero bytes, declaring uncompressed addon data size zero; `ReadAddonsInfo` then returns without decompression ([addon reader](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Server/WorldSession.cpp#L1220-L1229)).

After `CMSG_AUTH_SESSION`, initialize two stateful header ciphers from `K`:

- client-to-server: `RC4-drop1024(HMAC-SHA1(client-direction constant, K))`;
- server-to-client: `RC4-drop1024(HMAC-SHA1(server-direction constant, K))`.

AzerothCore's exact constants and drop are in [`AuthCrypt`](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/common/Cryptography/Authentication/AuthCrypt.cpp#L22-L48); the independent implementation records the [directional constants](https://github.com/gtker/wow_srp/blob/25ffab6433e1ee5eee629200cf42b592c1f36121/src/wrath_header/mod.rs#L92-L132) and [RC4-drop1024 construction](https://github.com/gtker/wow_srp/blob/25ffab6433e1ee5eee629200cf42b592c1f36121/src/wrath_header/inner_crypto/mod.rs#L20-L42).

Only headers are encrypted; payload bytes remain plaintext. Client-to-server headers are always six bytes: big-endian `u16 size` including the four-byte opcode, then little-endian `u32 opcode`. Server-to-client headers are four bytes normally, or five for large packets: a big-endian two- or three-byte size including the two-byte opcode, then little-endian `u16 opcode`. AzerothCore defines the [client header](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Server/WorldSocket.h#L57-L66), [server header](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Server/Protocol/ServerPktHeader.h#L25-L57), and [header-only encryption](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Server/WorldSocket.cpp#L165-L180).

The first encrypted server packet is `SMSG_AUTH_RESPONSE`; require `AUTH_OK`. The Reference Realm must set `Warden.Enabled = 0`, because Warden is enabled by default and would add a proprietary-client challenge outside this slice ([Warden default](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/apps/worldserver/worldserver.conf.dist#L1175-L1182)).

## Character selection and world entry

Send `CMSG_CHAR_ENUM` even when the Reference Realm provisioner already knows the character GUID. AzerothCore clears and repopulates its session-local `_legitCharacters` set only while handling enumeration ([enumeration handler](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Handlers/CharacterHandler.cpp#L228-L270)); `CMSG_PLAYER_LOGIN` is rejected unless its GUID is in that set ([login guard](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Handlers/CharacterHandler.cpp#L684-L707)).

Parse each full `SMSG_CHAR_ENUM` record so the next record begins at the correct offset. The slice consumes at least GUID, name, race/class/gender, level, map, area, position, and flags, while cosmetics, pet data, and 23 equipment entries may remain passive fields in the codec. Select the pre-provisioned character by configured name or assert that the single returned character matches expectations. Send its full `u64` GUID in `CMSG_PLAYER_LOGIN` ([client packet layout](https://github.com/gtker/wow_messages/blob/e1c9e15a8b94fce76cbd433cf49bd67c376a99d7/wow_world_messages/src/world/shared/cmsg_player_login_vanilla_tbc_wrath.rs#L5-L37)).

Login produces many initialization packets. The packet loop must be able to frame and skip unknown opcodes by declared length, rather than requiring a model for every game system. `SMSG_LOGIN_VERIFY_WORLD` is the hard world-entry boundary: its 20-byte body is `u32 map` followed by four `f32` values `x`, `y`, `z`, and orientation, matching AzerothCore's [login emission](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Handlers/CharacterHandler.cpp#L821-L830) and the independent [packet schema](https://github.com/gtker/wow_messages/blob/e1c9e15a8b94fce76cbd433cf49bd67c376a99d7/wow_world_messages/src/world/wrath/smsg_login_verify_world.rs#L7-L50).

For a normal, aura-free, non-flying test character, AzerothCore also sends `SMSG_MOVE_UNSET_CAN_FLY` after adding the player to the map. A well-behaved minimum client answers with `CMSG_MOVE_SET_CAN_FLY_ACK`, preserving the order counter and using current ground movement state. The server emission is in [`SetCanFly(false)`](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Entities/Unit/Unit.cpp#L16518-L16545); the independent schemas cover the [server request](https://github.com/gtker/wow_messages/blob/e1c9e15a8b94fce76cbd433cf49bd67c376a99d7/wow_world_messages/src/world/shared/smsg_move_unset_can_fly_tbc_wrath.rs#L5-L40) and [client acknowledgement](https://github.com/gtker/wow_messages/blob/e1c9e15a8b94fce76cbd433cf49bd67c376a99d7/wow_world_messages/src/world/wrath/cmsg_move_set_can_fly_ack.rs#L8-L55).

Answer every `SMSG_TIME_SYNC_REQ` with the same counter and the client's monotonic millisecond clock. AzerothCore uses it to translate later client movement timestamps and otherwise falls back to server time ([time-sync handler](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Handlers/MovementHandler.cpp#L932-L963)).

### Parse now, frame now, defer now

| Treatment | Packets/data |
| --- | --- |
| Parse and act | `SMSG_AUTH_RESPONSE`, `SMSG_CHAR_ENUM`, `SMSG_LOGIN_VERIFY_WORLD`, `SMSG_MOVE_UNSET_CAN_FLY`, `SMSG_TIME_SYNC_REQ`, and movement-related errors/disconnects. |
| Frame and skip | Account-data times, addon info, feature status, tutorial flags, bind point, spells/actions, reputations, achievements, MOTD, world states, social/guild data, and other initialization packets. |
| Defer behind a named research question | The self create/update inside `SMSG_UPDATE_OBJECT`, initial authoritative speeds/control fields, and correction-oriented update fields. They improve self-state fidelity but are not needed to identify the mover, render a placeholder at the login coordinates, or submit the first ground movement packets. |

For a short-lived slice, client `CMSG_PING`/server `SMSG_PONG` is robustness rather than an entry prerequisite. Add it before treating long-lived sessions or reconnect behavior as production-like.

## Minimal ground movement

Each of the three movement messages has the same body:

```text
PackedGuid active_mover
u32        flags
u16        flags2
u32        client_timestamp_ms
f32        x
f32        y
f32        z
f32        orientation
u32        fall_time
# optional transport, pitch, jump, and spline fields appear only when flags demand them
```

For the controlled ground path:

1. Use the selected character GUID, confirmed as the active mover during login.
2. Send `MSG_MOVE_START_FORWARD` with `flags = 0x00000001`, `flags2 = 0`, current position/orientation, synchronized client time, and `fall_time = 0`.
3. Locally advance through the Diagnostic World and send `MSG_MOVE_HEARTBEAT` with the same forward flag, increasing timestamps, and small plausible position deltas no faster than the character's run speed.
4. Send `MSG_MOVE_STOP` with no movement flags and the final position.

AzerothCore requires the packed GUID to equal the session mover, requires that mover to be in the world, parses and validates the movement state, relocates the server-side unit, and then broadcasts the packet to nearby players ([movement handler](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Handlers/MovementHandler.cpp#L363-L435)). The broadcast explicitly skips the sending player, so normal accepted movement has **no direct movement ACK or echo**. A silence-only assertion cannot distinguish acceptance from a discarded packet.

The Reference Realm should therefore pre-place the character at a stable, flat, non-transport coordinate and clear special movement auras/states. No terrain or collision data is delivered by this protocol path, and the Learning Client intentionally has no Blizzard map assets with which to reproduce client collision. The server accepts client-reported coordinates subject to validity and anti-cheat checks; the slice should use modest deterministic ground deltas and should not disable general movement checks pre-emptively.

### Observable proof of server-recognized movement

Use this black-box acceptance sequence:

1. Record map and position from `SMSG_LOGIN_VERIFY_WORLD`.
2. Send the start/heartbeat/stop sequence to a meaningfully different valid ground position.
3. Let the server complete a saving logout or disconnect, and wait until the character is offline.
4. Reauthenticate, enumerate, and log in again.
5. Require the next `SMSG_LOGIN_VERIFY_WORLD` to report the same map and the moved position within a small tolerance.

This works because server-side relocation updates the player's current position, a saving logout calls `SaveToDB` ([logout save](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Server/WorldSession.cpp#L785-L798)), and the character update persists map and coordinates ([position persistence](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Entities/Player/Player.cpp#L15236-L15261)). A live integration test may additionally query the character row after logout for faster diagnostics, but reconnect plus `SMSG_LOGIN_VERIFY_WORLD` is the protocol-visible acceptance contract.

## Reference Realm contract

The local realm bootstrap must establish these invariants:

- auth TCP endpoint published to the host, default `3724`;
- world address in `realmlist` resolvable/reachable from the host, default world port `8085`;
- realm build `12340` and a known realm ID;
- `StrictVersionCheck = 0` while still requiring build 12340;
- `Warden.Enabled = 0`;
- one normalized test account with no TOTP and no IP/country lock conflict;
- one named character owned by that account, at a stable flat coordinate, with no transport, vehicle, flight, root, hover, water-walk, feather-fall, or similar special state;
- no login queue for the acceptance fixture;
- credentials supplied outside logs and committed files; the session key remains memory-only and is never logged.

The protocol path needs no Blizzard data files. The Diagnostic World can render project-owned geometry, show the numeric map/position/orientation, and move a project-owned placeholder. AzerothCore database content remains server-side.

## Compatibility warning for a possible Rust core

`wow_messages`/`wow_srp` are useful evidence and possible dependencies, but they are not accepted wholesale by this trace. At the pinned revision, `wow_world_messages` models Wrath `MovementInfo.fall_time` as `f32` and serializes jump cosine before sine ([generated movement layout](https://github.com/gtker/wow_messages/blob/e1c9e15a8b94fce76cbd433cf49bd67c376a99d7/wow_world_messages/src/world/wrath/movement_info.rs#L8-L49), [serialization](https://github.com/gtker/wow_messages/blob/e1c9e15a8b94fce76cbd433cf49bd67c376a99d7/wow_world_messages/src/world/wrath/movement_info.rs#L116-L130)). Current AzerothCore expects a `u32 fallTime` and reads jump `zspeed, sinAngle, cosAngle, xyspeed` ([server movement reader](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/game/Server/WorldSession.cpp#L1067-L1101)).

The minimum ground packet remains byte-compatible because zero has identical four-byte representation and none of the optional jump fields are emitted. Any later fall/jump/transport support needs an AzerothCore-specific adapter or an upstream fix, backed by wire-level regression tests. This discrepancy must be included in the engine/core dependency decision rather than discovered during gameplay work.

## Consequences for later decisions

- The engine-independent core needs separate login and world codecs plus explicit session states; one generic socket abstraction is not the protocol model.
- The world receive loop must preserve cipher stream position and be able to skip unknown framed packets safely.
- Crypto, headers, packet layouts, and state transitions can be tested deterministically without an engine.
- `SMSG_LOGIN_VERIFY_WORLD`, not full world data, is sufficient to place the first placeholder.
- Time sync and the no-flight control acknowledgement belong in the minimum well-behaved session, not a later multiplayer feature.
- Movement authority/correction design must acknowledge that ordinary movement has no sender ACK; reconnect persistence is the initial proof oracle.
- The minimum authoritative self-update/speed boundary remains a focused research item before the architecture and movement contracts are finalized.
