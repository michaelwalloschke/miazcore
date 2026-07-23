# Prove Persisted Movement and Recover Explicitly

Type: implementation
Status: resolved
Blocked by: [Predict and Submit Bounded Ground Movement](17-predict-submit-movement.md)

## Parent

[Define the implementation slices and scope gates](09-define-implementation-slices.md), capability slice 7.

## What to build

Turn submitted movement into the sole accepted Movement Proof: after a sufficiently large successfully stopped move, freeze the client, save and log out, establish completely fresh login and world sessions, and compare the reconnect pose with the expected Submitted Pose. The UI must show how the conclusion was reached and provide explicit, redacted recovery for every accepted failure.

## Entry gate

- [x] Ticket 17 passes deterministically and in its reset-scoped live smoke.
- [x] The entry evidence, scoped reset evidence, and exact predecessor commit are recorded.

## Acceptance criteria

- [x] Movement Proof remains unavailable until a successfully submitted stopped pose is at least two metres from the Entry Anchor.
- [x] **Verify persisted movement** invokes the semantic `BeginMovementProof` operation and immediately freezes input and prediction.
- [x] The session completes saving logout and the accepted offline wait before discarding the old transport, cipher, protocol time, and session state.
- [x] Proof always creates fresh authenticated login and world sessions; it never resumes or reuses the previous encrypted streams.
- [x] The expected oracle is the stopped Submitted Pose, and success requires the reconnect Realm-observed Pose to use the same map and be within `0.25 m`.
- [x] Success and failure show expected and observed map, both poses, delta, tolerance, and reconnect as the evidence source.
- [x] Database queries and realm logs may diagnose a failure but cannot create a successful Movement Proof.
- [x] Reconnect observations apply the accepted smooth or snap presentation treatment without changing the comparison oracle.
- [x] Every accepted timeout, rejection, EOF, logout/offline-wait failure, reconnect failure, comparison failure, queue failure, and shutdown path is redacted, fail-closed, and provides explicit recovery.
- [x] Deterministic tests cover proof eligibility, input freeze, saving logout, offline wait, state disposal, fresh reconnect, all comparison outcomes, timeouts, failures, shutdown, and explicit retry.
- [x] The isolated canonical live scenario moves between two and four metres, successfully submits stop, completes saving logout and fresh reconnect, and passes within tolerance.
- [x] Two isolated live negative probes prove the accepted failure paths without allowing a database-derived success.
- [x] Formatting, locked native workspace/all-target checks, Clippy with warnings denied, native tests, dependency-boundary checks, redaction tests, scripted Metal smoke, and the Windows compile tripwire pass.
- [x] The exit evidence, remaining deferrals, canonical and negative-probe evidence, and exact passing commit are recorded.

## Explicit deferrals

- Broader locomotion or control, terrain/collision, gameplay, and multi-client interaction.
- Windows runtime/render acceptance.
- Reconnect convenience or recovery beyond the explicitly accepted Movement Proof path.

## Shared scope and evidence rules

- Preparation may happen in parallel, but this ticket cannot integrate until every declared blocker has passed its exit gate on the current branch.
- Ship tests and verification tooling with the behavior they prove.
- A newly observed protocol behavior enters this ticket only if the canonical Movement Proof cannot work correctly without it; otherwise record it as a deferral.
- Unsupported movement or control state fails explicitly rather than silently widening scope.
- Fix a failing gate here. Do not waive, mute, retry away, or postpone it to Acceptance hardening.
- Keep the Windows compile tripwire green without claiming Windows runtime acceptance.
- Work and verify this ticket on one candidate, then run `/code-review` and commit before advancing the frontier.

## Answer

Resolved by implementation commit `0faaca8` (`Complete persisted movement proof lifecycle`).

- Entry predecessor: `3e13f6f` (`Record ticket 17 exit evidence`), with Ticket 17's deterministic and reset-scoped live proof already recorded.
- Canonical reset-scoped persistence proof: `scripts/persisted-movement-smoke.sh` passed. The sidecar reports `PersistedMovementCompared`, source `fresh-reconnect-login-verify-world`, `delta_metres: 0.0`, tolerance `0.25 m`, and a stopped submitted move of `2.44 m` from the entry anchor.
- Negative probes: `scripts/persisted-movement-negative-probes.sh` passed. The short-move sidecar reports `PersistedMovementRejected`; the isolated Worldserver outage reports `ReconnectUnavailableRejected`, restores the Worldserver, and finishes with `realm health` green. Neither probe derives success from the database.
- Final automated gates: `scripts/check.sh`, `scripts/render-smoke.sh`, `scripts/live-diagnostic-world.sh`, `scripts/live-movement-smoke.sh`, and `CC_x86_64_pc_windows_msvc=clang cargo check --locked --workspace --all-targets --target x86_64-pc-windows-msvc` passed. Render proofs used Metal and non-black exact-window captures.
- Review outcome: no remaining standards blockers; the deterministic lifecycle harness covers two fresh sessions, settled logout, explicit input gating, retry/error boundaries, and comparison outcomes.

Remaining deferrals are unchanged: broader locomotion/control, terrain/collision, gameplay, multi-client interaction, Windows runtime acceptance, and convenience reconnect recovery beyond this explicit proof path.
