# Slice 15 Entry-gate Evidence

Ticket: [Reach MovementReady with Authoritative Self State](../issues/15-reach-movement-ready.md)

## Candidate

- Exact predecessor commit: `00d313ffdde6fa56da56d2c7f932d9660e5b013b`
- Predecessor ticket: ticket 14, resolved
- Entry-gate worktree state before verification: clean

## Required verification

- [x] Deterministic and routine ticket-14 gates pass on the exact predecessor.
- [x] The live Reference Realm character-selection gate passes on the exact predecessor.

## Result

Passed on 2026-07-22 before any ticket-15 behavior changed:

- `scripts/check.sh` passed formatting, locked workspace/all-target checks, Clippy with warnings denied, native tests, dependency boundaries, redaction checks, and the Windows compile tripwire.
- `scripts/live-character-selection.sh` rebuilt the label-scoped Reference Realm from fresh state, passed health and fixture checks, classified nonexistent-account and absent-character probes, selected exactly one `Miaztest`, and disconnected before `CMSG_PLAYER_LOGIN`.

The ticket-15 integration gate is open on exact predecessor `00d313ffdde6fa56da56d2c7f932d9660e5b013b`.
