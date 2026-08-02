# Implement role-reversed live machine proof

Type: implementation
Status: open
Blocked by: [Implement proof-aware Pair client boundary](27-implement-proof-aware-pair-client-boundary.md)

## Objective

Run one lock-held, reset-scoped macOS Role-reversed Replication Proof that
proves serial Pair A-to-B and Pair B-to-A observer semantics, retains an
immutable Machine Attempt, and leaves the Reference Realm healthy.

## Entry gate

Ticket 27 is resolved, macOS Screen Recording is granted, the canonical Realm
and Placement Probe are healthy, and the checkout is clean.

## Scope

- Implement the foreground parent, canonical lock/reset/health flow, exact Pair
  child admission, serial controls, observer-only checks, bounded manual
  checkpoints, two-window ScreenCaptureKit capture, shutdown/removal proof,
  reaping, recovery, and Machine Attempt provenance.
- Use the closed runtime/attempt separation and diagnostics rules.

## Out of scope

- Final human-bound bundle PASS, final attestation validator, LAN/Windows,
  simultaneous movement, persistence proof, peer reconnect/crash handling, or
  generalized process management.

## Acceptance

1. Fake-adapter script tests cover contention, admission, child identity,
   command ordering, deadlines, semantic mismatch, capture faults, cleanup,
   one recovery, retained lock on recovery failure, and no automatic retry.
2. One reset-scoped live macOS run proves two distinct Metal windows, serial
   observer-only Replicated Moves, calibrated timing/tolerance, Pair B removal
   observed by Pair A, Pair A settlement, final reset, and final Realm health.
3. Successful output is an immutable PASS Machine Attempt with closed
   provenance; failed attempts remain separate with redacted diagnostics.
4. Runtime controls and attempt-only diagnostics never enter a final-bundle
   root.
5. Contract/behavior scripts and complete workspace check pass.

## Required evidence

- Retained Machine Attempt with canonical semantic sidecars, capture metadata,
  provenance hash table, and final Realm health; no pixel-only semantic claim.
