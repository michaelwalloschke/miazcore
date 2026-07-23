# Enter the Live Diagnostic World through Bevy

Type: implementation
Status: resolved
Blocked by: [Reach MovementReady with Authoritative Self State](15-reach-movement-ready.md)

## Parent

[Define the implementation slices and scope gates](09-define-implementation-slices.md), capability slice 5.

## What to build

Make **Connect & Enter Reference Realm** work through the complete production `StartEntry` path. The real Bevy application must reach `MovementReady`, render the controlled placeholder and diagnostic markers at the actual Entry Anchor, expose sanitized session truth, remain responsive, and shut its worker down cleanly while movement publication stays disabled.

Project the real connection stages, sanitized identity, run speed, Entry Anchor, Realm-observed Pose, queue counters, and semantic event history into the accepted Diagnostic World experience. Preserve the strict Bevy system order and the established focus, camera, controls, and fail-closed diagnostic behavior.

## Entry gate

- [x] Ticket 15 passes headlessly against the live Reference Realm on the current branch.
- [x] The offline production shell and its complete verification gate remain green.
- [x] The entry evidence and exact predecessor commit are recorded.

## Acceptance criteria

- [x] **Connect & Enter Reference Realm** invokes only the complete `StartEntry` operation rather than exposing individual protocol transitions.
- [x] Real semantic stages, sanitized identity, run speed, Entry Anchor, Realm-observed Pose, queue counters, and bounded semantic history are visible without secrets or raw packet dumps.
- [x] The controlled placeholder and diagnostic markers spawn from authoritative state at the actual Entry Anchor.
- [x] Rendered, Submitted, and Realm-observed poses remain distinct and accurately labelled even while equal at entry.
- [x] `Ingress -> Input -> Presentation -> Camera -> Diagnostics` ordering is preserved with the live bridge.
- [x] Camera, orbit, zoom, viewport focus, and general window interaction remain responsive during connection and after entry.
- [x] Every accepted failure is visible, redacted, fail-closed, gates input, and includes explicit recovery guidance.
- [x] Movement intent publication remains disabled and no movement packet is emitted.
- [x] `MinimalPlugins` projection tests prove event/snapshot projection, ordering, phase gating, queue counters, and clean worker shutdown.
- [x] A real Metal scenario connects to the Reference Realm, reaches `MovementReady`, renders the placeholder at the Entry Anchor, confirms matching diagnostics and no movement output, and exits with the worker joined.
- [x] Formatting, locked native workspace/all-target checks, Clippy with warnings denied, native tests, dependency-boundary checks, redaction tests, scripted window checks, Metal smoke, and the Windows compile tripwire pass.
- [x] The exit evidence, remaining deferrals, and exact passing commit are recorded.

## Explicit deferrals

- Movement intent, prediction, start/heartbeat/stop frames, and Submitted Pose advancement.
- Live pose divergence, correction behavior driven by realm observations, and Movement Proof.

## Shared scope and evidence rules

- Preparation may happen in parallel, but this ticket cannot integrate until every declared blocker has passed its exit gate on the current branch.
- Ship tests and verification tooling with the behavior they prove.
- A newly discovered UI or session requirement enters this ticket only when the live entry experience cannot meet its exit gate without it.
- Reopen the Bevy choice only for an already-accepted platform reliability or substantial authored-content/UI trigger.
- Fix a failing gate here. Do not waive, mute, retry away, or postpone it to Acceptance hardening.
- Keep the Windows compile tripwire green without claiming Windows runtime acceptance.
- Work and verify this ticket on one candidate, then run `/code-review` and commit before advancing the frontier.

Resolved by `7cb6e4589956f75efdac07026196e06ba43696c2`; see
[Slice 16 exit-gate evidence](../research/slice-16-exit-gate.md).
