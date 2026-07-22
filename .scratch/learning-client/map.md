# Chart the AzerothCore-backed Learning Client

Type: wayfinder:map
Status: resolved

## Destination

Reach a decision-complete product and architecture specification for a Learning Client whose World-entry Slice authenticates with a real local Reference Realm, enters a pre-provisioned character, renders a Diagnostic World, and performs server-recognized movement.

The route is clear when the engine boundary, protocol/session path, local realm environment, diagnostic experience, verification contract, and implementation slices are settled well enough to begin implementation planning without material product or architecture questions.

## Notes

- This map is planning-only. It resolves decisions and produces linked research or prototype assets; it does not implement the Learning Client.
- Every session should consult `CONTEXT.md` and use `/grilling` plus `/domain-modeling` for human decisions.
- The Reference Realm uses AzerothCore's Docker infrastructure from this repository, but AzerothCore source code is neither vendored nor forked here.
- The client uses an existing open-source engine. The protocol/session core remains independent from the engine-facing presentation layer.
- The World-entry Slice targets macOS for development and acceptance. Choices must preserve a credible Windows path before a later multiplayer milestone.
- The World-entry Slice uses only project-owned placeholder content and does not require data from an installed Blizzard client.
- The local environment may pre-provision one test account and character; account creation, character management, and multi-realm selection are not initial client features.
- Protocol research may use AzerothCore source, documentation, and compatible open-source client implementations with provenance and license constraints recorded. Original-client packet captures are not assumed available.
- Verification must include deterministic codec/crypto tests, a real Reference Realm integration path through accepted movement, and a manual 3D acceptance scenario.
- The project is for private learning use.

## Decisions so far

<!-- Resolved child tickets are linked here, one gist per ticket. -->

- [Trace the minimal AzerothCore world-entry protocol](issues/01-trace-world-entry-protocol.md) — Use separate build-12340 login and world sessions, answer time/no-flight synchronization, and prove accepted ground movement through saved position on reconnect because AzerothCore sends no ordinary movement ACK to the mover; disable Warden on the Reference Realm.
- [Define the reproducible Reference Realm environment](issues/02-define-reference-realm-environment.md) — Run an original six-service Compose stack pinned to one AzerothCore source build and exact image digests; verify server data separately, provision an account plus player-dump fixture through the authoritative worldserver, publish only local auth/world ports, and use layered health with label-scoped resets.
- [Choose the Learning Client engine direction](issues/03-choose-engine-direction.md) — Use exactly pinned Bevy 0.19.0 and Rust 1.97.1 with engine-free protocol/session crates beneath a thin Bevy-only adapter; rely on primitives and basic UI, omit plugin-heavy gameplay systems, and treat Godot/C# as the authored-content fallback.
- [Prototype the diagnostic World-entry experience](issues/04-prototype-diagnostic-world-entry.md) — Use a viewport-first cockpit with automatic configured entry, always-visible phase/pose diagnostics, cyan Rendered/Submitted state, amber Realm-observed state, magenta correction reconciliation, and a same-map reconnect proof within 0.25 m after a movement of at least 2 m.
- [Prove the Reference Realm bootstrap](issues/05-prove-reference-realm-bootstrap.md) — The locked six-service realm passes on Apple Silicon with Apple Virtualization Framework plus Rosetta, verified server data, idempotent account/player-dump fixtures, semantic health, real build-12340 auth/world smoke, persistent restart, and label-scoped state/full resets; QEMU alone crashes the pinned worldserver.
- [Define the minimum authoritative self-state boundary](issues/10-define-minimum-authoritative-self-state-boundary.md) — Bounded-decode the compressed self `CreateObject2`, skip update values generically, derive run speed from its living block, ACK later run-speed changes, and isolate AzerothCore's correctly typed movement layout behind a project-owned codec.
- [Decide the minimal networked movement contract](issues/06-decide-networked-movement-contract.md) — Predict heading-aligned planar movement at 60 Hz inside a five-metre envelope, submit start/10 Hz heartbeat/stop frames without inventing ACKs, halt on transmission failure, and prove realm recognition only by a saving reconnect within 0.25 m.
- [Prove the Bevy shell and platform test path](issues/11-prove-bevy-shell-platform-path.md) — Rust 1.97.1 plus Bevy 0.19.0 `3d`/`ui` passes a real Apple Silicon Metal primitive-shell proof, engine-free `MinimalPlugins` tests, and a macOS-to-MSVC all-target compile check with a documented BLAKE3/Clang workaround; real Windows build/test/render evidence remains a later gate.
- [Design the engine-independent Learning Client architecture](issues/07-design-client-architecture.md) — Use project-owned protocol types beneath a dedicated deterministic session thread with bounded semantic queues and private test ports, while a thin ordered Bevy plugin group owns only input, interpolation, placeholder presentation, camera, and redacted diagnostics.
- [Specify the World-entry verification contract](issues/08-specify-verification-contract.md) — Require one clean candidate to pass deterministic protocol/session matrices, automated Bevy/platform checks, an isolated real-realm Movement Proof, and manual Metal interaction, then retain one hashed and redacted evidence bundle without hidden retries.
- [Define the implementation slices and scope gates](issues/09-define-implementation-slices.md) — Build eight cumulative capability slices from an offline production scaffold through real entry, movement, Movement Proof, and four-gate acceptance, admitting only requirements that block the active exit gate and branching Windows/gameplay/multiplayer afterward.

## Not yet specified

None.

## Implementation progress

- [Render the Offline Diagnostic World from the Production Scaffold](issues/12-render-offline-diagnostic-world.md) — The first production slice establishes the exact four-crate Rust/Bevy workspace, immutable and secret-safe offline session boundary, viewport-first Diagnostic World, routine native/Windows gates, and a hashed Metal proof without making a network claim.

## Out of scope

- Multiplayer behavior, a second simultaneous client, and Windows acceptance; these belong to a later milestone, although this map must avoid blocking that path.
- Reproducing Azeroth terrain, art, models, audio, interface, or other Blizzard client content.
- Combat, quests, NPC interaction, inventory, chat, social systems, and broad WoW-client feature parity.
- Account creation, character creation or management, and multi-realm user experience.
- Vendoring or maintaining a fork of AzerothCore source code.
- Public distribution or release-readiness work.
