# Harden and Accept the World-entry Candidate

Type: implementation
Status: ready-for-agent
Blocked by: [Prove Persisted Movement and Recover Explicitly](18-prove-persisted-movement.md)

## Parent

[Define the implementation slices and scope gates](09-define-implementation-slices.md), capability slice 8.

## What to build

Add no new product behavior. Close only the coverage, reliability, and evidence gaps required for one clean production candidate to pass the complete World-entry Acceptance contract. Produce a validated, hashed, redacted Acceptance Evidence Bundle tied to the exact accepted commit.

## Entry gate

- [ ] Ticket 18 passes deterministically and through the real production application.
- [ ] Its canonical Movement Proof, two negative probes, remaining deferrals, and exact predecessor commit are recorded.

## Acceptance criteria

- [ ] Missing golden, malformed-input, property, and fuzz-regression coverage required by the accepted protocol boundary is complete.
- [ ] Missing deterministic session, queue-pressure, timeout/EOF/rejection/write-fault, shutdown/retry, and Movement Proof coverage is complete.
- [ ] Missing Bevy adapter, ordering, phase/input gating, event/snapshot projection, correction, visible failure, dependency-boundary, and redaction coverage is complete.
- [ ] Repository-owned gate commands and the exclusive Reference Realm lock are consolidated and deterministic.
- [ ] The deterministic core gate passes codecs, crypto, framing, session transitions, queue pressure, malformed input, redaction, and movement behavior without real sockets, sleeps, Docker, Bevy rendering, or timing tolerances.
- [ ] The Bevy/platform gate passes formatting, locked native workspace/all-target compilation, Clippy with warnings denied, native tests, dependency assertions, `MinimalPlugins` scenarios, scripted Metal smoke, and the Windows MSVC compile tripwire.
- [ ] The isolated live Reference Realm gate passes clean bootstrap, exact fixture identity, authenticated world entry, authoritative self state, the reset-scoped canonical Movement Proof, and both negative probes using reconnect as the sole success oracle.
- [ ] The manual macOS gate passes real entry, accepted viewport/camera/focus controls, smooth movement, visible Rendered/Submitted/Realm-observed diagnostics, correction/failure presentation, Movement Proof, and clean disconnect.
- [ ] One clean `candidate_sha` passes all four gates without skips, expected failures, automatic retries, warning suppression, secret/session leakage, or missing evidence.
- [ ] Every attempt remains visible; a later pass does not erase earlier diagnostic failures.
- [ ] The Acceptance Evidence Bundle contains the accepted identity, commands, versions, results, semantic diagnostics, manual attestation, hashes, and explicit deferrals while excluding credentials, session material, and raw packet dumps.
- [ ] The bundle validates against its manifest, is hashed, curated, and recorded in an evidence-only commit.
- [ ] Disposable prototype code is removed only where equivalent production evidence now exists; its research records and referenced evidence remain.
- [ ] The final exit evidence and exact accepted commit are recorded.

## Explicit deferrals

- All new gameplay, content, multiplayer, LAN exposure, broader packet or movement coverage, and broader world state.
- Authored-content polish, general polish, unmeasured optimization, and public distribution work.
- Windows native build, test, render, and runtime acceptance; that becomes a separate post-acceptance branch before multiplayer.

## Shared scope and evidence rules

- Preparation may happen in parallel, but this ticket cannot integrate until every declared blocker has passed its exit gate on the current branch.
- Hardening closes agreed coverage and evidence gaps only; it is not a destination for functionality omitted from tickets 12 through 18.
- Fix only defects required by an already-agreed acceptance gate. New capabilities remain deferred.
- Do not waive, mute, retry away, or relabel a failing gate.
- Add an unmodeled opcode only if the live acceptance path proves it blocks an existing gate.
- Reopen Bevy only for an already-accepted platform reliability or substantial authored-content/UI trigger.
- Keep upstream protocol incompatibilities inside `client_protocol`; do not expose generated types, fork dependencies, or widen scope without a new decision.
- Work and verify all four gates on one candidate, then run `/code-review` and create the evidence-only acceptance commit.
