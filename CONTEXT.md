# Miazcore

Miazcore is a learning project for exploring game-client architecture and networked world interaction against AzerothCore.

## Language

**Learning Client**:
The user-facing game application whose initial purpose is entering and navigating an AzerothCore-backed world. Multiplayer interaction is not an initial success condition.
_Avoid_: WoW client replacement, multiplayer client

**World-entry Slice**:
The first end-to-end learning outcome against a real, locally controlled AzerothCore realm: enter a world, render a minimal placeholder environment, and move the controlled character in a way the realm recognizes.
_Avoid_: multiplayer slice, full client

**World-entry Acceptance**:
The aggregate decision that the World-entry Slice has passed its deterministic core, Bevy/platform, live Reference Realm, and manual macOS gates. No individual gate or evidence artifact substitutes for the other three.
_Avoid_: end-to-end test, smoke test, demo success

**Acceptance Evidence Bundle**:
The curated, redacted record tying World-entry Acceptance to one exact clean Git commit and its four verification gates. It contains reproducible summaries and selected artifacts, not raw authenticated traffic or unrestricted runtime logs.
_Avoid_: log dump, CI artifact, screenshot proof

**Reference Realm**:
The locally controlled AzerothCore instance that acts as the Learning Client's compatibility target. Its source code is an external dependency and is not owned by this repository.
_Avoid_: embedded server, server fork

**Shared-Host Multi-client Simulation**:
A private learning setup in which two macOS Learning Client processes on one host connect independently to the same local Reference Realm. It is a prerequisite environment for later multiplayer work, not Windows acceptance or public LAN exposure.
_Avoid_: Windows support, internet multiplayer, public server

**Fixture Pair**:
Two separately authenticated, reset-scoped Reference Realm test Characters provisioned for the Shared-Host Multi-client Simulation. They may log in concurrently and begin close enough for a deterministic Realm-replication proof.
_Avoid_: duplicate login, ad hoc player account, production identity

**Fixture Pair Start Placement**:
The reset-provisioned, same-map coordinate relation for the two Fixture Pair members. It is accepted only after a local Placement Probe proves the members can reach Movement-ready Session at the declared relation. It is not an Entry Anchor, which is Realm-observed only after login.
_Avoid_: Entry Anchor, local origin, remote-avatar evidence

**Fixture Profile**:
A named, non-sensitive Learning Client launch selection for one Fixture Pair member. It resolves to a separate private credential file and fixed test Character without accepting credentials through command-line arguments or emitting them in evidence.
_Avoid_: account name flag, shared credential, credential-bearing configuration

**Diagnostic World**:
The project-owned 3D representation used to make realm identity, character position, movement, and server corrections visible without reproducing Azeroth's terrain or art.
_Avoid_: Azeroth recreation, game world

**Entry Anchor**:
The Realm-observed Pose received when the controlled character enters the world, used as the baseline for meaningful movement and reconnect verification.
_Avoid_: spawn point, local origin

**Realm-observed Pose**:
The latest controlled-character pose supplied by explicit realm evidence such as `SMSG_LOGIN_VERIFY_WORLD`. It does not advance merely because the Learning Client submitted movement.
_Avoid_: server-confirmed pose, acknowledged pose

**Realm-replicated Avatar**:
A non-controlled character rendered in the Diagnostic World solely from World-Update evidence received from the Reference Realm. Its identity, presence, pose, and lifecycle never derive from local movement prediction or scripted mirroring.
_Avoid_: ghost, locally mirrored player, synthetic peer

**Remote Avatar Marker**:
The project-owned Diagnostic World presentation of a Realm-replicated Avatar: an identity-distinct placeholder, readable name or GUID shorthand, and a visible heading indicator. It does not use character models or Blizzard client assets.
_Avoid_: player model, Azeroth asset, generic decoration

**Remote Avatar Identity**:
The Realm GUID that keys a Realm-replicated Avatar across its lifecycle. A Character name is display-only metadata and cannot select, merge, or destroy the marker.
_Avoid_: name-keyed replication, fixture-name protocol identity, synthetic remote id

**Remote Avatar Lifecycle**:
The minimal Realm-replicated Avatar state sequence: presence after its Fixture Pair member logs in, pose and heading updates while that member moves, and removal after its clean logout.
_Avoid_: teleport support, arbitrary object tracking, combat state

**Remote Avatar Fault Boundary**:
The fail-safe treatment of unusable Remote Avatar data: remove that marker and expose a redacted diagnostic when framing remains valid; fail the entire World session when framing integrity cannot be established.
_Avoid_: guessed remote pose, silent stale marker, recoverable cipher drift

**Remote Pose Projection**:
The distinction for a Realm-replicated Avatar between its latest received Realm-observed Pose and its time-smoothed Rendered Pose. Projection never predicts a remote Character and snaps on map changes or large corrections.
_Avoid_: remote prediction, authoritative rendered pose

**Replicated Move**:
The bounded role-turn used by the Role-reversed Replication Proof: the moving Fixture Pair member performs a two-to-four metre heading-aligned move, and its observer receives an end pose on the same map within 0.25 metres.
_Avoid_: local transform check, inferred remote movement, unbounded traversal

**Submitted Pose**:
The latest controlled-character pose whose complete movement frame was successfully written to the world socket. It proves what was sent, not that the Reference Realm accepted or persisted it.
_Avoid_: authoritative pose, confirmed pose

**Rendered Pose**:
The controlled-character pose currently presented in the Diagnostic World. It may interpolate or reconcile toward Predicted Pose and is not evidence of submission or realm observation.
_Avoid_: actual pose, server pose

**Predicted Pose**:
The engine-independent fixed-step estimate of controlled-character pose advanced from movement intent within realm-provided limits. It is an input to presentation, not a claim of submission or realm observation.
_Avoid_: Rendered Pose, Submitted Pose, client position

**Correction Target**:
A pose supplied by a correction-capable client event toward which the Rendered Pose reconciles while preserving the event's source and delta for diagnosis.
_Avoid_: Realm-observed Pose, teleport destination

**Heading-aligned Movement**:
The World-entry Slice's locomotion mode in which any camera-relative planar input selects a world heading and moves the controlled character forward along it.
_Avoid_: strafing, backward movement, MMO-style movement

**Reference Movement Envelope**:
The five-metre horizontal area around the Entry Anchor within which the World-entry Slice may predict, render, and submit movement while retaining the anchor height.
_Avoid_: collision boundary, terrain boundary, playable world

**Movement-ready Session**:
A world session that has observed world entry and matching self state, obtained a positive run speed, and completed the required time and movement-control synchronization. Only this phase may consume movement intent.
_Avoid_: entered session, connected session, ready client

**Movement Proof**:
The saving-reconnect result that compares a fresh Realm-observed Pose with the final stopped Submitted Pose on the same map.
_Avoid_: movement acknowledgement, packet acceptance, database check

**Role-reversed Replication Proof**:
A Shared-Host Multi-client Simulation acceptance scenario in which each Fixture Pair member moves once while the other client observes its Realm-replicated Avatar. The roles are serial, not simultaneous, and success comes only from the observer's received World-Update evidence.
_Avoid_: local echo, concurrent-input proof, collision test

**Dual-Window Replication Evidence**:
The macOS-only acceptance record for the Shared-Host Multi-client Simulation: per-client semantic sidecars plus a controlled capture of both independently running Diagnostic World windows. It is neither a full-screen capture nor a manual substitute for the automated gate.
_Avoid_: desktop capture, screenshot-only proof, single-process simulation

**Dual-Client Orchestrator**:
A repository-owned process that starts the two Fixture Profiles concurrently, coordinates their readiness and role turns, collects their evidence, and shuts both down cleanly. It is the automated harness, not a replacement for manual exploration.
_Avoid_: manual double launch, test-only fake peer, shared client process
