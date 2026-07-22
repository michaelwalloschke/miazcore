# Slice 12 exit-gate evidence

Recorded: `2026-07-22T15:22:31Z`

Exact tested candidate: `21d197bcc6e6aaa849bd3b11609b3d14fd6ac952`

The candidate was checked out detached in an isolated worktree. Git status was clean before the gates and remained clean after them. Ignored `0600` synthetic credential files were generated locally for startup validation; no existing Reference Realm secret was copied into the candidate checkout.

## Routine code and platform gate

`scripts/check.sh` passed on the exact candidate with:

- Rust `1.97.1 (8bab26f4f 2026-07-14)` and Cargo `1.97.1 (c980f4866 2026-06-30)`;
- formatting and locked native workspace/all-target compilation;
- Clippy across the workspace and all targets with warnings denied;
- 26 native tests: 6 `client_bevy` unit tests, 2 `MinimalPlugins` adapter tests, 1 `client_protocol` test, 14 `client_session` unit tests, 1 offline-session integration test, and 2 composition-binary tests;
- the exact four-package, one-way dependency graph and exact direct dependency pins; and
- `x86_64-pc-windows-msvc` workspace/all-target compilation with the narrow BLAKE3/Clang workaround.

The Windows result remains a compile tripwire only. It is not Windows linking, testing, runtime, or rendering evidence.

## Metal render gate

`scripts/render-smoke.sh <temporary-output-directory>` passed on macOS `26.5.2` / `arm64` using the `Apple M1 Max` adapter and Metal backend.

- Screenshot: PNG, `2560x1440`, SHA-256 `27c8705ad4ef1ceedd64d472caff30b0f35d7534569be7d3d1998bc9e20a9f6d`.
- Semantic sidecar: SHA-256 `63f460f5314c1392876aa4346b56b0da172f1ec702a5cb3a508b5aa60446ddf4`.
- Renderer log: SHA-256 `e85b2f18d59cd294c2d15da07acc20f3c60743313135dd654f14285d76bfbe46`.
- Visual inspection confirmed the viewport-first cockpit, project-owned grid and placeholder, offline display guide, session ladder, identity/build data, Rendered display pose, unavailable Submitted and Realm-observed poses, semantic event tail, bounded-queue counters, controls, and explicit `NO SOCKETS / NO PACKETS` claim.
- The established macOS debug-link `__eh_frame` warning and post-capture winit destroyed-window warning remained non-fatal and matched the disposable proof's known warnings.

Routine render artifacts were intentionally ephemeral; only their redacted semantic description and hashes are retained.

## Review and supporting checks

- The required two-axis code review rechecked the corrected candidate and reported no remaining Standards or Spec finding.
- Local Markdown links, shell syntax, executable modes, dependency boundaries, and diff whitespace passed.
- The candidate diff was scanned against the local Reference Realm password files without exposing their values; no local password material was present.
- [Entry-gate evidence](slice-12-entry-gate.md) records the passing Reference Realm health/smoke and disposable Bevy native, Metal, and Windows checks completed before implementation.

## Remaining deferrals

- Every login or world codec, socket, real session, live realm entry, prediction step, movement frame, and Movement Proof.
- Any claim that the production Learning Client contacted AzerothCore. It did not; the Reference Realm entry gate and production offline application were separate paths.
- Windows linking, tests, runtime behavior, and rendering.
- Gameplay, content, multiplayer, LAN exposure, and wider platform scope.
- Deletion of the disposable Bevy prototype. Its code and durable research evidence remain available until equivalent later production evidence makes deletion appropriate.

This evidence-only record names the tested clean candidate and does not change or invalidate it.
