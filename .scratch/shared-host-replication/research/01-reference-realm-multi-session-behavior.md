# Reference Realm multi-session behavior

Ticket: [01 – Measure Reference Realm multi-session behavior](../issues/01-measure-reference-realm-multi-session-behavior.md)
Measured: 2026-07-29
Scope: local macOS host, loopback `127.0.0.1`, local Docker Reference Realm, project-owned fixtures only

## Method

`scripts/measure-reference-realm-multi-session.sh` began with a scoped Realm
reset, provisioned a temporary second account and Character from the
project-owned fixture dump, then ran two independently authenticated retained
Learning Client sessions. The script waited for both sessions to become
`MovementReady`, recorded non-sensitive identity/pose facts, requested clean
shutdown of both sessions, measured Realm-side `online` settlement, and then
performed a separate duplicate-login observation.

The successful redacted machine record is retained locally under
`artifacts/multi-session-research/<run-id>/measurement.json`. It contains no
credentials or authenticated payloads. Failed runs retain only
allowlisted, sanitized failure/status lines and do not reset the Realm. The
temporary account is disposable measurement setup; it is not a durable Fixture
Pair profile, naming scheme, or reset design decision. Successful runs restore
the standard single-fixture Realm only after retaining their evidence.

## Observed facts

- Two separately authenticated sessions reached `MovementReady` concurrently.
  The selected Characters were distinct Realm GUIDs: `Miaztest` = `1` and
  `Miazpeer` = `2`.
- Both Entry Anchors were on map `0` at the same recorded coordinates
  (`-8949.950`, `-132.493`, `83.531`), yielding a measured horizontal distance
  of `0.000 m`. The Fixture Pair can therefore start in deterministic same-map
  proximity without a LAN or advertised-address change.
- The database observed both Characters online while both sessions were ready.
- Immediately after the primary client completed its local clean disconnect,
  the Realm database still reported both `online=1`. This proves the `online`
  flag is an asynchronous persistence observation, not a socket-liveness
  acknowledgement.
- After both local clean disconnects, the Realm eventually reported both
  Characters offline. From the respective local disconnect observations, the
  recorded settlements were 61 s for the primary and 60 s for the peer. A
  later proof must use an explicit settlement phase rather than infer logout
  completion from a client process exit.
- In the duplicate-fixture observation, the first session reached `failed`
  after the second session for the same fixture was started; the second reached
  `movement-ready`. The safe operational rule is therefore one active session
  per Fixture Profile. This result does not identify a Realm replacement,
  kick, or other wire-level mechanism for the first failure.

## Not measured here

- No authenticated World-Update traffic was stored or decoded. This run proves
  concurrent Realm admission, distinct identities, and proximity; it does not
  prove that either client received a remote-player create, pose, or destroy
  record. This negative result is intentional: retaining or decoding that
  traffic is the separately scoped evidence question of Ticket 03.
- The identical initial positions establish co-location, not an independently
  rendered remote-avatar visibility claim. Presentation evidence remains
  deferred to Tickets 03, 04, and 07.
- The measured 60–61 s figures are one local Realm observation, not a general
  network-latency budget or a remote-host guarantee.

## Decisions unlocked

- [Ticket 02 – Fixture Pair provisioning](../issues/02-design-fixture-pair-provisioning.md)
  must use separate accounts and Characters, preserve same-map proximity, and
  prevent duplicate profile launches.
- [Ticket 03 – Remote-player World-Update boundary](../issues/03-trace-remote-player-world-update-boundary.md)
  must obtain the missing remote create/update/destroy evidence without using
  this database measurement as a substitute.
- [Ticket 06 – Loopback topology](../issues/06-map-loopback-multi-client-topology.md)
  must model a shared, bounded logout-settlement phase and retain diagnostics
  without destructive failure cleanup.
