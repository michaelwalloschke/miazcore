# Slice 15 exit-gate evidence

Recorded: `2026-07-22`

Exact passing candidate: `e73bc03ee26024689829f816a1350710c9d12eb7`

Exact predecessor: `00d313ffdde6fa56da56d2c7f932d9660e5b013b`

The candidate was committed before the final routine, rendering, review, and
live gates ran. The later evidence-only commit changes only this record and the
local ticket resolution.

## Delivered boundary

The engine-independent production session now continues from exact `Miaztest`
selection through `CMSG_PLAYER_LOGIN`, authoritative entry corroboration, and
the required control exchanges to the public `MovementReady` invariant. The
session exposes only sanitized semantic state: selected identity, Entry Anchor,
realm-observed pose, current run speed, phase, counters, and bounded diagnostics.

The project-owned protocol boundary selectively implements:

- `SMSG_LOGIN_VERIFY_WORLD` and bounded compressed/uncompressed update
  containers;
- exactly one matching player `CreateObject2` with `SELF | LIVING`, while
  structurally consuming movement blocks and skipping opaque update values;
- the AzerothCore movement layout used by this capability, including integer
  fall time, jump-data ordering, and all nine bootstrap speeds;
- later forced run-speed change/acknowledgement, initial time synchronization,
  and the no-flight acknowledgement.

It does not construct a general object/update-field model and does not consume
or send movement intent. The implementation was independently derived against
the pinned AzerothCore source at
`a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f` and the pinned `wow_messages`
no-flight acknowledgement at
`e1c9e15a8b94fce76cbd433cf49bd67c376a99d7`; no AzerothCore or client code was
copied into the repository.

## Deterministic and platform gates

`./scripts/check.sh` passed on the exact candidate. It covered Rustfmt, locked
native workspace/all-target compilation, Clippy with warnings denied, all 61
reported native unit/integration tests, the exact four-package one-way
dependency graph and pinned dependencies, redaction tests, and the
`x86_64-pc-windows-msvc` all-target compile tripwire.

The protocol suite passed 15 tests, including four focused world-entry golden
tests. Eleven new world-entry fixture manifests cover the player-login body,
login verification, uncompressed and compressed self projection, later run
speed and acknowledgement, time request/response, no-flight request/ack, and a
sanitized live self projection. All 24 versioned protocol manifests passed
independent payload and SHA-256 validation.

The 35 `client_session` unit tests cover the complete ordered path and the new
fail-closed matrix: malformed/truncated/mismatched/absent self state, compressed
stream bounds, unsupported self control, EOF, bootstrap and synchronization
timeouts, player-login and every required control write fault, shutdown during
bootstrap/synchronization, clean socket closure, gated movement, and explicit
retry from fresh sockets, ciphers, time, and entropy. Assertions pin category,
stage, sanitized context, recovery action, one attempt, final disconnect, and
the absence of movement-ready or movement-submitted evidence on every failure.
Internal malformed-world diagnostics retain opcode and byte offset without
exposing either detail through the public failure surface.

`./scripts/render-smoke.sh` passed using Metal at `2560x1440`, preserving the
accepted explicitly offline Diagnostic World while the live Bevy bridge remains
deferred:

- screenshot SHA-256:
  `27c8705ad4ef1ceedd64d472caff30b0f35d7534569be7d3d1998bc9e20a9f6d`;
- semantic sidecar SHA-256:
  `63f460f5314c1392876aa4346b56b0da172f1ec702a5cb3a508b5aa60446ddf4`.

The Windows result remains a compile tripwire only; native Windows runtime and
rendering acceptance are not claimed.

## Sanitized live fixture

The semantic-only capture procedure and provenance are recorded in
[Slice 15 sanitized live self projection](slice-15-live-self-projection.md).
The observed ordinary-ground pose and all nine speeds were rebuilt into an
independent fixture with a synthetic GUID/timestamp and zero opaque values. No
authenticated packet body, update-field value, credential, key, or real GUID
was retained.

## Live Reference Realm gate

`./scripts/live-movement-ready.sh` passed on the exact candidate while holding
the repository's exclusive live-gate lock. Its label-checked reset removed and
recreated only the managed Reference Realm state, reprovisioned `Miaztest`, and
passed layered container, process, host-socket, and semantic fixture health
before the client attempt.

The headless production path then performed a fresh login, realm discovery,
world authentication, exact character selection, player login, authoritative
self discovery, entry-pose corroboration, and required control synchronization:

```text
world entry: PASS name=Miaztest guid=0x1 map=0 anchor=(-8949.950,-132.493,83.531,0.000) run_speed=7.000 ready_events=1 movement_events=0 disconnected=true
```

The invariant therefore had one `MovementReady` event, the realm-provided run
speed, and an authoritative Entry Anchor; it emitted zero movement events and
disconnected cleanly. The script's final layered Reference Realm health check
also passed after the session.

## Review gate

The required two-axis review compared the implementation with the exact
predecessor. Initial standards findings about capability-specific naming,
target-policy switches, and protocol-error ownership were fixed by centralizing
target policy, using neutral Entry terminology, and moving the shared error to
its own module. Local decoder cursors remain intentionally separate because
login, world, and world-entry framing require different validation and error
context.

Initial specification findings about captured-fixture provenance, exact
malformed diagnostics, and incomplete external-wait fault coverage were fixed
by the sanitized live projection, opcode/byte-offset internal diagnostics, and
the full bootstrap/control fault matrix. Both independent final re-reviews of
`e73bc03ee26024689829f816a1350710c9d12eb7` returned `PASS` with no hard
standards or specification gaps.

## Remaining deferrals

- Prediction and every movement start/heartbeat/stop packet.
- Movement Proof and persisted/corrected realm movement.
- Live Bevy connection, authoritative placeholder placement, rendering
  interpolation, and correction presentation.
- A general game-object model, full update-field model, or broader opcode
  coverage.
- Native Windows build, test, runtime, and rendering acceptance.

The production Bevy application still uses `OfflineSession`; the complete
network entry path is intentionally available only through the engine-free
session boundary and headless live harness until ticket 16.
