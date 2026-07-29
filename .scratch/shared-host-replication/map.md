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

## Not yet specified

- Exact remote World-Update layout and timing behaviour must be measured from
  the local Reference Realm before their implementation contracts can be
  locked. The Fixture Pair's same-map initial proximity and logout-settlement
  requirement are now measured; durable account/profile and reset ownership
  remain Ticket 02 decisions.
- The implementation slices, evidence schema, and Windows/LAN follow-up
  boundaries will be refined only after the earlier protocol and environment
  decisions are resolved.

## Out of scope

- Native Windows build, test, render, packaging, and runtime acceptance.
- LAN, remote-host, router, firewall, or public-service exposure.
- More than two accepted clients, simultaneous movement, arbitrary object/NPC
  tracking, peer crash recovery, remote reconnect, or general multiplayer
  session management.
- Chat, groups, combat, inventory, quests, social systems, collision, terrain,
  authored content, Blizzard assets, or a World of Warcraft client replacement.
