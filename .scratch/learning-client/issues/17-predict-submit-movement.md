# Predict and Submit Bounded Ground Movement

Type: implementation
Status: claimed
Blocked by: [Enter the Live Diagnostic World through Bevy](16-enter-live-diagnostic-world.md)

## Parent

[Define the implementation slices and scope gates](09-define-implementation-slices.md), capability slice 6.

## What to build

Let the player move and turn the live placeholder smoothly inside the Reference Movement Envelope while the production session submits valid ground movement to the realm. The UI must show Rendered and Submitted motion separately from unchanged Realm-observed state and remain explicit that persistence has not yet been proven.

Publish camera-relative intent through the latest-value mailbox. Implement deterministic 60 Hz Heading-aligned Movement using the current realm-provided run speed, preserve Entry Anchor height, enforce the five-metre envelope, write immediate ordered start/stop frames and 10 Hz coalescible heartbeats through `AcoreMovementInfo`, and advance Submitted Pose only after complete writes.

## Entry gate

- [ ] Ticket 16 passes while proving that it emits no movement.
- [ ] The entry evidence and exact predecessor commit are recorded.

## Acceptance criteria

- [ ] Camera-relative normalized input becomes heading-aligned planar movement, and the character turns to its world heading.
- [ ] Prediction advances on a deterministic 60 Hz clock using the current positive realm-provided run speed.
- [ ] Predicted displacement is bounded by elapsed time, retains Entry Anchor height, and never leaves the five-metre Reference Movement Envelope.
- [ ] The latest-value mailbox may replace steady intent without dropping any lossless start or stop transition.
- [ ] Start and stop frames write immediately and in order; moving state emits coalescible heartbeats at 10 Hz through project-owned `AcoreMovementInfo`.
- [ ] Submitted Pose advances only after the corresponding frame has been written completely.
- [ ] Realm-observed Pose remains unchanged until a later authoritative observation; it is never relabelled as accepted or acknowledged.
- [ ] Rendered Pose interpolates smoothly from Predicted Pose, and diagnostics clearly expose divergence among Rendered, Submitted, and Realm-observed state.
- [ ] Generic scripted corrections use the accepted smooth treatment below five metres and snap treatment at or above five metres or on map change.
- [ ] Queue overflow, socket failure, partial/failed write, focus loss, and unsupported movement/control state stop prediction, gate input, retain evidence, and require explicit recovery.
- [ ] Fake-clock tests cover prediction and heartbeat rates, elapsed-time bounds, transition ordering, heartbeat coalescing, envelope edges, focus loss, complete-write semantics, failures, and correction thresholds.
- [ ] A reset-scoped live smoke shows smooth movement/turning, valid start/heartbeat/stop writes, visible pose separation, envelope enforcement, and a connection that remains healthy after stop.
- [ ] The application and evidence make no claim of realm acceptance or persistence.
- [ ] Formatting, locked native workspace/all-target checks, Clippy with warnings denied, native tests, dependency-boundary checks, redaction tests, scripted Metal smoke, and the Windows compile tripwire pass.
- [ ] The exit evidence, remaining deferrals, scoped reset evidence, and exact passing commit are recorded.

## Explicit deferrals

- Saving logout and reconnect comparison.
- Database-based success conclusions and every claim of accepted, acknowledged, authoritative, or persisted movement.
- Terrain/collision, broader locomotion, special movement/control states, and gameplay.

## Shared scope and evidence rules

- Preparation may happen in parallel, but this ticket cannot integrate until every declared blocker has passed its exit gate on the current branch.
- Ship tests and verification tooling with the behavior they prove.
- Add an unmodeled movement behavior only when the live realm proves it blocks this exact exit capability; otherwise fail explicitly and defer it.
- Fix a failing gate here. Do not waive, mute, retry away, or postpone it to Acceptance hardening.
- Keep the Windows compile tripwire green without claiming Windows runtime acceptance.
- Keep protocol incompatibilities contained inside `client_protocol`; do not widen generated types or fork dependencies.
- Work and verify this ticket on one candidate, then run `/code-review` and commit before advancing the frontier.

## Comments

- 2026-07-23: Implementation is in progress across `ebc9bba757a19170cbb85ceb28b4ae97c45ffb54` through `1aceab4`. The retained receive/correction subslice and both compositor proofs pass. Deterministic retained-loop evidence now covers a 900-ms heartbeat catch-up coalesced to one latest frame, heading replacement, focus-loss stop intent, EOF-to-recoverable failure, and partial-write transport errors. Ticket exit evidence is still being consolidated, so this ticket is deliberately not resolved.
