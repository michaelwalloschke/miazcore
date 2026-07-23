# Slice 16 exit-gate evidence

Recorded: `2026-07-23`

Exact passing candidate: `7cb6e4589956f75efdac07026196e06ba43696c2`.
Exact predecessor: `aae357b57520ff0e40ef3214948279b4112aabb7`.

## Delivered boundary

The Bevy Diagnostic World now drives exactly one complete `StartEntry` through
the live session boundary and projects its sanitized `MovementReady` truth.
It exposes the Entry Anchor, realm-provided run speed, Rendered Pose, Submitted
Pose, Realm-observed Pose, counters, and bounded semantic history while movement
publication stays disabled. Failure retains explicit retry guidance and gates
input; the worker disconnects and joins on application exit.

macOS Metal proof capture uses the repository-owned compositor adapter rather
than the unreliable Bevy/WGPU readback. It waits for a settled proof state,
finds only the `learning_client` window by PID and exact title using
CoreGraphics, captures that window with `screencapture -l`, rejects missing,
black, undersized, or ambiguous captures, and then permits the app to exit.

## Gates

`./scripts/check.sh` passed on the candidate: formatting, locked native
all-target compilation, Clippy with warnings denied, 61 native tests,
dependency-boundary validation, redaction coverage, and the Windows MSVC
compile tripwire.

`./scripts/render-smoke.sh` passed with an actual Metal compositor capture:

- `2560x1504` offline Diagnostic World PNG SHA-256
  `cd1d9ecb63e7a63debe013866396bbcd85914c79febd2dd3caebeb882c3c5e50`;
- offline semantic sidecar SHA-256
  `63f460f5314c1392876aa4346b56b0da172f1ec702a5cb3a508b5aa60446ddf4`.

The reset-scoped `./scripts/live-diagnostic-world.sh` passed against the fresh
Reference Realm. Its Metal compositor capture reached `MovementReady` with
`Miaztest`, map `0`, Entry Anchor `(-8949.950, -132.493, 83.531)`, run speed
`7.000`, three equal entry pose truths, and disabled movement publication:

- live PNG SHA-256
  `8e78740fac2265d5a0429611f3d93995d4a74dae7e516f2d5a5e060d921ee0a9`;
- live semantic sidecar SHA-256
  `71372133e01359651418e179befb0eb35832e0a02a1c53fa497247c9187ddc42`.

Both final independent reviews returned `PASS`: standards found no documented
breaches or Fowler blockers, and specification review found no remaining plan
or ticket-16 blocker.

## Remaining deferrals

- Movement intent, prediction, start/heartbeat/stop frames, and Submitted Pose
  advancement.
- Live pose divergence/correction behavior and Movement Proof.
- Windows runtime/rendering acceptance; the Windows gate is compile-only.
