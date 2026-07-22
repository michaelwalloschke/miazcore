# Reach MovementReady with Authoritative Self State

Type: implementation
Status: claimed
Blocked by: [Select Miaztest through an Authenticated World Session](14-select-fixture-character.md)

## Parent

[Define the implementation slices and scope gates](09-define-implementation-slices.md), capability slice 4.

## What to build

Complete the headless public `StartEntry` path from the selected character through player login and synchronization to a fail-closed `MovementReady` state. The production session must identify the selected player in authoritative world state, corroborate the entry pose, obtain the realm's run speed, complete required acknowledgements, and disconnect cleanly without sending movement.

Implement bounded zlib handling, compressed and uncompressed update containers, targeted authoritative self decoding, the project-owned AzerothCore movement codec, all nine bootstrap speeds, later run-speed changes and acknowledgements, initial time synchronization, and the no-flight acknowledgement. Consume or safely skip only the initialization traffic required to reach the accepted invariant; do not construct a general game-object or update-field model.

## Entry gate

- [x] Ticket 14 passes deterministically and against the live Reference Realm on the current branch.
- [x] The entry evidence and exact predecessor commit are recorded.

## Acceptance criteria

- [ ] The session sends `CMSG_PLAYER_LOGIN` for the exactly selected character and safely consumes the required initialization sequence.
- [ ] `SMSG_LOGIN_VERIFY_WORLD` establishes the candidate entry map and pose.
- [ ] Bounded decompression rejects malformed, oversized, or incomplete compressed updates without unsafe allocation or stream drift.
- [ ] Update-container decoding locates exactly one matching player `CreateObject2` carrying `SELF | LIVING`, structurally consumes required blocks, and skips opaque update values without building a broad update-field model.
- [ ] The selected and self GUIDs match, and entry/self map and pose data agree under the accepted validation rules.
- [ ] Project-owned `AcoreMovementInfo` uses AzerothCore's field widths and jump-data ordering for self updates and required acknowledgements.
- [ ] All nine bootstrap speeds are consumed, a positive realm-provided run speed becomes current, and later run-speed changes receive the required acknowledgement.
- [ ] Initial time synchronization and the no-flight acknowledgement complete before input or movement can become enabled.
- [ ] `StartEntry` reaches the public `MovementReady` invariant with sanitized semantic stages, input gating, clean failure/shutdown, and explicit retry from fresh sockets, ciphers, time, and entropy.
- [ ] Sanitized authoritative self-update fixtures are captured and independently validated.
- [ ] Deterministic scenarios cover every malformed self-state and synchronization boundary, timeout, EOF, rejection, unsupported movement/control state, shutdown, and fresh retry.
- [ ] A headless live production session reaches `MovementReady`, proves the invariant, emits no premature movement, and disconnects cleanly.
- [ ] Formatting, locked native workspace/all-target checks, Clippy with warnings denied, native tests, dependency-boundary checks, redaction tests, scripted Metal smoke, and the Windows compile tripwire pass.
- [ ] The exit evidence, remaining deferrals, and exact passing commit are recorded.

## Explicit deferrals

- Prediction and all movement packets.
- Movement Proof, live Bevy connection, rendering interpolation, and live correction presentation.
- A general game-object model, full update-field model, or broader opcode coverage.

## Shared scope and evidence rules

- Preparation may happen in parallel, but this ticket cannot integrate until every declared blocker has passed its exit gate on the current branch.
- Ship tests and verification tooling with the behavior they prove.
- Add an unmodeled opcode only when the live realm proves it blocks `MovementReady`; otherwise skip it safely where possible and defer it.
- Unsupported movement or control state fails this ticket explicitly rather than widening scope silently.
- Fix a failing gate here. Do not waive, mute, retry away, or postpone it to Acceptance hardening.
- Keep the Windows compile tripwire green without claiming Windows runtime acceptance.
- Keep upstream incompatibilities inside `client_protocol`; do not expose generated types or fork dependencies.
- Work and verify this ticket on one candidate, then run `/code-review` and commit before advancing the frontier.
