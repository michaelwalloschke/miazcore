# Role-reversed Replication Evidence Contract

Ticket: [11 – Specify the replication evidence contract](../issues/11-specify-replication-evidence-contract.md)

## Decision

The Role-reversed Replication Proof has one new, independently validated
`miazcore.shared-host-replication-evidence.v1` bundle. It is separate from and
must not mutate the completed World-entry Acceptance Bundle. The exact clean
candidate SHA must pass every machine gate once, with no skip or substitute.
A failed attempt is retained and exits non-zero; a fresh operator-started
attempt for the same SHA needs a distinct attempt ID and cannot overwrite,
delete, or stand in for that failure.

The bundle is a semantic proof first. A dual-window capture and a manual
attestation are required additional evidence, but neither can establish a
Remote Avatar lifecycle, pose, or identity by itself.

## Bundle layout and integrity

```text
artifacts/shared-host-replication/<attempt-id>/
  candidate_sha
  commands.json
  versions.json
  manifest.json
  pair-a/sidecar.json
  pair-b/sidecar.json
  turns.json
  capture.png
  capture.json
  manual-attestation.json
  report.md
```

`candidate_sha` is a full 40-hex clean Git SHA. `manifest.json` records the
schema, candidate SHA, attempt ID, canonical loopback Realm identity, command
exit results, SHA-256 for every retained regular file except `manifest.json`,
and the explicit deferrals. Its canonical header domain is exactly `schema`,
`attempt_id`, `candidate_sha`, `realm`, `commands`, `deferrals`, and `result`;
its digest is `manifest_header_sha256`. Its `attestation_subject_files` domain
is the sorted bytewise path-to-hash table for immutable machine inputs:
`candidate_sha`, `commands.json`, `versions.json`, both sidecars, `turns.json`,
`capture.png`, and `capture.json`. It excludes `manual-attestation.json`,
`report.md`, and `manifest.json`; its digest is `manifest_files_sha256`. Both
pre-attestation digests appear in the manual attestation. The final `files`
table separately lists every retained regular file except `manifest.json`,
including the completed attestation and report. This prevents a hash cycle
while still binding every final retained file. `manifest.json` is written last
through a temporary sibling and rename. The validator accepts no symlink,
directory, unlisted file, missing hash, hash
mismatch, unexpected field, non-finite number, or schema/version mismatch.
Each later PASS leaves earlier attempt directories untouched.

All JSON objects use exact keys, UTF-8 values, and no optional/additional
fields. `commands.json` has `schema, commands`; `versions.json` has `schema,
candidate_sha, host, rust, realm, fixtures`; each sidecar has `schema,
attempt_id, profile, guid, entry_anchor, movement_ready, events, terminal`;
`turns.json` has `schema, attempt_id, turns`; `capture.json` has `schema,
attempt_id, candidate_sha, capture_sha256, backend, timestamp, windows, dimensions`;
the attestation has `schema, attempt_id, candidate_sha, capture_sha256,
manifest_header_sha256, manifest_files_sha256, checks, result, notes`; and the
manifest has `schema, attempt_id, candidate_sha, realm, commands,
attestation_subject_files, files, deferrals, result`.
Schemas are `miazcore.shared-host-replication-{file}.v1`; GUIDs are non-zero
lowercase hexadecimal shorthand, revisions positive/strictly increasing,
SHA-256 values lowercase 64-hex, and poses exactly `map_id, east, north,
elevation, orientation` with finite numeric coordinates. Events/turns are
ordered arrays with no duplicate sequence/revision; all check/result enums are
only `PASS` or `FAIL`. `windows` is exactly two objects with `profile, pid,
window_id, title`; `dimensions` is `width, height`; and `timestamp` is a UTC
RFC-3339 string. Nested command, version, event, terminal, and turn objects
are versioned closed records owned by their stated file schema; no consumer may
accept an unknown key.

`commands.json` records argv tokens only for repository-owned commands; it
contains no environment, endpoint override, credential path, account, secret,
or arbitrary child command. `versions.json` records the Rust/toolchain/host,
repository commit, locked Realm image/fixture digests, and canonical
`127.0.0.1:3724` / `127.0.0.1:8085` topology. Raw World frames, packet bodies,
database rows/dumps, unrestricted logs, session/cipher material, credentials,
and names beyond the already-approved Fixture Profile labels are forbidden.
The validator redaction-scans both field names and strings before hashing.

## Required semantic sidecars

There are exactly two atomic, immutable sidecars, one for each closed profile:
`pair-a` and `pair-b`. They contain the profile token, GUID shorthand, local
map/Entry Anchor, `MovementReady` completion, and a bounded ordered semantic
event list. Identity is compared only by non-zero GUID; Character names are
display labels and never key a claim.

For each serial role turn, `turns.json` binds a unique increasing revision to
one mover and one observer. It requires:

1. The mover accepted a heading-aligned `2–4 m` intent, published a stopped
   Submitted Pose, and made no other movement write during that turn.
2. The observer was `MovementReady` for the entire turn and records one
   `Created`, at least one `Updated`, and the mover's terminal `Updated` in
   order.
3. The observer's terminal **Realm-observed** pose shares the mover's map and
   is within `0.25 m` (3D Euclidean distance) of the mover's stopped Submitted
   Pose. Its arrival meets Ticket 09's `508 ms` terminal deadline.
4. The reciprocal B-moves/A-observes turn satisfies the identical assertions.
5. Only after both turns, Pair B cleanly logs out while the still-connected
   Pair A records `Removed` within Ticket 09's `19,760 ms` deadline. Pair A's
   later shutdown is settlement only, not a second observed-removal claim; no
   peer reconnect or third observer is introduced.

Every pose declares its source: `submitted` belongs to the controlled mover;
`realm_observed` belongs only to an observer-side Remote Avatar event; and
`rendered` is capture/diagnostic-only. The validator rejects a submitted,
predicted, or rendered pose in place of a Remote Avatar observation, an
unmatched GUID, map mismatch, duplicate/reordered lifecycle, faulted lifecycle
reported as success, stale command revision, or a second accepted Avatar.

## Capture and manual extension

`capture.png` is one controlled compositor capture containing exactly the two
independent native windows. `capture.json` binds their process IDs/profile
tokens, window IDs/titles, dimensions, capture backend, timestamp, and the
PNG SHA-256. It must prove plausible non-black project-owned window content
and show both local capsules plus the counterpart Remote Avatar marker,
GUID shorthand, `OBSERVED` and `RENDERED` pose rows, and lifecycle state. It
is rejected if full-screen, a wrong/duplicate window, an unbound PNG, an
implausibly small/black image, or a missing macOS permission/back-end.

The SHA-bound manual attestation extends the existing macOS checklist with:

- two distinct Pair Profile titles/local identities;
- both amber Remote Avatar markers and GUID shorthand after `Created`;
- serial A-then-B movement, where the other window's `OBSERVED` changes before
  its `RENDERED` projection;
- visible `PROJECTION SNAP` on the scripted large/map correction case;
- `Removed`/`ABSENT` after the observable final clean logout, or a redacted `FAULT` panel with
  no stale marker; and
- normal local camera/focus controls without equating visual motion to Realm
  evidence.

All answers must be `PASS`. The attestation must match the bundle attempt ID,
candidate SHA, `capture.png` SHA-256, `manifest_header_sha256`, and
`manifest_files_sha256`. Human review cannot waive a missing machine
assertion or be copied from a different attempt at the same SHA.

## Fail-closed outcome and deferrals

Any absent, malformed, conflicting, stale, secret-bearing, or semantically
insufficient evidence fails the attempt, retains a redacted failure summary,
performs the orchestrator's one recovery, and cannot generate `report.md` as a
pass. A valid Remote Avatar `Faulted` outcome is evidence of fault handling,
not a successful replication turn.

This contract does not add Windows runtime acceptance, LAN/cross-host proof,
more than two clients/one accepted avatar, simultaneous movement, peer crash
or reconnect policy, name-query, general object replication, terrain, or
gameplay. It also does not reopen World-entry Acceptance or copy its curated
bundle into this new proof.

## Verification required by implementation

1. Pure validator tests cover every required field, SHA binding, redaction,
   symlink/extra-file rejection, wrong SHA, swapped profiles/GUIDs, stale
   revisions, map/pose/timing violations, lifecycle reorder, and a rendered
   pose substituted for Realm-observed truth.
2. Scripted sidecars cover both successful serial turns, `Faulted`, capture
   absence/blackness/wrong-window, malformed attestation, and retention of a
   failed attempt.
3. The live proof runs only under the canonical Realm lock, captures two real
   Metal windows, preserves the semantic sidecars, validates the bundle, and
   ends with final Realm health.
