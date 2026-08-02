# Deterministic replication test harness

Ticket: [16 – Design the deterministic replication test harness](../issues/16-design-deterministic-replication-test-harness.md)

## Decision

The replication harness is a layered, test-only fixture vocabulary, not a
Realm emulator and not a new workspace crate. It reuses the existing
`IncrementalWorldServerDecoder`, `RetainedScriptTransport`/scripted-transport
pattern, and `FixedClock` pattern at their current crate-local test seams.

```text
synthetic plaintext record builders
  -> encrypted World-frame script and arbitrary poll chunks
  -> retained World worker + FakeClock + semantic ClientEvent FIFO
  -> pure RemoteAvatarPresentation / headless Bevy schedule
  -> observer-only assertion collector
```

No layer opens Docker, a TCP listener, a Bevy window/GPU renderer, a real
clock, the filesystem for credentials, or a real Pair Profile. It simulates
only what one observer receives after entry: a bounded encrypted sequence of
server frames. It does not simulate a Realm, authentication, movement physics,
server persistence, visibility, peer crash/reconnect, or general multiplayer.

## Fixture provenance and construction

Fixtures are checked-in source code built from a small semantic trace of
`Create`, `Movement`, `OutOfRange`, `Destroy`, safe ignored records, or one
intentionally malformed record. They contain only test GUIDs, map `0`, finite
synthetic coordinates, fixed timestamps, and a documented test-only 40-byte
World cipher key. They must not contain captured traffic, Realm dumps,
passwords, live session keys, account IDs, Character names, assets, or opaque
values-mask/display data.

`client_protocol` test helpers construct complete relevant update bodies and
encrypt headers sequentially with that fixed key. They use the production
header direction/layout but remain independent synthetic plaintext and are not
exported from `client_protocol`. The `client_session` test module owns its own
small, independent synthetic server-header encoder over the public
`HeaderCipher`/`HeaderDirection` API; it is deliberately not imported from a
dependency's `#[cfg(test)]` module. The two encoders are checked against the
same declarative `FrameScript` vectors, while protocol unit tests remain the
authority for byte/framing behavior. This keeps all helpers crate-local and
still gives the retained-loop matrix genuine encrypted chunk input.

`FrameScript` is an ordered `{ opcode, plaintext_payload }` list.
`EncryptedWorldScript` encrypts headers without resetting cipher state.
`PollScript` cuts that one stream into explicit retained-read steps:

```text
Pending | Bytes(Vec<u8>) | Eof | ReadError(ErrorKind)
```

A `Bytes` step may be one byte, a header plus partial payload, one frame, or
multiple coalesced frames. The scripted transport returns one step per poll;
every fragmentation and timing assertion is therefore reproducible without
sleeping.

## Clock and scenario driver

The monotonic `FakeClock` advances only through named scenario actions; its
`sleep` records requests and never blocks. A scenario has a fixed maximum
poll/action count and fails if it does not consume its script, so `Pending`
cannot spin forever. It records only virtual `Duration` values.

`ObserverScenario` owns exactly one retained observer session. Its only
role-labelled input is a synthetic peer GUID; it does not instantiate a mover
client. Its driver has `advance_to`, `poll_once`, `drain_observer_events`, and
`snapshot` actions. Immediately after each successful `poll_once`, before any
later clock advance or drain, the driver stamps every newly accepted Remote
Avatar event as `{ event, received_at: FakeClock.now() }`. The timestamp is a
test-only observation wrapper, never a public event field or sidecar value.

The pure timeline oracle uses Ticket 09's scoped limits: first observed pose
at or before `331 ms`, terminal stop at or before `508 ms`, clean removal at
or before `19,760 ms`, and terminal same-map delta at or below `0.25 m`.
Exact boundaries pass; one millisecond later fails. The oracle compares the
captured `received_at` values, never the later drain time. This verifies later
proof policy arithmetic/order, not network performance or live latency.

## Test placement and seams

No production type becomes public solely for testing. The harness stays split
by production ownership:

| Location | Responsibility |
| --- | --- |
| `client_protocol` unit tests beside `world.rs`/`world_entry.rs` | synthetic bodies, sequential encryption, fragmentation/coalescing, decoding, structural errors |
| `client_session` runtime/boundary tests | independent synthetic header encoder, scripted retained transport, fake clock, lifecycle/FIFO/snapshot, faults, retry/backpressure |
| `client_bevy` unit tests and headless schedule test | private ingress, fence, marker state, smoothing/snap/heading, redaction; no window/capture |
| `client_session` test-only observer helper | terminal lifecycle/pose assertions from observer remote events and snapshot source sequence |

Protocol/session helpers remain `#[cfg(test)]` and crate-private. Bevy tests
consume only the public semantic session vocabulary of a real bridge. This
preserves the one-way dependency graph and prevents test helpers becoming
runtime transport or ECS APIs.

## Required deterministic matrix

### Framing and decoder

1. Deliver create, unknown complete frame, movement, and destroy at every
   header/payload boundary and then as one coalesced read. Semantic output must
   match and the next encrypted frame must decode, proving ignored-record
   cipher alignment.
2. Cover uncompressed and bounded compressed containers; fully consume valid
   self/NPC/values/near/other-GUID records before a valid peer record.
3. Malformed header, frame/buffer limit, incomplete EOF, inflate failure,
   invalid update structure, and trailing bytes fail the World session. No
   later-frame/cipher-recovery assertion is allowed.
4. A complete structurally consumable non-finite or unsupported movement
   becomes the specified per-GUID unusable record, not a framing success.

### Session and observer truth

1. `Create → Update → Destroy` and `Create → Update → OutOfRange` emit ordered
   consecutive Remote Avatar events; snapshots point to exact source sequence
   and removal leaves no snapshot.
2. A second eligible GUID, unmatched removal, stale movement, and ignored
   object leave selected observer truth unchanged. Duplicate create and every
   accepted-GUID unusable category yield a redacted `Faulted` outcome.
3. Pending, one-byte, header-partial-payload, coalesced, EOF, and read-error
   polls exercise the retained loop without wall-clock sleeps. Post-ready Time
   Sync still receives its normal reply.
4. Fill the 64-slot semantic FIFO after remote create, then overflow with an
   update. The session fails, clears remote state, publishes its invalidation
   fence, and never reports a successfully published dropped update.
5. Retry starts a new transport, cipher, fake-session state, accepted-GUID
   slot, snapshot, and observer collector; first-attempt pose/event data is
   rejected by the second.

### Presentation without rendering

1. A headless schedule receives more than eight remote transitions in one
   ingress drain. Projection sees every event; the UI tail alone keeps eight.
2. Create/hydration place rendered exactly at observed. Same-map delta below
   `1.628 m` follows the documented virtual-delta blend without overshoot;
   exactly `1.628 m` snaps; heading takes the normalized short arc.
3. Removed, fault, map-context unavailable, saturated fence, and normal
   failed/offline state remove marker/poses. Old events cannot resurrect it;
   newer create and matching update map recovery can.
4. Local presentation and all controlled-character pose truths remain
   bit-for-bit unchanged under every remote sequence.

### Observer-only evidence

The test-only observer collector accepts success only when its observer session
has ordered `Created`, terminal `Updated`, and matching `Removed`, along with
their poll-time `received_at` wrappers, terminal source sequence, and
Realm-observed pose. It calculates the `0.25 m` comparison only from that
observer event pose and scripted target.

It rejects mover-local, rendered, Submitted, predicted, database, absent
removal, different-GUID, stale-source, map-mismatch, or fault inputs. It is a
unit semantic collector, not a sidecar writer and not a substitute for Ticket
11's hash-bound bundle or Ticket 17's live proof.

## Explicit non-claims

- Passing does not prove live Realm peer replication, a live render, or timing
  outside the measured Fixture Pair scope.
- It is not a general decoder, multiplayer simulator, benchmark, live
  acceptance substitute, Windows acceptance, or credential fixture route.
- Docker reset/provisioning, orchestration, capture, sidecar validation, and
  manual two-window acceptance remain later tickets.
