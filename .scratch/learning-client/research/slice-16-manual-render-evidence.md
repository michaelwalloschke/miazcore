# Slice 16 manual live-render evidence

Recorded: `2026-07-23`

Screenshot supplied by the operator after launching the normal Bevy client,
selecting **Connect & Enter Reference Realm**, and waiting for the visible
Diagnostic World to reach `MovementReady`:

- artifact: `artifacts/live-diagnostic-world/manual-live-diagnostic-world.png`;
- dimensions: `1270x715` pixels;
- SHA-256:
  `d7e877f83ffce678c8e2eff9180c0ff8656ec6a0f40a20c2577eb6931551b45d`.

The visible client confirms the intended manual acceptance surface:

- `REFERENCE REALM / DIAGNOSTIC WORLD` and character `Miaztest`;
- the complete session ladder through `MOVEMENT READY`;
- Entry Anchor, Rendered Pose, Submitted Pose, and Realm-observed Pose all at
  `map 0, -8949.95, -132.49, 83.53` at entry;
- realm-provided run speed `7.000 m/s`;
- `control 0/16`, `events 0/64`, and `intent revision 0`;
- explicit `MOVEMENT PUBLICATION DISABLED` and the no-movement acceptance
  message.

This is visual evidence only. The programmatic Metal screenshot path still
occasionally produces an all-black PNG although the window itself renders
correctly. The capture path now waits three real seconds after the presentation
is armed and refuses to enqueue a second screenshot, but the automated artifact
gate remains open until it produces a non-black screenshot deterministically.
