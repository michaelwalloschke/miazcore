# Miazcore

Miazcore is a learning project for exploring game-client architecture and networked world interaction against AzerothCore.

## Language

**Learning Client**:
The user-facing game application whose initial purpose is entering and navigating an AzerothCore-backed world. Multiplayer interaction is not an initial success condition.
_Avoid_: WoW client replacement, multiplayer client

**World-entry Slice**:
The first end-to-end learning outcome against a real, locally controlled AzerothCore realm: enter a world, render a minimal placeholder environment, and move the controlled character in a way the realm recognizes.
_Avoid_: multiplayer slice, full client

**Reference Realm**:
The locally controlled AzerothCore instance that acts as the Learning Client's compatibility target. Its source code is an external dependency and is not owned by this repository.
_Avoid_: embedded server, server fork

**Diagnostic World**:
The project-owned 3D representation used to make realm identity, character position, movement, and server corrections visible without reproducing Azeroth's terrain or art.
_Avoid_: Azeroth recreation, game world

**Entry Anchor**:
The Realm-observed Pose received when the controlled character enters the world, used as the baseline for meaningful movement and reconnect verification.
_Avoid_: spawn point, local origin

**Realm-observed Pose**:
The latest controlled-character pose supplied by explicit realm evidence such as `SMSG_LOGIN_VERIFY_WORLD`. It does not advance merely because the Learning Client submitted movement.
_Avoid_: server-confirmed pose, acknowledged pose

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
