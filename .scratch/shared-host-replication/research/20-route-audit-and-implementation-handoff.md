# Shared-Host Replication route audit and implementation handoff

Ticket: [20 – Verify the route and prepare the implementation handoff](../issues/20-verify-route-and-prepare-implementation-handoff.md)

## Audit result

The route is complete for protocol, session, presentation, deterministic
verification, Fixture Pair preconditions, manual review, and live proof
semantics. The destination is corrected to the actually accepted single-removal
scope: both Pair members observe the counterpart appear and perform their
serial Replicated Move, while Pair A observes Pair B disappear after Pair B's
clean logout. Pair A's later shutdown is settlement only, as Tickets 11 and 17
require.

One final evidence-layout contradiction prevents an implementation-ready
handoff. It is isolated as [Ticket 22 – Decide machine-attempt curation for
final evidence](../issues/22-decide-machine-attempt-curation.md). Therefore no
implementation tickets may be created from the seven-slice plan until Ticket
22 is resolved.

## Confirmed route

| Area | Governing decision | Handoff status |
| --- | --- | --- |
| Fixture Pair reset, profiles, placement, lock, recovery | Tickets 2, 5, 6, 12, 21 | Implemented prerequisite; revalidate at every later live gate. |
| Remote World-Update framing and decoding | Tickets 3, 8, 13 | Slice 1 has a bounded complete-frame-only seam and no general-object expansion. |
| One accepted Remote Avatar session truth | Tickets 4, 8, 14 | Slice 2 has typed GUID ownership, lossless event/snapshot ordering, and a fail-closed fence. |
| Diagnostic World projection | Tickets 7, 15 | Slice 3 has one marker, source-separated poses, deterministic smoothing/snap, and no feedback into session truth. |
| Deterministic composition | Tickets 9, 16 | Slice 4 has test-only encrypted scripts, fake clock, and observer-only truth assertions. |
| Per-client and parent proof control | Tickets 10, 17, 18 | Slices 5–6 have exact Pair admission, serial roles, bounded manual checkpoints, ScreenCaptureKit capture, cleanup, and Realm health. |
| Final human-bound evidence | Tickets 11, 18 | Slice 7 has a closed attestation, non-cyclic pre-attestation digest domains, and final-manifest rules, subject to Ticket 22. |

The route preserves these non-negotiable scope gates:

- Only the minimal create/move/out-of-range/destroy World behavior enters
  Slice 1. Complete unrelated frames are structurally consumed/ignored; no
  later slice may add wire behavior as a convenience.
- A non-zero Realm GUID is the only Remote Avatar identity. Names, credentials,
  paths, raw frames, cipher/session material, database rows, and local rendered
  values are never evidence of remote lifecycle or pose truth.
- The live proof is one macOS, loopback-only, lock-held Pair A/B attempt with
  serial role turns. It is not Windows runtime acceptance, LAN proof,
  simultaneous movement, persistence proof, or generalized multiplayer.
- The reviewer observes the original live run at its bounded checkpoints; a
  fresh Pair session cannot be substituted for the hash-bound attempt.
- Only a complete Slice 7 validation may claim final bundle PASS; a capture
  or manual attestation never replaces semantic sidecars.

## Newly revealed blocker: machine attempt versus final bundle

Ticket 17's successful attempt root retains additional regular files and a
directory needed for operating/debugging the machine proof:

    pair-a/command.json
    pair-b/command.json
    machine-summary.json
    logs/

Ticket 11, in contrast, defines the final bundle root as a closed layout and
requires its validator to reject an unlisted regular file or directory. Its
final-evidence vocabulary does not define schemas or manifest membership for
the per-client command files, machine-summary.json, or allowlisted logs.
Ticket 18's finalization sequence says it writes the attestation, report, and
manifest but does not decide how those machine-only files are curated.

Deleting or silently ignoring them would violate retained-attempt/no-overwrite
semantics; adding them to the final bundle without a schema would weaken the
closed evidence contract. Reusing either layout for the other cannot be left to
the future implementation. Ticket 22 owns that exact curation decision.

## Handoff after Ticket 22

Once Ticket 22 resolves the staging/curation boundary, the handoff consists of
the following ordered implementation tickets, each mapped one-to-one to the
accepted slices:

1. Implement minimal Remote-player protocol records.
2. Implement retained-session Remote Avatar truth.
3. Implement Diagnostic World Remote Avatar projection.
4. Implement deterministic replication harness.
5. Implement the proof-aware Pair client boundary.
6. Implement the role-reversed live machine proof.
7. Implement attestation validation and final evidence curation.

Every ticket must cite its corresponding Slice 19 entry/exit gate and retain
all earlier passing gates. The code owner is respectively client_protocol,
client_session, client_bevy, crate-local test seams, learning_client,
repository scripts/capture adapter, and evidence validator/finalizer. No
ticket may absorb the unresolved curation decision, introduce a second peer,
or broaden protocol/presentation scope.

## Verification required before ticket creation

Ticket 22 must establish an exact, validator-enforceable answer for:

1. where successful and failed machine-attempt files live relative to the
   immutable final bundle;
2. which machine facts become canonical closed-schema bundle files and which
   remain redacted diagnostic provenance outside it;
3. how the final bundle binds the selected machine attempt without copying an
   untrusted/mutable file, reintroducing a digest cycle, or allowing a
   finalizer to omit an accepted proof input; and
4. how failure retention, no-retry, lock recovery, and later PASS attempts
   remain visible without allowing an earlier failure to be overwritten.

After that decision, a checker must independently validate the selected layout,
and the seven implementation tickets can be created without a hidden evidence
policy decision.

## Explicitly not a product change

This audit does not implement the Remote Avatar decoder, session events, Bevy
marker, test harness, proof client, orchestrator, capture adapter, evidence
validator, or final bundle. It neither mutates a Realm nor claims a live
Replication Proof.

