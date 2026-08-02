# Machine-attempt curation for final evidence

Ticket: [22 – Decide machine-attempt curation for final evidence](../issues/22-decide-machine-attempt-curation.md)

## Decision

A Role-reversed Replication Proof has three physically separate ownership areas:
an ephemeral controller workspace, one immutable retained Machine Attempt, and,
only after successful human-bound finalization, one immutable Final Evidence
Bundle. They never share a root directory.

The controller's current command files are coordination state, not evidence.
Their complete redacted command history is instead committed into the closed
machine commands record. Machine diagnostics are retained with the Machine
Attempt but never copied into the Final Evidence Bundle. The final bundle
contains only closed-schema canonical proof inputs, one closed machine
provenance record, the manual attestation, report, and final manifest.

## Exact paths and allowed contents

    .scratch/shared-host-replication/runtime/<attempt-id>/
      pair-a/command.json
      pair-b/command.json

    artifacts/shared-host-replication/attempts/<attempt-id>/
      attempt.json
      candidate_sha
      versions.json
      commands.json
      pair-a/sidecar.json
      pair-b/sidecar.json
      turns.json
      capture.png
      capture.json
      machine-provenance.json             successful machine proof only
      machine-summary.json
      diagnostics.json                    redacted machine diagnostics only

    artifacts/shared-host-replication/bundles/<attempt-id>/
      candidate_sha
      versions.json
      commands.json
      machine-provenance.json
      pair-a/sidecar.json
      pair-b/sidecar.json
      turns.json
      capture.png
      capture.json
      manual-attestation.json
      report.md
      manifest.json

    artifacts/shared-host-replication/finalization-failures/<attempt-id>/
      finalization-failure.json

All roots are mode 0700. Attempt IDs are unique UTC-plus-candidate-SHA values.
No symlink, hard-link, unlisted directory, or extra file is allowed in any
final bundle. The only directories inside a Final Evidence Bundle are pair-a
and pair-b, each containing exactly sidecar.json.

The runtime root is created by the parent immediately after lock acquisition.
It owns atomic command-file replacement and is removed only after both tracked
children have a terminal reap result. It is never retained under artifacts.
The closed commands.json ledger records every accepted parent command as
profile, revision, command, acknowledgement result, and allowed timestamp; it
is the preserved parent command/control record. A stale, rejected, or
unacknowledged runtime command is recorded only through machine-summary.json
and cannot be represented as an accepted ledger entry.

## Retained Machine Attempt

attempt.json is an atomically written closed record with schema, attempt ID,
candidate SHA, UTC start, and machine result. Its result is only PASS or FAIL.
machine-summary.json is a closed redacted record with schema, attempt ID,
machine result, terminal phase, cleanup/recovery outcome, and final Realm
health result. diagnostics.json is a closed, bounded list of redacted
allowlisted phase diagnostics: preflight, reset, build, child, capture,
cleanup, or recovery. It has no child stdout/stderr, arbitrary command log,
endpoint override, account value, secret path, credential, raw frame,
database row, or session/cipher material.

A failed attempt always retains attempt.json, machine-summary.json, and
diagnostics.json. It may retain whichever canonical source inputs existed
before failure, but has neither machine-provenance.json nor a final bundle.
A later attempt always has a new attempt ID and cannot delete, rename, modify,
or stand in for that failure.

A successful machine attempt has all canonical inputs and writes
machine-provenance.json only after final Realm health. This closed record has
schema, attempt ID, candidate SHA, canonical Realm identity, machine result
PASS, and a sorted path-to-SHA-256 table for exactly candidate_sha,
versions.json, commands.json, both sidecars, turns.json, capture.png, and
capture.json. Its table digest is the machine subject digest. It has no
workstation path, runtime PID, log, credential, or manual-attestation field.
The writer computes every source hash after the last machine write, validates
the source tree as regular files only, then writes provenance through a
temporary sibling and rename. Canonical source files and provenance are
immutable after that point.

## Final Evidence Bundle and binding

Finalization accepts only an attempt with machine-provenance.json whose
candidate, attempt ID, Realm identity, result, path set, and every source hash
validate. It creates one private temporary sibling below bundles, byte-copies
only the eight provenance-listed source files plus machine-provenance.json,
and re-hashes each target before it can be named as the final bundle. A bundle
never references a local source path; equality is proven solely by attempt ID,
candidate SHA, and exact file hashes.

Ticket 11's pre-attestation subject table is updated to contain the eight
canonical source files plus machine-provenance.json. Its digest can be computed
without the attestation, report, or manifest. The provenance table inside
machine-provenance.json contains only the eight source files and therefore has
no self-hash. The manual attestation binds the pre-attestation subject digest,
capture digest, candidate SHA, and attempt ID. This creates no cycle:

    machine source hashes -> machine-provenance hash -> attestation subject digest
    attestation + report + all bundle leaves -> final manifest files table

After a completed PASS attestation validates, finalization writes
manual-attestation.json and report.md to the temporary bundle, computes the
final files table for every regular final-bundle file except manifest.json,
then writes manifest.json last through rename and atomically publishes the
bundle directory. The final manifest never contains attempt-only summary,
diagnostics, or runtime-control content. The validator rejects an incomplete
temporary root, a published duplicate bundle ID, a source/target hash mismatch,
an attempt ID or candidate mismatch, any unexpected path, or a manifest that
omits machine-provenance.json.

If curation or finalization fails, the temporary bundle is discarded before
publication and one create-once finalization-failure.json is written in the
separate finalization-failures sibling root with a redacted category and the
attempt ID/candidate SHA. No PASS report or bundle is created, the closed
Machine Attempt is never changed, and no automatic rerun occurs. A human
cannot repair the evidence by editing files; a fresh full Machine Attempt is
required.

## Contract amendments

- Ticket 11's final bundle root is bundles/<attempt-id>, not the Machine
  Attempt root. It adds machine-provenance.json to its exact allowed layout
  and pre-attestation subject set.
- Ticket 17's proof root is attempts/<attempt-id>. It writes runtime command
  files outside artifacts, retains only commands.json as accepted control
  evidence, and replaces the logs directory with diagnostics.json.
- Ticket 18 finalizes only by the byte-copy-and-rehash curation flow above; it
  never copies arbitrary Machine Attempt files.
- Ticket 20's handoff is now implementation-ready. Ticket 19 Slice 6 produces
  the retained Machine Attempt and Slice 7 performs final curation.

## Verification required by implementation

1. Pure validators reject every extra file/directory, symlink, hard-link,
   unknown schema key, malformed result, duplicate ID, mutable source,
   missing provenance member, source/target hash mismatch, and provenance
   table mismatch.
2. Scripted curation tests prove accepted command history is preserved in
   commands.json while command.json runtime files and diagnostics never enter
   the bundle.
3. Failure tests prove failed machine attempts, failed curation, failed manual
   attestation, cleanup failure, and a later successful attempt remain separate
   and visible; no operation overwrites an earlier root or appends to a closed
   Machine Attempt.
4. End-to-end finalization tests prove the attestation cannot bind another
   attempt with the same candidate SHA, cannot create a digest cycle, and
   produces a fresh-process-valid bundle only after final Realm health.
5. Redaction tests cover every attempt and bundle file, including diagnostic
   values and report text, before hashing or publication.

## Deferred

This decision does not add a transport, decoder, session event, presentation
entity, capture backend, retry policy, Windows runtime proof, LAN behavior, or
gameplay feature.
