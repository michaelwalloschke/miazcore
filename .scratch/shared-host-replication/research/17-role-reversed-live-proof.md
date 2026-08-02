# Role-reversed live proof

Ticket: [17 – Design the Role-reversed live proof](../issues/17-design-role-reversed-live-proof.md)

## Decision

`scripts/role-reversed-replication-proof.sh` is one foreground, no-retry,
reset-scoped macOS machine gate. It owns the canonical Realm lock for its whole
attempt, starts exactly two direct native-window `learning_client` children,
and proves A-to-B then B-to-A serial replication before one observable Pair B
logout. It is the live complement to Ticket 16's deterministic harness, not a
multiplayer service, a general process manager, or a replacement for Ticket
18's human check.

The script requires a clean candidate checkout and creates one immutable
attempt directory:

```text
artifacts/shared-host-replication/attempts/<utc>-<candidate-sha-prefix>/
  attempt.json
  candidate_sha
  versions.json
  pair-a/sidecar.json
  pair-b/sidecar.json
  commands.json
  turns.json
  capture.png
  capture.json
  machine-provenance.json             # successful machine proof only
  machine-summary.json
  diagnostics.json
```

Its success is a **machine-proof pass pending Ticket 18's manual attestation**.
The bounded human review is a pause inside this same lock-held attempt, after
the ready barrier and around its actual role turns; it never launches a fresh
pair or replays an already completed proof. The script does not create a final
`report.md`, accept an attestation, or claim the full Ticket 11 bundle passed.
Ticket 18 later binds the recorded manual review and manifest finalization to
this retained, successful machine attempt.
`versions.json` is already a closed, run-time provenance input to that future
manifest; manual finalization may read it but must never recreate or replace
it.

## Admission, ownership, and child contract

1. Acquire `.scratch/learning-client/.realm-test.lock` atomically. Contention
   exits `75` before reset, build, child, or capture activity.
2. Record only script, PID, UTC start, candidate SHA, and run identifier in
   the lock owner record. Create the attempt directory mode `0700` and write
   its full 40-hex candidate SHA before a Realm mutation. In the same
   pre-mutation admission phase, collect and atomically close the exact
   Ticket 11 `versions.json`: schema, candidate SHA, host/OS identity,
   Rust/toolchain identity, locked Realm image identity, Fixture Pair digest,
   and canonical `127.0.0.1:3724` / `127.0.0.1:8085` topology. It is produced
   only from allowlisted version commands and sanitized infrastructure facts;
   collection failure, an unexpected key, or a post-write mutation fails the
   attempt before reset. Later manifest finalization hashes this immutable
   file alongside the sidecars, commands, turns, and capture metadata.
3. Run canonical `realm preflight`, then one
   `MIAZCORE_REALM_LOCK_HELD=1 realm reset-state --yes`, followed by health.
   Reset provenance and Fixture Pair placement remain infrastructure facts,
   never Remote Avatar evidence.
4. Build the locked Learning Client once. Validate an exact admission manifest
   of `pair-a,pair-b`; no repeated profile, caller-supplied child command,
   endpoint, account, credential, credential path, environment override, or
   third child is accepted.
5. Start exactly these direct children, with only parent-owned non-secret
   proof paths/options in addition to the closed selector:

```text
learning_client --fixture-profile pair-a --shared-host-proof-dir <run/pair-a>
learning_client --fixture-profile pair-b --shared-host-proof-dir <run/pair-b>
```

The clients privately resolve credentials through the established profile
boundary. Each child owns its World session, cipher, bounded movement, atomic
sidecar, and command acknowledgement; the parent owns PIDs, command files,
capture, lock, reset, and recovery. Child PIDs must differ.

Each client title is exact and stable for capture:
`Miazcore — Diagnostic World — PAIR A` or `Miazcore — Diagnostic World — PAIR B`.
Profile tokens and GUID shorthand are allowed display data; Character names
remain only their existing sanitized local labels and never establish identity.

## Atomic sidecars and command protocol

Every sidecar is written through a temporary sibling and rename, then becomes
immutable for its acknowledged revision. The parent validates its closed
Ticket 11 schema before using it and rejects extra keys, non-finite values,
secret-bearing vocabulary, profile/GUID drift, wrong attempt ID, reordered or
duplicate event sequence, or a second accepted Remote Avatar.

The parent atomically writes `{revision, command}` to each assigned command
file in its 0700 ephemeral runtime workspace below `.scratch/`. Revisions are positive and strictly increasing per profile. Valid
commands are exactly:

```text
idle | perform-role-turn | show-projection-snap | request-clean-shutdown
```

No next command is written until the matching sidecar acknowledgement is
terminal. A stale, duplicate, skipped, wrong-profile, wrong-directory, or
unacknowledged command fails the attempt; it never causes a retry. The closed
`commands.json` ledger retains each accepted command and acknowledgement; the
mutable runtime command files are reaped with the children and never enter an
artifact root.

The ready barrier requires both sidecars to show independently authenticated
`MovementReady`, distinct non-zero GUIDs, same map Entry Anchors, no failure,
and one peer `Created` event for the counterpart GUID. These creation events
may precede the first role turn and are referenced by sequence in both turn
records; the proof must not manufacture a second create between turns.

## Serial machine state

```text
Acquire -> PreflightResetHealth -> StartPair -> BothReadyAndPeersPresent
  -> ManualReadyReview -> A-turn (A moves, B observes) -> ManualAReview
  -> B-turn (B moves, A observes) -> ManualBReview -> ManualSnapReview
  -> CaptureTwoWindows -> B-clean-logout (A observes Removed)
  -> ManualRemovalReview -> A-clean-shutdown -> OfflineSettlement
  -> FinalResetHealth -> MachinePass
```

For every role turn, the parent snapshots each observer event sequence before
atomic command commit, takes `time.monotonic_ns()` immediately after that
rename, and polls sidecars at a bounded 10 ms interval. The first observer
`Updated` after the waterline must arrive within `331 ms`; the terminal
observer `Updated` matching the mover's stopped Submitted Pose must arrive
within `508 ms`. The parent uses this monotonic receipt time as a conservative
same-host upper bound; it never substitutes later sidecar-read time, a client
clock, rendered pose, a log, or database state.

For A-turn, only Pair A accepts `perform-role-turn` and Pair B stays `idle`.
Pair A must acknowledge one 2–4 m heading-aligned intent and one stopped
Submitted Pose; Pair B must remain MovementReady and record its pre-existing
`Created`, at least one post-waterline `Updated`, and terminal
`realm_observed` pose for A's GUID. The two poses must share a map and have
3D Euclidean delta `<= 0.25 m`. B-turn repeats these exact assertions with
roles reversed. No role command may invoke the saving/reconnect persistence
proof, and no result may include or assert a reconnect observation.

At each named `Manual*Review` checkpoint the controller holds both existing
children open and exposes only an acknowledge-or-fail reviewer checkpoint. The
reviewer may use local orbit, zoom, and focus controls, but cannot publish
movement or write a role command. `ManualReadyReview` occurs after both
`MovementReady`/`Created` assertions; `ManualAReview` and `ManualBReview`
occur immediately after their corresponding actual serial turn, with the
observer's changed `OBSERVED` and projected `RENDERED` values still visible.
`ManualSnapReview` presents a project-owned fixed same-map large-distance
correction diagnostic, labelled as presentation only and never as Realm
evidence. It is produced by one parent-owned `show-projection-snap` command to
the selected still-present observer after B-turn: the child applies that
scripted correction to its Remote Pose Projection, emits one closed diagnostic
acknowledgement, restores its `PRESENT` marker to the unchanged
Realm-observed pose before capture, writes no movement frame, and neither
changes its Realm-observed pose nor claims an extra Remote Avatar event. The
parent rejects that command at every other phase, from a mover, or without the
expected acknowledgement. The map-context hide is deterministic/headless
coverage only, so it cannot invalidate the required two-marker live capture.

After B's matching semantic `Removed` arrives, `ManualRemovalReview` holds the
still-connected Pair A window long enough for the reviewer to see `ABSENT` and
the cleared marker/pose rows before A shutdown. An unacknowledged review
deadline, failed review signal, child fault, malformed snap acknowledgement, or
input publication attempt fails the same attempt and follows its one recovery
path; no review phase replays a turn or starts another pair.

After both turns, both live windows remain open long enough for capture. The
parent then asks only Pair B for clean shutdown. Pair A must record matching
`Removed` within `19,760 ms` from Pair B's acknowledged shutdown request.
Pair A's later shutdown is settlement only: no second removal claim, peer
reconnect, or third observer is introduced.

## Two-window compositor capture

After B-turn passes and before B shutdown, a repository-owned macOS adapter
captures exactly the two active child windows by PID and their exact titles.
It uses ScreenCaptureKit desktop-independent window filters, not a desktop or
full-screen capture, and composites only those two returned window images into
`capture.png`. It retains each `CGWindowID`, title, PID, profile, backend,
dimensions, UTC timestamp, and PNG SHA-256 in `capture.json`.

The adapter fails closed for missing Swift/ScreenCaptureKit/Screen Recording
permission, no exact window, duplicate/wrong title, absent compositor frame,
wrong image dimensions, implausibly small PNG, black/near-empty content, or a
capture acknowledgement/client-exit race. The children keep rendering until
the adapter has atomically acknowledged success. The capture validator requires
two different PID/window-ID/title/profile records and non-black project-owned
content; it does not infer semantic lifecycle from pixels.

The captured state must show two distinct local capsules, both Remote Avatar
markers, GUID shorthand, `REALM-OBSERVED` and `RENDERED` remote rows, and
`PRESENT` lifecycle. This is visual corroboration only. The subsequent B
logout's `Removed` proof remains semantic sidecar evidence.

## No-retry failure and cleanup

Every timeout, child exit, missing/malformed sidecar, semantic mismatch,
capture failure, or Realm-health failure stops coordination immediately. The
attempt directory is retained with a closed redacted `machine-summary.json`
whose result is `FAIL`; it cannot be finalized as a PASS bundle.

The parent sends clean shutdown only where a child remains responsive, then
sends TERM, waits 10 seconds, sends one KILL/reap attempt, and records only
the terminal reaping result. It waits at most 60 seconds for Pair offline
settlement; process exit alone never proves settlement. It performs exactly
one same-owner reset plus health recovery attempt:

- If recovery succeeds, release the lock and exit non-zero. Do not re-run any
  admission, role, capture, or validation phase.
- If recovery fails or a child cannot be reaped, retain the lock and a redacted
  recovery marker. A later operation fails closed until a human repairs health
  and removes that exact lock directory under the existing recovery procedure.

On machine success, request A shutdown after B's observed removal, wait offline
settlement, run one final canonical reset and health check, write
`machine-summary.json` with machine result `PASS`, then atomically write the
Ticket 22 `machine-provenance.json` over the canonical source hashes, and only
then release the lock. `EXIT`, `INT`, and `TERM` use the same cleanup path.

## Required script validation

Contract/behavior tests must use fake realm/client/capture adapters to prove:

1. lock contention exits `75` before mutation/build/spawn; exact Pair
   admission yields two distinct direct PIDs and no forbidden child arguments;
2. command atomicity and sequence validation, both serial role directions,
   shared create-waterline references, and exact `331/508/19,760 ms` boundary
   pass/fail behavior under a monotonic controller clock;
3. observer-only terminal comparison rejects mover Submitted/Predict/Rendered,
   wrong GUID/map, faulted lifecycle, absent removal, and any persistence-proof
   or reconnect claim;
4. capture accepts only two exact non-black desktop-independent windows and
   rejects full-screen/wrong/duplicate/permission/empty/ack-race cases;
5. every fail path retains a redacted attempt, reaps children, makes exactly
   one recovery attempt, never retries, and retains the lock on failed recovery;
6. success reaches final reset and Realm health before an immutable machine
   result `PASS` and its `machine-provenance.json` are written.

## Explicit deferrals

- Ticket 18's human checklist, manual attestation, manifest finalization, and
  final full-bundle PASS claim.
- Peer crash/reconnect convenience, automated re-run, simultaneous movement,
  multi-peer scale, LAN/cross-host, Windows runtime acceptance, and gameplay.
- Any saving/reconnect persistence re-proof, database-as-replication evidence,
  name/model decoding, terrain, collision, or general object support.
