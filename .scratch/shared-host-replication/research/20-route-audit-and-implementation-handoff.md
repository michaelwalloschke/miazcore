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

The former evidence-layout contradiction is resolved by [Ticket 22 – Decide
machine-attempt curation for final evidence](22-machine-attempt-curation.md).
The seven-slice implementation handoff is now ready: Machine Attempts and
Final Evidence Bundles have separate closed layouts and a hash-bound curation
boundary.

## Confirmed route

| Area | Governing decision | Handoff status |
| --- | --- | --- |
| Fixture Pair reset, profiles, placement, lock, recovery | Tickets 2, 5, 6, 12, 21 | Implemented prerequisite; revalidate at every later live gate. |
| Remote World-Update framing and decoding | Tickets 3, 8, 13 | Slice 1 has a bounded complete-frame-only seam and no general-object expansion. |
| One accepted Remote Avatar session truth | Tickets 4, 8, 14 | Slice 2 has typed GUID ownership, lossless event/snapshot ordering, and a fail-closed fence. |
| Diagnostic World projection | Tickets 7, 15 | Slice 3 has one marker, source-separated poses, deterministic smoothing/snap, and no feedback into session truth. |
| Deterministic composition | Tickets 9, 16 | Slice 4 has test-only encrypted scripts, fake clock, and observer-only truth assertions. |
| Per-client and parent proof control | Tickets 10, 17, 18 | Slices 5–6 have exact Pair admission, serial roles, bounded manual checkpoints, ScreenCaptureKit capture, cleanup, and Realm health. |
| Final human-bound evidence | Tickets 11, 18, 22 | Slice 7 has a closed attestation, non-cyclic pre-attestation digest domains, provenance-checked curation, and final-manifest rules. |

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

## Resolved machine-attempt curation boundary

Before Ticket 22, Ticket 17 placed additional regular files and a directory
needed for operating/debugging the machine proof in its successful attempt
root:

    pair-a/command.json
    pair-b/command.json
    machine-summary.json
    logs/

Ticket 11, in contrast, defined the final bundle root as a closed layout and
requires its validator to reject an unlisted regular file or directory. Its
final-evidence vocabulary does not define schemas or manifest membership for
the per-client command files, machine-summary.json, or allowlisted logs.
Ticket 18's finalization sequence now uses Ticket 22's byte-copy-and-rehash
curation and does not admit those machine-only files to the final bundle.

Ticket 22 resolves this by keeping runtime command files ephemeral, retaining
accepted control history in `commands.json`, preserving redacted diagnostics
only in a Machine Attempt, and copying only provenance-listed canonical files
to a separate Final Evidence Bundle. No implementation ticket must change that
boundary.

## Implementation handoff

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
ticket may weaken the resolved curation boundary, introduce a second peer, or
broaden protocol/presentation scope.

## Verification required by the handoff

Ticket 22 requires a checker that independently validates the selected layout,
the provenance hash table, source/target equality, separation of retained
attempts, and failure visibility. That verification is an exit requirement of
Slice 7 before the seven implementation tickets can be claimed complete.

## Explicitly not a product change

This audit does not implement the Remote Avatar decoder, session events, Bevy
marker, test harness, proof client, orchestrator, capture adapter, evidence
validator, or final bundle. It neither mutates a Realm nor claims a live
Replication Proof.
