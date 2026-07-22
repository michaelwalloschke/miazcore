# Discover the Reference Realm through Authenticated Login

Type: implementation
Status: claimed
Blocked by: [Render the Offline Diagnostic World from the Production Scaffold](12-render-offline-diagnostic-world.md)

## Parent

[Define the implementation slices and scope gates](09-define-implementation-slices.md), capability slice 2.

## What to build

Deliver the first real production network path as a headless integration: authenticate the configured fixture account with the local login server, discover the expected Reference Realm, verify its identity and endpoint, then disconnect cleanly. The visible Bevy application must remain offline so an incomplete `StartEntry` path cannot become user-facing.

Implement the fixture-manifest format and synthetic SRP/header-crypto vectors, login framing, challenge/proof exchange, authentication rejection mapping, and realm-list decoding. Introduce the real session worker behind private transport, monotonic-clock, and entropy ports; it must publish the accepted redacted semantic stages through the final application boundary.

## Entry gate

- [x] Ticket 12 passes its full production scaffold and routine verification gate on the current branch.
- [x] Reference Realm `health` is green.
- [x] The entry evidence and exact predecessor commit are recorded.

## Acceptance criteria

- [ ] Independent synthetic vectors and golden transcripts prove the SRP6 and login-frame byte boundaries.
- [ ] The login state machine handles challenge, proof, rejection, and realm-list decoding through production protocol and session code.
- [ ] The dedicated session worker owns blocking transport and ordered protocol state while tests substitute private transport, clock, and entropy ports.
- [ ] Accepted semantic stages reach the final command/event/snapshot boundary without exposing protocol-library types.
- [ ] Deterministic scenarios cover success, authentication rejection, timeout, fragmented reads, malformed frames, orderly shutdown, and secrecy.
- [ ] A headless live integration authenticates the fixture account, selects realm ID `1` named `Miazcore Reference Realm`, verifies build `12340` and its advertised world endpoint, and disconnects.
- [ ] Independent golden tests and the real login/realm path pass through the production codecs and runtime.
- [ ] Formatting every emitted command, event, snapshot, failure, and diagnostic proves credentials and session material cannot leak.
- [ ] Worker shutdown is reliable after success and every covered failure.
- [ ] The offline production application remains green and exposes no partially implemented connection action.
- [ ] Formatting, locked native workspace/all-target checks, Clippy with warnings denied, native tests, dependency-boundary checks, redaction tests, scripted Metal smoke, and the Windows compile tripwire pass.
- [ ] The exit evidence, remaining deferrals, and exact passing commit are recorded.

## Explicit deferrals

- World authentication and live encrypted world headers.
- Character enumeration or selection, player login, Bevy-driven connection, and all movement behavior.
- Making `StartEntry` partially user-facing; the intermediate capability remains headless and engine-independent.

## Shared scope and evidence rules

- Preparation may happen in parallel, but this ticket cannot integrate until every declared blocker has passed its exit gate on the current branch.
- Ship tests and verification tooling with the behavior they prove.
- A newly observed login opcode or behavior joins this ticket only when the live realm proves it blocks realm discovery; otherwise skip it safely where allowed and defer it.
- Fix a failing gate here. Do not waive, mute, retry away, or postpone it to Acceptance hardening.
- Keep the Windows compile tripwire green without claiming Windows runtime acceptance.
- Keep upstream incompatibilities inside `client_protocol`; do not expose generated types or fork dependencies.
- Work and verify this ticket on one candidate, then run `/code-review` and commit before advancing the frontier.
