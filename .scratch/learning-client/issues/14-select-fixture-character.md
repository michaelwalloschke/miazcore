# Select Miaztest through an Authenticated World Session

Type: implementation
Status: resolved
Blocked by: [Discover the Reference Realm through Authenticated Login](13-discover-reference-realm.md)

## Parent

[Define the implementation slices and scope gates](09-define-implementation-slices.md), capability slice 3.

## What to build

Extend the headless production path from realm discovery through a fresh authenticated world session and exact selection of the configured `Miaztest` character. Disconnect before player login so this ticket proves the encrypted session and character boundary without beginning world entry.

Implement the world challenge and session proof for build 12340, independent inbound and outbound encrypted header streams, fragmented and coalesced world framing, and safe skipping only after an unknown opcode's complete frame has been read. Decode complete character-enumeration records and extend deterministic session behavior through `Entering(CharacterSelection)`.

## Entry gate

- [x] Ticket 13 passes deterministically and against the live Reference Realm on the current branch.
- [x] The entry evidence and exact predecessor commit are recorded.

## Acceptance criteria

- [x] The production world session performs challenge and exact build-12340 authentication after fresh login/realm discovery.
- [x] Independent inbound and outbound header-cipher state remains aligned across multiple fragmented and coalesced packets.
- [x] Unknown opcodes are skipped only as safe, complete frames without desynchronizing the stream.
- [x] Character enumeration consumes complete records and selects exactly one configured character named `Miaztest`.
- [x] The semantic session state reaches `Entering(CharacterSelection)` without exposing generated protocol types.
- [x] Deterministic scenarios cover cipher drift, malformed headers, world-auth rejection, absent or duplicate configured character, timeout, EOF, cancellation, clean shutdown, and redacted diagnostics.
- [x] The live integration performs login, realm discovery, fresh world authentication, and exact character selection, then disconnects before sending `CMSG_PLAYER_LOGIN`.
- [x] Live nonexistent-account and absent-character probes produce the accepted stable failure categories without leaking secrets.
- [x] The offline application and all predecessor behavior remain green.
- [x] Formatting, locked native workspace/all-target checks, Clippy with warnings denied, native tests, dependency-boundary checks, redaction tests, scripted Metal smoke, and the Windows compile tripwire pass.
- [x] The exit evidence, remaining deferrals, and exact passing commit are recorded.

## Explicit deferrals

- `CMSG_PLAYER_LOGIN`, initialization/bootstrap consumption, and world verification.
- Authoritative self state, control/time synchronization, live Bevy entry, prediction, and movement.

## Shared scope and evidence rules

- Preparation may happen in parallel, but this ticket cannot integrate until every declared blocker has passed its exit gate on the current branch.
- Ship tests and verification tooling with the behavior they prove.
- Add an unmodeled world opcode only when the live realm proves it blocks exact character selection; otherwise safely skip and defer it.
- Fix a failing gate here. Do not waive, mute, retry away, or postpone it to Acceptance hardening.
- Keep the Windows compile tripwire green without claiming Windows runtime acceptance.
- Keep protocol incompatibilities and generated types contained inside `client_protocol`; do not fork dependencies.
- Work and verify this ticket on one candidate, then run `/code-review` and commit before advancing the frontier.

## Answer

Implemented authenticated build-12340 world-session negotiation, independent encrypted header streams, bounded complete-frame handling, full character enumeration, and exact configured-character selection in the engine-free production path. The worker now exposes sanitized `RealmDiscovered` and `CharacterSelected` semantics, then disconnects before player login; the visible Bevy application remains explicitly offline.

Independent fixtures, deterministic fault scenarios, fresh-state live success and negative probes, all routine native/Windows checks, and the Metal proof pass on exact candidate `08472c59de9f6a23bc8d9bf54b5f32877d8e3bad`. Full evidence and remaining deferrals are recorded in [Slice 14 exit-gate evidence](../research/slice-14-exit-gate.md).
