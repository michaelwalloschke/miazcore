# Loopback multi-client topology

Ticket: [06 – Map the loopback multi-client topology](../issues/06-map-loopback-multi-client-topology.md)

## Decision

The Shared-Host Multi-client Simulation has one macOS host, one canonical local
Docker Compose project, and two independent Learning Client child processes.
All client-to-Realm traffic stays on host loopback. It creates no LAN listener,
advertised-address override, firewall rule, second Realm, or Windows runtime
claim.

```text
Shared-Host orchestrator (one repository process; owns realm lock)
  ├─ Docker Compose project: miazcore-reference-realm
  │    ├─ authserver  127.0.0.1:3724
  │    ├─ worldserver 127.0.0.1:8085
  │    └─ database and server-data volumes (no host database port)
  ├─ Learning Client A --fixture-profile pair-a
  └─ Learning Client B --fixture-profile pair-b
```

Each client gets an independent process, session worker, login socket, World
socket, cipher state, configuration object, and artifact subdirectory. The
clients share only the Reference Realm endpoints and the repository-owned
orchestrator protocol. They never share a process, account, credential file,
session material, or in-memory Remote Avatar state.

## Fixed local network and Compose ownership

- The only Compose project is `miazcore-reference-realm`, using the checked-in
  `infra/azerothcore/compose.yaml`. The orchestrator does not accept
  `MIAZCORE_COMPOSE_PROJECT` or `MIAZCORE_REALM_ADDRESS` overrides.
- Auth and World remain published exactly as `127.0.0.1:3724` and
  `127.0.0.1:8085`. MySQL remains on the private Compose network. Pair clients
  use the existing loopback-only `ClientConfig` validation.
- The project-labelled database-data and server-data volumes, Compose network,
  ignored secret directory, and both published ports are exclusive shared
  Realm resources. A reset, Worldserver fault injection, fixture provisioning,
  or pair client run must not overlap another owner of any of them.

## Realm lock and run ownership

The canonical repository-wide lock is the existing atomic directory:

```text
.scratch/learning-client/.realm-test.lock
```

Every future pair reset, placement probe, replication measurement, orchestrator,
and live proof acquires it with one `mkdir` before querying, resetting,
provisioning, or starting either client. Lock contention exits with status `75`
and a redacted message naming the exclusive Realm operation; it never waits,
deletes an existing lock, joins another run, or starts a second Compose project.

The lock holder writes a non-secret owner record containing only its script
name, PID, UTC start time, and run directory. Normal `EXIT`, `INT`, and `TERM`
cleanup reaps tracked children and removes the lock only after Realm health.
A stale or recovery-failed lock is preserved with a non-secret failure marker.
The next run fails closed until a human verifies no holder is alive, inspects
the retained diagnostics, restores `realm health`, and removes the exact lock
directory manually. PID existence alone is not permission to remove it.

## Reset, lifecycle, and cleanup sequence

1. Acquire the lock, create a unique ignored run directory, and record owner
   metadata before changing Docker state.
2. Run scoped `realm reset-state --yes`, which establishes the accepted
   three-fixture set: the existing single-client fixture plus Fixture Pair A/B.
   Require health before opening either Pair client.
3. Spawn Pair A/B as direct child processes with their closed Fixture Profile
   selectors. Record their PIDs and profile tokens only in redacted metadata.
4. Coordinate readiness and serial role turns. A child exit, malformed sidecar,
   duplicate profile, or unexpected health result stops the tracked peer and
   fails the run; no in-place retry occurs.
5. On success, request clean shutdown, wait for one bounded pair-specific
   offline settlement phase, restore the same accepted three-fixture Realm
   through `reset-state`, require final health, then release the lock.
6. On failure after Realm mutation, retain the redacted run record and make one
   same-owner scoped recovery attempt: reap children, reset, and require health.
   If this fails, retain the lock marker for human recovery instead of risking
   an overlapping reset.

The observed 60–61 second logout settlement is an initial local observation.
The later pair helper uses one shared bounded deadline after both clean
disconnect observations; it never infers offline state from child exit. Ticket
09 measures the final acceptance tolerance.

## Evidence and failure visibility

- Each run uses `artifacts/shared-host-replication/<utc-run-id>/` with separate
  `pair-a/` and `pair-b/` subdirectories plus an orchestrator summary. It may
  contain profile tokens and sanitized Character identity, never accounts,
  passwords, credential paths, session keys, raw payloads, or unrestricted logs.
- The summary records lock lifecycle, canonical endpoints, client PIDs, phase
  transitions, cleanup, and final health. It never treats a database position
  or `online` field as Remote Avatar proof.
- Reset failure, lock contention, child cleanup failure, or final health failure
  is explicit. No script hides it with a new Compose project, background client,
  unscoped Docker command, or destructive retry.

## Explicit deferrals

- Cross-host, LAN, router, firewall, public-service, Windows runtime, or
  container-network client access.
- Parallel independent Realm test runs, a distributed lock service, a shared
  credential service, Pair provisioning implementation (Ticket 12), and the
  process-admission protocol (Ticket 10).

## Verification required by later implementation

1. Lock tests prove a second operation exits `75` without touching Docker,
   secret files, or either client; normal cleanup releases only after health.
2. Orchestrator tests prove Pair A/B are distinct child PIDs with independent
   Fixture Profiles and no duplicate-profile Realm run.
3. Failure injection proves child cleanup plus one same-owner recovery attempt;
   failed recovery retains the lock marker and blocks the next run.
4. A live proof records only `127.0.0.1:3724`/`8085`, final health, and redacted
   per-profile evidence, with no address or Compose-project override.
