# Implement deterministic replication harness

Type: implementation
Status: open
Blocked by: [Implement Diagnostic World Remote Avatar projection](25-implement-remote-avatar-projection.md)

## Objective

Provide a deterministic, no-Realm test harness proving that the protocol,
retained session, and Diagnostic World projection compose into observer-only
Remote Avatar semantics.

## Entry gate

Tickets 23–25 are resolved with their focused suites passing, and no test-only
helper has been promoted to a public runtime API.

## Scope

- Add crate-local synthetic record builders, encrypted frame and poll scripts,
  fake clock, one-observer scenario driver, and observer-only assertions.
- Exercise framing, lifecycle, timing arithmetic, projection, faults, fences,
  and retry without runtime network, Docker, credentials, window, or real
  clock dependencies.

## Out of scope

- A Realm emulator, live acceptance artifact, sidecar writer, compositor
  capture, process orchestration, or latency/performance claim.

## Acceptance

1. Synthetic fragmented/coalesced encrypted inputs prove exact decoder and
   retained-session behavior across ignored frames, faults, EOF, and errors.
2. The observer-only oracle accepts only ordered remote Created/Updated/Removed
   facts, Remote Avatar Realm-observed terminal Pose, calibrated deadlines, and
   the fixed terminal tolerance.
   The virtual receipt-time boundaries are exactly 331 ms for first observed
   pose, 508 ms for terminal pose, and 19,760 ms for removal: each exact limit
   passes and one millisecond later fails.
3. It rejects mover-local, predicted, Submitted, rendered, database, wrong-GUID,
   stale, map-mismatched, or faulted substitutes.
4. Headless presentation coverage proves projection/fence behavior alongside
   the observer scenario without public test-only runtime APIs.
5. Focused per-crate tests and the complete workspace check pass.

## Required evidence

- Checked-in synthetic vectors and fake-clock tests with no captured traffic,
  credentials, live keys, Docker, TCP listener, GPU window, or wall-clock
  sleeps.
