# Dual-Client Orchestrator

Ticket: [10 – Design the Dual-Client Orchestrator](../issues/10-design-dual-client-orchestrator.md)

## Decision

`scripts/shared-host-orchestrate.sh` is one synchronous, foreground
repository-owned parent process. It owns the canonical Realm lock for its full
run, starts exactly two direct Learning Client children, and returns only after
their clean terminal cleanup or an explicit retained failure boundary. It does
not daemonize, auto-rerun, share a client process, create a second Compose
project, or use a fake peer.

The only child invocations are the closed selectors:

```text
learning_client --fixture-profile pair-a [orchestrator-owned non-secret proof options]
learning_client --fixture-profile pair-b [orchestrator-owned non-secret proof options]
```

The script never passes a credential, credential path, account, endpoint, or
Realm override. It remains loopback-only through the existing configuration
boundary.

## Ownership and run layout

1. Atomically acquire `.scratch/learning-client/.realm-test.lock`; contention
   exits `75` before Docker, secrets, or children are touched.
2. Create `artifacts/shared-host-replication/<utc-run-id>/` and write the
   non-secret lock owner record before a Realm mutation.
3. Run `infra/azerothcore/realm reset-state --yes`, require health, then create
   `pair-a/`, `pair-b/`, and `summary.json` paths below that run directory.
4. Build and validate the fixed two-entry admission manifest before spawning:
   `[pair-a, pair-b]`, exactly once each, no caller-supplied additions or
   substitutions. A malformed or duplicate manifest fails before either PID or
   Realm login exists.
5. Spawn both direct children, record only PID, profile token, sanitized
   Character identity, and sidecar paths. The PIDs must differ and post-ready
   sidecars must match the pre-spawn manifest; this is an integrity check, not
   duplicate-profile admission.

The orchestrator owns the pair child processes and their redacted output files;
the client owns its session, cipher, worker, and per-client semantic sidecar.
Docker, fixture secrets, and reset-provisioned data remain Reference Realm
resources owned through the canonical lock, never by an individual client.

## Sidecar coordination protocol

Each client writes an atomic, sanitized sidecar in its assigned directory. The
orchestrator polls bounded completion files; it never scrapes interactive
window text, logs, packet bytes, database positions, or process output.

For each live child, the parent also owns one atomic command file under its
assigned run directory: `pair-a/command.json` or `pair-b/command.json`. The
file contains only `{revision, command}` where `command` is one of `idle`,
`perform-role-turn`, or `request-clean-shutdown`. A client accepts exactly the
next increasing revision from its own assigned directory, writes the same
revision and terminal command result to its sidecar, then removes no evidence.
The parent writes through a temporary sibling plus rename, waits for the
matching sidecar acknowledgement, and never writes a second command until the
first is terminal. Directory ownership and exact profile admission bind the
channel to the run; it is coordination metadata, not a credential or a remote
control API. Cleanup removes only the whole owned run directory after children
are reaped and evidence has been retained.

Required sidecar facts are:

| Phase | Required pair facts |
| --- | --- |
| Admission | exact profile token, sanitized Character identity, non-zero Realm GUID, canonical loopback Realm identity |
| Ready | independent `MovementReady`, Entry Anchor map, and no failure state |
| Observer lifecycle | Remote Avatar `Created`, raw Realm-observed pose updates, final stop, and `Removed`/`Faulted` semantic outcome |
| Controlled mover | accepted bounded role intent, stopped Submitted Pose, clean shutdown request/result |
| Completion | sidecar schema/version, terminal phase, redacted failure category when failed |

Malformed, missing, duplicate, secret-bearing, profile-mismatched, or
unexpectedly changing sidecars fail the run. A sidecar proves client-observed
semantic state only; it cannot replace the later capture or Realm evidence
contract.

## Serial role-turn state machine

```text
AcquireLock -> ResetAndHealth -> StartPair -> AwaitBothReady
  -> A-moves / B-observes -> await A stop + B final observed stop
  -> B-moves / A-observes -> await B stop + A final observed stop
  -> RequestCleanShutdown -> AwaitPairSettlement -> FinalResetAndHealth
  -> Success
```

Only one role receives a move command in either turn. The observer must remain
ready for the whole peer turn. Ticket 09 supplies calibrated time and pose
limits once Fixture Pair measurement is valid; until then this state machine
has no accepted replication-success timeout.

`perform-role-turn` is therefore issued first to A while B remains `idle`; only
after A's matching terminal acknowledgement and B's observer sidecar records
the required lifecycle boundary may the parent issue the next revision to B.
An out-of-order, stale, duplicate, or wrong-directory command/acknowledgement
is a malformed-sidecar failure, never an implicit retry.

## Liveness timeout matrix

These are orchestration liveness limits, not Remote Avatar replication-success
limits. A measurement-driven role-pose deadline remains deferred to Ticket 09.

| Phase | Hard deadline | Timeout action and retained evidence |
| --- | --- | --- |
| Lock acquisition | none | atomic contention exits `75` immediately |
| Scoped reset plus initial/final health | existing `realm up` 900 s health deadline | fail, retain phase/health category, then one same-owner recovery |
| Each child `MovementReady` sidecar | 45 s | stop both children; retain ready-phase and sanitized sidecars |
| Command acknowledgement | 5 s | stop both children; retain command revision/profile and last sidecar state |
| Turn terminal acknowledgement | 60 s | stop both children; retain role, command revision, and semantic terminal state; this is not a pose-success claim |
| Clean shutdown acknowledgement | 30 s | reap children and recover; retain shutdown phase |
| Child reap after TERM | 10 s, then one KILL/reap attempt | retain unreaped-PID category and recover; never release lock before a terminal reap result |
| Pair offline settlement | 60 s shared deadline | recover and retain settlement phase; no child-exit inference |

No phase waits indefinitely. Any timeout follows the failure and cleanup
contract below and cannot cause an automatic rerun.

## Failure and cleanup contract

Any child exit, duplicate profile, readiness timeout, malformed sidecar,
semantic failure, or unexpected Realm health result immediately stops role
coordination. The parent sends bounded clean-stop requests where the child is
responsive, reaps both tracked PIDs, and performs exactly one same-owner
`reset-state --yes` plus health recovery attempt.

- Recovery success: retain the redacted failed run summary, release the lock,
  and exit non-zero. No retry is started.
- Recovery failure: retain the run summary and a non-secret lock failure
  marker; do not remove the lock. The next operation fails closed until a human
  verifies the holder/diagnostics, restores Realm health, and removes that
  exact lock directory.
- `EXIT`, `INT`, and `TERM` use the same cleanup path. Normal success releases
  the lock only after final reset and health.

`summary.json` records phase transitions, canonical endpoints, PIDs, profile
tokens, sidecar validation, child/cleanup outcomes, and final health. It omits
credentials, account values, secret paths, session material, raw frames,
payloads, unrestricted logs, and database rows.

## Explicit deferrals

- The Fixture Pair reset/provisioning implementation, Pair A/B client profile
  implementation, exact sidecar schema, calibrated replication deadlines, and
  screenshot composition.
- Automatic retries, peer crash/reconnect recovery, parallel Realm runs,
  cross-host/LAN transport, Windows runtime acceptance, and general process
  management.

## Verification required by later implementation

1. Script tests prove contention returns `75` without Docker/client effects,
   and normal success releases the lock only after final health.
2. Fake children/sidecars prove direct distinct PIDs, exact one-per-profile
   admission, serial role turns, malformed-sidecar failure, and no rerun.
3. Failure injection proves both child reap and one recovery attempt; failed
   recovery retains the lock/failure marker and blocks a next run.
4. A reset-scoped live proof proves only canonical loopback endpoints and
   redacted per-client evidence are retained.
