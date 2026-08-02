# Implement retained-session Remote Avatar truth

Type: implementation
Status: resolved
Blocked by: [Implement minimal Remote-player protocol records](23-implement-remote-player-protocol-records.md)

## Objective

Make one retained World session own a single Realm-replicated Avatar and publish
its lifecycle and Realm-observed Pose through the established session boundary.

## Entry gate

Ticket 23 is resolved, and the retained World loop keeps its current
incremental receive and failure boundaries intact.

## Scope

- Select at most one non-zero Remote Avatar identity from Ticket 23 records.
- Publish typed Created, Updated, Removed, and Faulted changes losslessly with
  source sequence, latest snapshot, and invalidation fence semantics.
- Attach only the authenticated local entry-map context to remote poses.

## Out of scope

- Bevy entities, smoothing, sidecar export, a second accepted avatar, peer
  reconnect/crash policy, and generic subscriptions.

## Acceptance

1. Create/update/remove/out-of-range and unusable-record outcomes follow the
   closed one-GUID lifecycle without foreign GUIDs mutating selected truth.
2. Snapshot/event publication is atomic; FIFO saturation fails the World
   session, clears remote state, and prevents queued old events from revival.
3. A malformed Remote-player decoder result fails the whole retained session,
   while Time Sync and clean retry state remain correct.
4. Controlled-character predicted, Submitted, and Realm-observed Pose remain
   distinct from the Remote Avatar's Realm-observed Pose.
5. Focused session tests and the workspace check pass.

## Required evidence

- Fake-clock encrypted-frame tests for lifecycle ordering, faults, map absence,
  backpressure/fence, fragmentation, EOF/read failure, and retry isolation.

## Answer

The retained World worker now decodes every complete inbound frame through the
Ticket 23 boundary and owns one selected `RemoteAvatarId`. It publishes
lossless Created, Updated, Removed, and Faulted changes with the global event
sequence, while its latest snapshot retains only the selected GUID,
authenticated-entry-map pose, and source sequence. Foreign records cannot
alter selected truth.

Both remote and generic FIFO saturation clear Remote Avatar truth; only a
failed Remote Avatar enqueue advances its invalidation fence. Framing and
decoder failures also clear that truth before the retained session reaches its
ordinary failure boundary. Focused fake transport/boundary tests cover
lifecycle, faults, missing map, retry, saturation, fragmentation, EOF, and a
malformed complete update. `cargo test -p client_session`, workspace Clippy,
and `scripts/check.sh` pass.
