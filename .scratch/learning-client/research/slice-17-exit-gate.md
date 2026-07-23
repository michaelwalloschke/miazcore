# Slice 17 exit-gate evidence

Recorded: `2026-07-23`

Exact passing candidate: `db04dd47eddc903bed8e846a789de3e29c3aed9a`.
Exact predecessor: `7cb6e4589956f75efdac07026196e06ba43696c2` (Slice 16).

## Delivered boundary

The retained authenticated World session accepts only bounded on-ground intent.
It serializes a PackedGuid-prefixed `AcoreMovementInfo` for immediate start and
stop plus 10 Hz heartbeats, updates Submitted Pose only after `write_all`, and
keeps Realm-observed Pose separate from client-written truth. Prediction is
heading-aligned at 60 Hz, keeps Entry Anchor height, and remains inside the
five-metre envelope.

Inbound retained frames are incrementally decoded without advancing header
cipher state on fragmented data. Later time-sync requests are answered, EOF,
transport failure, and unsupported self-control fail through the redacted
recovery boundary. Scripted corrections are explicitly non-realm evidence:
same-map deltas below five metres smooth the Rendered Pose, while a delta at
the boundary or a map change snaps it.

## Deterministic and platform gates

`./scripts/check.sh` passed on the candidate: formatting, locked workspace
all-target compilation, Clippy with warnings denied, native tests,
dependency-boundary validation, redaction coverage, and the
`x86_64-pc-windows-msvc` compile tripwire. The Windows result is compile-only;
it is not Windows runtime or rendering acceptance.

The retained-loop fake-clock regression covers a 100 ms step followed by a
900 ms catch-up: the latter spans nine heartbeat slots but submits exactly one
latest heartbeat. It also covers heading replacement, focus-loss stop intent,
the five-metre envelope, EOF-to-recoverable failure, and partial-write errors.

`./scripts/render-smoke.sh` passed with a non-black Metal compositor capture:

- offline PNG SHA-256
  `11eaf6af0d245d19fd4348d3f524e87aa99a46c088ddeb0efb6ba0364afb483c`;
- offline sidecar SHA-256
  `63f460f5314c1392876aa4346b56b0da172f1ec702a5cb3a508b5aa60446ddf4`.

The reset-scoped `./scripts/live-diagnostic-world.sh` passed with
`movement_publication: "disabled"`:

- live-entry PNG SHA-256
  `4eee44f83110484d0c9e15ae4e453d63ec016cb39bcb03b6759f93dfd908ba2c`;
- live-entry sidecar SHA-256
  `0344701c4b970336fcfd27de72530b00a2c72791025b46d07c03e71276e0d085`.

The independently reset-scoped `./scripts/live-movement-smoke.sh` passed with
a non-black Metal capture, bounded movement in both scripted directions,
distinct Submitted versus Realm-observed poses, and post-stop realm health:

- movement PNG SHA-256
  `0f31b7a21b2000ef7656676bab3a8a445bf17a4ad308733c5c81dc2c803ddd87`;
- movement sidecar SHA-256
  `09fda8efc6d25b48d1168b4dee41a72ceadfbfcd174d045770ab98977fbb879d`.

Final independent spec review returned `PASS` for heartbeat coalescing and
EOF lifecycle evidence. The standards review found no documented violation
after the explicit queued-action test harness replaced poll-count coupling.

## Remaining deferrals

- Saving logout and reconnect comparison, and any claim that the realm
  accepted, acknowledged, or persisted movement.
- Realm-wire decoding of post-entry self-update corrections; Slice 17 uses
  only the documented internal scripted boundary.
- Terrain, collision, broader movement/control states, gameplay, multiplayer,
  and Windows runtime/render acceptance.
