# Shared-Host Replication implementation slices and scope gates

Ticket: [19 – Define the Shared-Host Replication implementation slices and scope gates](../issues/19-define-implementation-slices-and-scope-gates.md)

## Decision

The Shared-Host Multi-client Simulation is implemented as seven cumulative
slices. Each slice owns one boundary, has a concrete exit gate, and leaves no
ambiguous partial proof to be treated as acceptance. The completed Fixture Pair
reset/profile work from Ticket 21 is a prerequisite, not a slice to repeat:
every new live slice starts from its canonical reset, lock, Placement Probe,
and three-fixture health contract.

No slice reopens World-entry Acceptance. Native Windows, LAN/cross-host
operation, gameplay, arbitrary object decoding, multiple accepted Remote
Avatars, peer crash/reconnect policy, and persistence re-proofs remain outside
all exits below.

## Dependency order

```text
Fixture Pair reset/profiles (existing prerequisite)
  -> 1 Protocol remote records
  -> 2 Retained-session Remote Avatar truth
  -> 3 Bevy Remote Avatar projection
  -> 4 Deterministic end-to-end replication harness
  -> 5 Proof-aware Pair client boundary
  -> 6 Role-reversed live machine proof
  -> 7 Manual attestation and bundle finalization
```

Slices 1–3 establish production capability in ownership order. Slice 4 proves
their joint semantics without a Realm or renderer. Slice 5 adds only the
closed per-client proof control/evidence boundary; Slice 6 is the first live
Replication Proof; Slice 7 is the only step that can produce the final
Ticket 11 PASS bundle.

## Slice 1 — Minimal Remote-player protocol records

**Owner:** `client_protocol`.

Implement `RemotePlayerDecoder` and the narrow `decode_remote_player_frame`
seam from Ticket 13. It consumes only complete plaintext `WorldServerFrame`
values after the existing incremental decoder and emits the bounded
GUID/ordinary-ground record vocabulary for player create, movement,
out-of-range, destroy, and unusable movement. It fully consumes supported
update containers, including bounded compressed containers, without becoming
a generic object decoder.

**Entry gate:** current World-entry codec/framing suite passes; the pinned
build-12340 sources and checked-in synthetic fixture policy remain unchanged.

**Exit gate:** protocol golden/unit tests prove supported player records,
ignored complete frames/objects, fragmentation and coalescing cipher alignment,
exact compressed consumption, and fail-closed malformed cases. No public API
exposes a cursor, raw update values, names, models, registry, socket, or
credential.

**Verification:** focused `client_protocol` tests run during implementation;
the workspace check runs before handoff. Test data is project-owned synthetic
plaintext, never captured traffic.

**Deferred:** accepted-avatar selection, map attachment, FIFO publication,
rendering, live Pair use, name queries, and every unsupported movement form.

## Slice 2 — Retained-session Remote Avatar truth

**Owner:** `client_session`.

Integrate the Slice 1 seam after each complete inbound retained World frame and
implement Ticket 14's one accepted `RemoteAvatarId`, typed lifecycle changes,
latest snapshot, source sequence, and invalidation fence. The session attaches
only its authenticated entry-map context; it publishes losslessly through the
existing `WorkerBoundary` and fails closed on remote-event FIFO saturation.

**Entry gate:** Slice 1 exits cleanly; the retained World loop and its
incremental receive/error boundaries retain current World-entry behavior.

**Exit gate:** fake-clock/scripted-transport tests prove create/update/remove
and out-of-range ordering, one-GUID capacity, ignored foreign records,
unusable-record faults, missing-map failure, snapshot/event atomicity,
backpressure fence, malformed-decoder `ProtocolError` whole-session failure
that clears accepted Remote Avatar state, Time Sync continuity, and clean retry
state isolation.
Controlled-character predicted, Submitted, and Realm-observed truths remain
bit-for-bit distinct from Remote Avatar truth.

**Verification:** `client_session` focused tests inject encrypted fragmented
frames with a crate-local test encoder; no TCP, Docker, credentials, or wall
clock enters the test path.

**Deferred:** Bevy entities, smoothing, visual diagnostics, sidecar export,
second avatar, peer reconnection, and any generic session subscription.

## Slice 3 — Remote Avatar Diagnostic World projection

**Owner:** `client_bevy`.

Implement Ticket 15's private `RemoteAvatarPresentation`, ingress ordering and
fence handling, one amber marker entity tree, GUID shorthand, inspector/event
text, and exact observed-versus-rendered projection rules. Bevy may consume
only the typed session snapshot/events; it cannot write session, movement, or
semantic evidence state.

**Entry gate:** Slice 2's public session vocabulary and source-sequence fence
are stable; current offline/live local Diagnostic World tests pass.

**Exit gate:** pure and headless Bevy tests prove exact create/hydration,
smooth below `1.628 m`, snap at or above it, short-arc heading, map-context
hide/recovery, removal/fault/offline/fence despawn, bounded diagnostic tail,
and no mutation of local-camera or controlled-character truths. One ingress
drain with more than eight transitions must still reach projection in strict
sequence; only the visible diagnostic tail may retain eight. The Windows
compile tripwire still passes, but this is not Windows render acceptance.

**Verification:** focused `client_bevy` unit/headless tests, then workspace
checks. No native window, Metal capture, or live Realm is required at this
exit.

**Deferred:** model assets, terrain, a second marker, remote prediction,
capture orchestration, manual review, and Windows runtime verification.

## Slice 4 — Deterministic replication harness

**Owner:** crate-local test seams in `client_protocol`, `client_session`, and
`client_bevy`.

Implement Ticket 16's synthetic record builders, encrypted frame/poll scripts,
fake clock, one-observer scenario driver, and observer-only oracle. This
slice adds no runtime protocol or application feature: it proves that the
three production boundaries compose under fragmentation, timing, faults, and
presentation invalidation.

**Entry gate:** Slices 1–3 pass their focused suites; no test-only helper has
been promoted to a public runtime API.

**Exit gate:** the full deterministic matrix passes: exact cipher alignment,
lifecycle/fault/fence state, first/terminal/removal deadline arithmetic,
`0.25 m` observer Realm-observed comparison, snapshot source sequencing,
projection behavior, retry isolation, and rejection of mover/rendered/
Submitted/predicted/database substitutes. The test suite opens no Docker,
socket, real clock, credential file, or Bevy window.

**Verification:** focused per-crate tests first, then the complete workspace
test/check command. The harness produces no acceptance artifact and makes no
live-latency claim.

**Deferred:** a Realm emulator, live server test, sidecar writer, visual
capture, process orchestration, and manual acceptance.

## Slice 5 — Proof-aware Pair client boundary

**Owner:** `learning_client` plus its existing `client_session`/`client_bevy`
bridges.

Add the closed Pair proof mode needed by Ticket 17: fixed `pair-a|pair-b`
profile selection, parent-owned non-secret proof directory, atomic command and
semantic sidecar records, bounded role-turn/control acknowledgements, review
checkpoints, presentation-only same-map snap acknowledgement, and clean
shutdown. The existing private Fixture Profile credential boundary remains the
only route to credentials.

**Entry gate:** Slice 4 passes and Ticket 21's Pair profiles, placement probe,
and canonical Realm recovery are healthy. The CLI rejects unknown profiles,
caller credentials/paths/endpoints, arbitrary proof directories, and unowned
commands before a session starts.

**Exit gate:** client tests prove closed CLI parsing, atomic temporary-sibling
rename, revision/order/directory/profile rejection, redaction, input gating
at review checkpoints, no movement write for the presentation snap, and
clean/offline terminal semantics. Sidecars carry only the Ticket 11 closed
semantic fields and never derive success from pixels, database data, or a
local pose.

**Verification:** CLI/sidecar unit tests, fake session/Bevy integration tests,
and an existing Pair readiness smoke. No live role-reversed success assertion
exists yet.

**Deferred:** parent lock/reset ownership, dual-window capture, manifest
finalization, human attestation, retry, and an externally usable remote-control
API.

## Slice 6 — Role-reversed live machine proof

**Owner:** repository scripts plus the macOS capture adapter.

Implement `scripts/role-reversed-replication-proof.sh` and its contract tests
from Ticket 17. The foreground parent owns one canonical lock, reset/health,
exact Pair A/B child PIDs, serial command commits, observer-only sidecar
assertions, bounded review checkpoints, exact two-window ScreenCaptureKit
capture, B removal observation, child reaping, one recovery, and retained
Machine Attempt artifacts under `artifacts/shared-host-replication/attempts`.

**Entry gate:** Slices 1–5 pass; macOS Screen Recording is granted; the
canonical Realm and Placement Probe are healthy; and the checkout is clean.

**Exit gate:** fake-adapter script tests cover every fail-closed condition,
then one reset-scoped live run reaches immutable machine result `PASS` pending
manual finalization: two distinct
Metal windows, serial A-to-B/B-to-A semantics, `331 ms`/`508 ms`/`19,760 ms`
bounds, `0.25 m` observer-only comparisons, exact capture metadata, Pair B
`Removed`, Pair A settlement, final reset, and Realm health. A failed attempt
is retained and never auto-retried; successful output includes closed machine
provenance while failed recovery retains the lock.

**Verification:** contract/behavior shell tests before the live gate; the
live run uses only the canonical loopback Realm and exact app titles. It does
not claim persistence, LAN behavior, Windows runtime, or pixel-derived
semantic success.

**Deferred:** final PASS report, manual-attestation validation, multi-peer,
peer crash/reconnect, and generalized orchestration.

## Slice 7 — Manual attestation and evidence finalization

**Owner:** evidence validator/finalizer, documentation, and the human
reviewer.

Implement the closed `miazcore.shared-host-replication-manual-attestation.v1`
validator and the Ticket 11/22 two-stage manifest finalizer. Slices 5 and 6
have already implemented and orchestrated Ticket 18's manual checkpoints
inside the original lock-held run; this slice validates and binds that recorded
review to immutable Machine Attempt provenance, byte-copies and re-hashes only
canonical sources into `bundles/<attempt-id>`, then writes the full bundle only
after all hashes validate.

**Entry gate:** a retained Slice 6 machine result `PASS` attempt has immutable
machine inputs, sidecars, two-window capture, versions provenance, final Realm
health, and the fixed pre-attestation header/subject-file digests.

**Exit gate:** all manual checklist values are `PASS`; the attestation matches
attempt, candidate, capture, and pre-attestation digests; the final manifest
hashes every retained regular file; the report is written only after full
validation; and a fresh validator process accepts the bundle. Any malformed,
secret-bearing, mismatched, or failing attestation retains failure and cannot
produce a PASS report.

**Verification:** pure schema/hash/redaction/extra-file/symlink tests, scripted
finalization failures, and one macOS run with the real two-window manual review.
The final evidence records both automatic and human gates without treating the
latter as a substitute.

**Deferred:** a second World-entry Acceptance, manual timing measurement,
Windows runtime acceptance, LAN/cross-host evidence, gameplay, and all
unaccepted remote capabilities.

## Cross-slice scope gates

- New World wire behavior is admitted only in Slice 1 when it is necessary for
  the accepted create/move/remove lifecycle. Every other complete frame is
  consumed/ignored or fails at the owning structural boundary; no later slice
  broadens decoding to unblock convenience work.
- The sole Remote Avatar identity is a non-zero Realm GUID. Names, account
  data, display/model fields, credentials, paths, raw frames, cipher/session
  material, and database rows never cross a slice boundary or enter evidence.
- A slice may add deterministic tests before a live proof, but only Slice 6
  may make a live replication claim and only Slice 7 may make a final bundle
  PASS claim.
- Every live slice uses the canonical lock and reset/recovery procedure. No
  timeout produces an automatic rerun, and no process exit alone proves Realm
  offline settlement or final health.
- A later slice cannot weaken an earlier exit. If a new capability requires a
  new protocol, lifecycle, presentation, evidence, or acceptance policy, it
  starts a new decision ticket rather than modifying this route implicitly.
