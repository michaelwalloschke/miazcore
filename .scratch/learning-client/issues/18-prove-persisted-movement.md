# Prove Persisted Movement and Recover Explicitly

Type: implementation
Status: ready-for-agent
Blocked by: [Predict and Submit Bounded Ground Movement](17-predict-submit-movement.md)

## Parent

[Define the implementation slices and scope gates](09-define-implementation-slices.md), capability slice 7.

## What to build

Turn submitted movement into the sole accepted Movement Proof: after a sufficiently large successfully stopped move, freeze the client, save and log out, establish completely fresh login and world sessions, and compare the reconnect pose with the expected Submitted Pose. The UI must show how the conclusion was reached and provide explicit, redacted recovery for every accepted failure.

## Entry gate

- [ ] Ticket 17 passes deterministically and in its reset-scoped live smoke.
- [ ] The entry evidence, scoped reset evidence, and exact predecessor commit are recorded.

## Acceptance criteria

- [ ] Movement Proof remains unavailable until a successfully submitted stopped pose is at least two metres from the Entry Anchor.
- [ ] **Verify persisted movement** invokes the semantic `BeginMovementProof` operation and immediately freezes input and prediction.
- [ ] The session completes saving logout and the accepted offline wait before discarding the old transport, cipher, protocol time, and session state.
- [ ] Proof always creates fresh authenticated login and world sessions; it never resumes or reuses the previous encrypted streams.
- [ ] The expected oracle is the stopped Submitted Pose, and success requires the reconnect Realm-observed Pose to use the same map and be within `0.25 m`.
- [ ] Success and failure show expected and observed map, both poses, delta, tolerance, and reconnect as the evidence source.
- [ ] Database queries and realm logs may diagnose a failure but cannot create a successful Movement Proof.
- [ ] Reconnect observations apply the accepted smooth or snap presentation treatment without changing the comparison oracle.
- [ ] Every accepted timeout, rejection, EOF, logout/offline-wait failure, reconnect failure, comparison failure, queue failure, and shutdown path is redacted, fail-closed, and provides explicit recovery.
- [ ] Deterministic tests cover proof eligibility, input freeze, saving logout, offline wait, state disposal, fresh reconnect, all comparison outcomes, timeouts, failures, shutdown, and explicit retry.
- [ ] The isolated canonical live scenario moves between two and four metres, successfully submits stop, completes saving logout and fresh reconnect, and passes within tolerance.
- [ ] Two isolated live negative probes prove the accepted failure paths without allowing a database-derived success.
- [ ] Formatting, locked native workspace/all-target checks, Clippy with warnings denied, native tests, dependency-boundary checks, redaction tests, scripted Metal smoke, and the Windows compile tripwire pass.
- [ ] The exit evidence, remaining deferrals, canonical and negative-probe evidence, and exact passing commit are recorded.

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
