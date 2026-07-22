# Render the Offline Diagnostic World from the Production Scaffold

Type: implementation
Status: ready-for-agent
Blocked by: None — can start immediately

## Parent

[Define the implementation slices and scope gates](09-define-implementation-slices.md), capability slice 1.

## What to build

Deliver the first production Learning Client executable as an offline, Metal-rendered Diagnostic World. A developer must be able to launch the real application, inspect the accepted viewport-first cockpit, move the placeholder with the accepted controls, and observe safe diagnostics without any network path being present.

Build this as the production Cargo workspace containing `client_protocol`, `client_session`, `client_bevy`, and the `learning_client` composition binary. Pin Rust 1.97.1, Bevy 0.19.0, all other dependencies, and the lockfile. Enforce the one-way dependency direction from composition and Bevy through session to protocol.

The end-to-end offline path includes immutable configuration, credential-file validation, redacted and zeroizing secret ownership, sanitized identity, the final semantic command/event/snapshot boundary with its bounded queues, and a fake offline session source. Reimplement the accepted primitive world, ordered plugin composition, diagnostics, chase/orbit camera, and input mapping in production code; do not copy either prototype into the production workspace.

## Entry gate

- [ ] Reference Realm `health` and `smoke` pass on the current branch.
- [ ] The disposable Bevy proof still passes its native tests, Metal render proof, and Windows MSVC all-target compile check.
- [ ] The entry evidence is recorded before implementation begins.

## Acceptance criteria

- [ ] The production workspace contains the four accepted crates and an exact dependency lock.
- [ ] Only `client_bevy` and the composition binary depend on Bevy; dependencies never point back toward the engine.
- [ ] Configuration is immutable after startup, validates credential files without disclosing their contents, and rejects invalid or unsafe identity/configuration values visibly.
- [ ] Secret-bearing values are zeroized where ownership ends, and formatting every public command, event, snapshot, error, and diagnostic proves that credentials and session material cannot appear.
- [ ] The final control FIFO, latest-value movement-intent mailbox, event FIFO, and latest snapshot projection exist behind an engine-independent boundary; the offline source exercises that same boundary.
- [ ] The production app renders the accepted `Offline` Diagnostic World through Metal using project-owned primitives and UI.
- [ ] Input, viewport focus, chase/orbit camera, zoom, and the Rendered/Submitted/Realm-observed diagnostic presentation match the accepted experience while remaining offline.
- [ ] Bevy systems execute in the accepted `Ingress -> Input -> Presentation -> Camera -> Diagnostics` order.
- [ ] Formatting, locked native workspace and all-target checks, Clippy with warnings denied, and the full native test suite pass.
- [ ] `MinimalPlugins` adapter tests, dependency-boundary assertions, configuration/credential/redaction tests, scripted Metal smoke, and the Windows compile tripwire pass.
- [ ] The exit evidence, remaining deferrals, and exact passing commit are recorded.

## Explicit deferrals

- Every login or world codec, socket, real session, live realm entry, prediction step, movement frame, and Movement Proof.
- Any claim that the production application has contacted AzerothCore.
- Deletion of the disposable Bevy prototype until equivalent production evidence exists. Retain its research record and referenced evidence even after its code is removed.

## Shared scope and evidence rules

- Preparation may happen in parallel, but this ticket cannot integrate until every declared blocker has passed its exit gate on the current branch.
- Ship tests and verification tooling with the behavior they prove.
- A newly discovered requirement enters this ticket only if this ticket's exit capability cannot work correctly without it; record useful non-blockers as explicit deferrals.
- Fix a failing gate here. Do not waive, mute, retry away, or postpone it to Acceptance hardening.
- Keep the Windows compile tripwire green without claiming Windows runtime acceptance.
- Do not widen gameplay, content, protocol, multiplayer, or platform scope.
- Work and verify this ticket on one candidate, then run `/code-review` and commit before advancing the frontier.
