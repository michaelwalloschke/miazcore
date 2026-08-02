# Manual two-window acceptance

Ticket: [18 – Extend the manual two-window acceptance](../issues/18-extend-manual-two-window-acceptance.md)

## Decision

The manual extension is a short, repeatable macOS review inside the one
lock-held Role-reversed Replication Proof attempt. It uses Ticket 17's bounded
`Manual*Review` checkpoints while its original Pair A/B children are still
alive; it never launches a fresh pair or replays a completed role turn. Once
the original machine proof has completed its shutdown, settlement, and final
health, the recorded review is bound to the retained attempt. It is an
additional readability and interaction review, not a way to establish
Remote Avatar identity, lifecycle, timing, or pose truth. Those claims remain
the automated sidecar, turn, and capture validators' responsibility.

The reviewer follows the closed [checklist](#closed-checklist) at those
checkpoints. The controller continues to own the Realm lock, Pair A/B commands,
role-turn ordering, capture, shutdown, offline settlement, and recovery. The
reviewer neither opens a second client nor edits a command/sidecar, Realm data,
or retained artifact. If the original run does not reach the requested visual
state in its bounded phase, the answer is `FAIL`; it is never retried inside
the same attempt.

## Admission and reviewer setup

1. Select the controller-owned active attempt before its `ManualReadyReview`.
   It must already have a full candidate SHA, Pair A/B identities, and the
   immutable pre-mutation `versions.json`. The controller has structurally
   validated its closed machine inputs, but the final Ticket 11 manifest cannot
   exist before the later attestation. Reject a missing, failed, mutable,
   different-SHA, or incomplete active attempt before reviewing windows.
2. Confirm macOS Screen Recording is available to the repository-owned capture
   adapter. Place the two native windows side by side without resizing them to
   obscure their title or inspector. The reviewer may move, focus, orbit, and
   zoom windows, but must not cover either with another application during the
   controlled capture.
3. The foreground controller launches exactly Pair A and Pair B with the
   closed profile selector and displays their stable titles:

   ```text
   Miazcore — Diagnostic World — PAIR A
   Miazcore — Diagnostic World — PAIR B
   ```

   The reviewer accepts only these two titles and two independent processes.
   Profile tokens and their existing sanitized local labels may be read; no
   account, credential, endpoint, file path, Character name beyond that label,
   raw World frame, database row, or log is requested or recorded.
4. Wait for the controller's `BothReadyAndPeersPresent` state. Do not infer
   readiness from a capsule alone. The controller's pre-existing semantic
   `MovementReady` and `Created` assertions are the admission evidence; the
   human only confirms their intended presentation.

## Closed checklist

Each check has exactly one result: `PASS` or `FAIL`. A missing observation,
unclear label, obscured window, apparent stale marker, controller fault, or
attempt mismatch is `FAIL`. Free-text notes can describe only non-sensitive
visual/control observations and cannot waive a failed check.

| Attestation key | Human observation | Required visible result |
| --- | --- | --- |
| `distinct_pair_windows_and_local_identities` | Inspect both title bars and cockpit headers while side by side. | Exactly one `PAIR A` and one `PAIR B` native window; each shows its own cyan local capsule and local identity label. |
| `remote_marker_guid_and_presence_readability` | After the controller's ready barrier, inspect both worlds and remote panels. | Each window shows one warm-amber, non-capsule Remote Avatar Marker, counterpart GUID shorthand, and `PRESENT`; cyan/amber shape distinction is readable without colour alone. |
| `observer_observed_then_rendered_projection` | During the scripted A turn and then the scripted B turn, watch the non-moving window. | In each direction, the observer's `OBSERVED` remote row changes before or independently of its `RENDERED` projection; no row calls rendered data authoritative. This is a visual ordering/readability check, not a timing assertion. |
| `heading_and_pose_source_distinction` | Compare the moving local capsule/heading arrow with the observer's amber marker/heading arrow and inspector rows. | Local and remote headings are separately visible; remote `OBSERVED` and `RENDERED` rows show map/east/north/elevation/orientation and remain distinct from the mover's local truth panel. |
| `scripted_projection_snap_readability` | Observe the controller-provided scripted same-map large-distance correction demonstration at `ManualSnapReview`. | `PROJECTION SNAP` is visible; the marker then returns to its unchanged Realm-observed `PRESENT` state before the controlled capture. Map-context hiding remains deterministic/headless coverage rather than a live-capture phase. |
| `clean_logout_removal_readability` | After both role turns and the controlled capture, observe only Pair B's controller-requested clean shutdown from Pair A. | Pair A visibly records `Removed`, its amber Pair B marker disappears, and its remote panel becomes `ABSENT` with no pose rows. A redacted `REMOTE AVATAR FAULT` with no stale marker is a fault demonstration, never a successful removal check. |
| `camera_focus_controls_remain_local_only` | Use right-mouse orbit, wheel zoom, focus switching, and the allowed local camera controls before the controller freezes the role turn. | Camera/focus interaction remains responsive in the focused window and does not create, move, relabel, or remove the Remote Avatar Marker; visual movement is not described as Realm proof. |

The controller must keep each reviewed phase visible long enough for a normal
operator to make the observation. The reviewer does not use independent input
to move a Fixture during role turns, does not test simultaneous movement, and
does not request saving/reconnect, peer-crash, reconnect, LAN, Windows, or
multi-peer behaviour.

## Attestation and finalization

After the original machine proof has produced its immutable machine inputs, the
controller derives the fixed Ticket 11 header digest and the pre-attestation
`attestation_subject_files` digest, then supplies those two values to the
reviewer. The reviewer copies
[`docs/shared-host-replication-manual-attestation.template.json`](../../../docs/shared-host-replication-manual-attestation.template.json)
to a new file outside the retained attempt, fills the exact `attempt_id`,
40-hex `candidate_sha`, `capture_sha256`, `manifest_header_sha256`, and
`manifest_files_sha256` values supplied by the controller, sets every check to
`PASS`, and supplies concise redacted notes. The placeholder file is never
edited in place and the reviewer does not manufacture digests.

Finalization first validates the completed closed attestation against the
machine attempt and Ticket 11 redaction/schema rules. It then follows Ticket
22's byte-copy-and-rehash curation: only provenance-listed canonical sources
and `machine-provenance.json` enter a temporary final-bundle root; attempt-only
diagnostics and runtime controls never do. It writes the accepted attestation,
the derived `report.md`, the final sorted files table (including both files),
and the manifest last. The attestation binds only the earlier immutable header
and `attestation_subject_files` digests, so it does not hash itself. Full
validation still requires hashes for every final retained regular file. A
`FAIL` attestation, failed binding, bad digest, missing check, extra key,
secret-bearing note, or failed bundle validation creates Ticket 22's separate
redacted finalization-failure record, leaves the closed Machine Attempt's
machine result unchanged, and cannot produce a passing report. Human approval
cannot replace a missing automated assertion.

## Explicit deferrals

- A second broad World-entry Acceptance, a screenshot-only result, or manual
  measurement of the automated `331 ms`, `508 ms`, and `19,760 ms` bounds.
- Remote prediction, a second accepted Remote Avatar, general object/NPC
  visibility, models, terrain, combat, chat, or gameplay UI.
- Peer crashes, reconnect policy, persistence proof, LAN/cross-host operation,
  and Windows runtime acceptance.

## Verification required by later implementation

1. Attestation schema tests reject missing/extra/non-`PASS` fields, wrong
   attempt/SHA/capture/manifest digests, encoded material, credential words,
   paths, account values, and an attestation copied from another attempt.
2. Finalization tests prove it cannot create `report.md` or a PASS manifest
   before the closed machine proof, a complete manual attestation, and all
   Ticket 11 hashes validate; failed finalization retains evidence without
   overwriting a prior attempt.
3. A scripted controller/UI test keeps the two required windows and each
   review state visible, rejects reviewer movement during a role turn, and
   proves camera/focus controls cannot mutate Remote Avatar session truth.
4. The macOS live acceptance uses two real Metal windows during the bounded
   review checkpoints of the automated role-reversed proof, then retains the
   SHA-bound manual attestation as corroborating evidence only.
