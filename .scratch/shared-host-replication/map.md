# Chart the Shared-Host Realm Replication Slice

Type: wayfinder:map
Status: open

## Destination

Reach a decision-complete, implementation-ready plan for a private Shared-Host
Multi-client Simulation: two independent macOS Learning Client processes enter
the local Reference Realm and each observes the other's Realm-replicated Avatar
appear, perform a bounded move, and disappear after clean logout.

## Notes

- Consult `CONTEXT.md`, `docs/adr/`, `/grilling`, and `/domain-modeling` in
  every session. Use the established Learning Client vocabulary.
- The Fixture Pair uses separate accounts, Characters, and private credential
  files. Profiles are named, non-sensitive selections; credentials never reach
  CLI arguments, logs, sidecars, or Git.
- The route is macOS-only and loopback-only: `127.0.0.1`, local Docker ports,
  and no LAN binding, advertised-address change, firewall work, or Windows
  runtime acceptance.
- The first proof is role-reversed and serial. Each Character moves two to four
  metres; the observing client must receive an end pose on the same map within
  0.25 m. Movement persistence is reused from World-entry Acceptance, not
  repeated as a nested acceptance requirement.
- Remote data must come only from actual World-Update evidence. Rendered remote
  motion may smooth between received poses but never predicts; it snaps on map
  change or large correction. Remote identity is the Realm GUID, not a name.
- Scope is one Remote Avatar per client in acceptance, although the model is
  GUID-keyed. Required lifecycle is Create/Update/Destroy after clean logout.
  Abrupt peer loss, remote reconnect, NPCs, combat, chat, groups, collision,
  terrain, and arbitrary object replication are deferred.
- Acceptance combines per-client semantic sidecars, a controlled dual-window
  macOS capture, deterministic scripted role turns, and a short manual
  two-window extension. It is not a fullscreen capture or a manual substitute.

## Decisions so far

- [Reference Realm multi-session behavior](research/01-reference-realm-multi-session-behavior.md): two separate loopback sessions can become concurrently `MovementReady` with distinct GUIDs and co-located Entry Anchors; Realm `online` settlement is asynchronous (60–61 s in the measured run), and a duplicate Fixture Profile run ended with the first session `failed` and the second `movement-ready`. World-Update visibility remains unmeasured.
- [Fixture Pair provisioning contract](research/02-fixture-pair-provisioning.md): an isolated new Pair A/B is separate from the existing single-client fixture, `reset-state` owns all three fixtures, and Pair B starts exactly three metres east of Pair A after a local Placement Probe.
- [Remote-player World-Update boundary](research/03-remote-player-world-update-boundary.md): source tracing and the reset-scoped local semantic transcript establish a non-self player `CreateObject2`, peer `MSG_MOVE_HEARTBEAT`/`MSG_MOVE_STOP`, and `SMSG_DESTROY_OBJECT` after controlled logout. Names are a separate query protocol; arbitrary update-field decoding is deferred.
- [Realm-replicated Avatar contract](research/04-realm-replicated-avatar-contract.md): the World-session boundary owns one GUID-keyed Remote Avatar's lifecycle and raw Realm-observed Pose; Bevy alone owns its smoothed Rendered Pose. Remote prediction and a second accepted Avatar are out of scope.
- [Fixture Profile and secret contract](research/05-fixture-profile-and-secret-contract.md): `--fixture-profile pair-a|pair-b` is a closed, non-sensitive CLI selector. The binary privately maps it to a fixed Character and separate ignored `0600` credential files; the existing session loader remains the only credential boundary.
- [Loopback multi-client topology](research/06-loopback-multi-client-topology.md): one canonical loopback-only Docker Realm is guarded by the existing atomic Realm-test lock; Pair A/B are independent child processes, and reset/cleanup ownership fails closed on contention or unrecovered Realm health.
- [Dual-window Diagnostic World experience](research/07-dual-window-diagnostic-experience.md): two independent viewport-first windows make one Local Character and one Remote Avatar Marker unmistakable through separate primitive shapes, GUID shorthand, observed-versus-rendered remote pose rows, lifecycle events, and a redacted fault state.
- [Remote Avatar fault boundary](research/08-remote-avatar-fault-boundary.md): encrypted-frame or unwalkable-container integrity failures fail the whole World session; after intact framing, unusable accepted-avatar data removes only its marker with a redacted diagnostic, while valid out-of-scope data is fully consumed and ignored.
- [Dual-Client Orchestrator](research/10-dual-client-orchestrator.md): one foreground repository parent owns the canonical Realm lock, starts exact Pair A/B child processes, coordinates only serial sidecar-driven role turns, and either completes after final health or retains a visible same-owner recovery failure.
- [Paired Fixture reset task](research/12-paired-fixture-reset-task.md): reviewed Pair Pdump provenance and one lock-held reset establish exactly the three fixtures, while a separate Placement Probe verifies Pair A/B readiness and placement without claiming Remote Avatar replication.
- [Fixture Pair replication timeline and tolerances](research/09-replication-timeline-and-tolerances.md): a shared-clock, three-sample Pair A/B study calibrates the serial observer deadlines (`331 ms` first pose, `508 ms` terminal stop, `19,760 ms` removal), the fixed `0.25 m` terminal comparison, and a `1.628 m` Remote Pose Projection snap boundary from local Realm evidence.
- [Role-reversed Replication Evidence Contract](research/11-replication-evidence-contract.md): an independent, hash-bound pair-evidence bundle requires semantic sidecars and serial GUID/lifecycle/Realm-observed-pose assertions; dual-window capture and the manual extension are mandatory but cannot substitute for them.
- [Minimal remote-player protocol decoder](research/13-minimal-remote-player-protocol-decoder.md): a protocol-owned, stateless semantic decoder receives only complete plaintext World frames, fully consumes bounded update containers, and emits GUID-keyed create/movement/removal records while deferring selection, display metadata, and all general object decoding.
- [Remote Avatar session-event boundary](research/14-remote-avatar-session-event-boundary.md): the retained World worker owns one accepted GUID and publishes lossless Remote Avatar lifecycle/pose transitions via the existing bounded semantic FIFO and latest-state snapshot; saturation fails the World session rather than silently coalescing updates.
- [Remote Avatar presentation boundary](research/15-remote-avatar-presentation-boundary.md): Bevy privately projects one GUID-keyed marker from ordered Remote Avatar events and the snapshot fence, smooths only small same-map observed deltas, snaps at `1.628 m` or map change, and cannot feed visual state back into session truth.
- [Deterministic replication test harness](research/16-deterministic-replication-test-harness.md): a layered, crate-local fake-clock and encrypted-frame fixture vocabulary proves decoder, session, presentation, and observer-only semantic contracts without Docker, a window/GPU renderer, or wall-clock timing claims.

## Not yet specified

- The role-reversed live-proof and manual two-window acceptance contracts are
  still design work. The Fixture Pair's same-map initial proximity, fixed
  placement, reset ownership, local timing boundaries, minimal decoder seam,
  session-event boundary, Bevy presentation boundary, and deterministic test
  harness are decided.
- The implementation slices and Windows/LAN follow-up boundaries will be
  refined only after the earlier protocol and environment decisions are
  resolved.

## Out of scope

- Native Windows build, test, render, packaging, and runtime acceptance.
- LAN, remote-host, router, firewall, or public-service exposure.
- More than two accepted clients, simultaneous movement, arbitrary object/NPC
  tracking, peer crash recovery, remote reconnect, or general multiplayer
  session management.
- Chat, groups, combat, inventory, quests, social systems, collision, terrain,
  authored content, Blizzard assets, or a World of Warcraft client replacement.
