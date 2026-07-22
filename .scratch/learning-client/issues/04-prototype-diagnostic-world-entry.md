# Prototype the diagnostic World-entry experience

Type: wayfinder:prototype
Status: resolved

## Question

What rough Bevy 0.19 prototype—using only project-created primitives, a small chase/orbit camera, basic built-in UI, and scripted engine-free client events—makes authentication state, world entry, map identity, controlled-character movement, local prediction, and server correction understandable; proves the narrow code-first shell on Apple Silicon plus a Windows compile/headless `App` path; and gives the manual 3D acceptance scenario an unambiguous success condition?

## Scope note

The first throwaway experience artifact is browser-hosted because this planning repository has no client shell and the current machine has no Rust/Cargo toolchain. It can settle presentation, interaction, terminology, and manual acceptance semantics, but it cannot honestly prove Bevy or platform viability. That proof is split into [Prove the Bevy shell and platform test path](11-prove-bevy-shell-platform-path.md).

## Prototype feedback

- Chosen base: **A — viewport-first cockpit**. Keep the project-owned 3D scene primary, with an always-visible session ladder, identity/pose inspector, short semantic event tail, and explicit acceptance state.
- Correction presentation: show the target and correction vector immediately in magenta, interpolate the rendered placeholder over approximately 300 ms, and retain source/delta telemetry. Snap on map changes or corrections greater than 5 m. A correction does not itself move the realm-observed marker unless its authoritative event explicitly supplies that observation.
- Diagnostic detail: show semantic engine-free `ClientEvent`s in the primary event tail. Exact packet/opcode information is expandable provenance on the event; raw packet dumps do not occupy the primary Diagnostic World view.
- Entry initiation: expose one explicit **Connect & Enter Reference Realm** command backed by configured credentials, realm, and character. Authentication, realm selection, character enumeration, and player login then advance automatically through the visible ladder; this slice has no login form, realm picker, or character screen.
- Movement and camera: use camera-relative planar `WASD`, right-mouse drag to orbit, wheel zoom, and a continuously following chase camera. Defer jump, sprint, physics, collision, and animation.
- Manual success condition: enable **Verify persisted movement** only after a stopped pose at least 2 m from the entry anchor. Perform a saving logout and reconnect; pass only when the next `SMSG_LOGIN_VERIFY_WORLD` reports the same map and a position within 0.25 m of the last submitted stop pose. Always show expected and observed values on pass or failure.
- Pose visualization: cyan identifies the moving Rendered Pose and its local-prediction trail; a hollow cyan marker identifies the latest Submitted Pose; amber identifies the Realm-observed Pose and remains at the Entry Anchor during ordinary unacknowledged movement; magenta identifies an active correction vector and Correction Target. Successful reconnect advances amber to the persisted pose and turns acceptance green.

## Answer

Use the [viewport-first cockpit prototype](../prototypes/diagnostic-world-entry/README.md) as the Diagnostic World experience contract. One configured **Connect & Enter Reference Realm** action automatically advances through an always-visible session ladder while the project-owned grid, chase/orbit camera, controlled placeholder, identity/pose inspector, and short semantic event tail remain on screen.

The cockpit must preserve four visually distinct truths: cyan Rendered Pose plus prediction trail, hollow-cyan Submitted Pose, amber Realm-observed Pose, and magenta Correction Target/vector. Ordinary movement does not move the amber marker or imply an ACK. Corrections interpolate for approximately 300 ms, snapping only on a map change or a delta greater than 5 m, and expose their source and delta. Primary diagnostics are engine-free semantic `ClientEvent`s with expandable opcode provenance rather than a raw packet stream.

The manual success path is explicit: enter the world, move and stop at least 2 m from the Entry Anchor, invoke **Verify persisted movement**, complete a saving logout and reconnect, and pass only if the next `SMSG_LOGIN_VERIFY_WORLD` reports the same map within 0.25 m of the last Submitted Pose. The result displays expected and observed values whether it passes or fails.

The linked artifact is deliberately browser-hosted and throwaway; it settles experience semantics, not Bevy/platform feasibility. Real Apple Silicon rendering plus Windows compile/headless `App` proof belongs to [Prove the Bevy shell and platform test path](11-prove-bevy-shell-platform-path.md).
