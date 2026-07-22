# Bevy shell and platform path proof

Proof date: 2026-07-22

## Answer in one sentence

Yes: Rust 1.97.1 and Bevy 0.19.0 with exactly `default-features = false`
plus `3d` and `ui` render a project-owned primitive shell through Metal on the
current Apple Silicon machine, run engine-free scripted events through a
headless `MinimalPlugins` app, and compile-check the same crate boundary for
`x86_64-pc-windows-msvc`, provided the non-Windows host selects Clang and
enables BLAKE3's pure-Rust path; the cross-check is not a Windows link, test, or
render proof.

## Disposable proof asset

The isolated prototype is [Bevy shell and platform proof](../prototypes/bevy-shell-platform/README.md).
It is deliberately outside the future client workspace and has an explicit
deletion boundary. Its structure proves the intended dependency direction:

```text
engine-free model/events  <-  thin Bevy resource/schedule adapter  <-  shell
```

`src/model.rs` has no Bevy imports. `src/adapter.rs` owns the ECS resources and
event-draining schedule. `src/main.rs` owns only the window, primitive scene,
input, camera, UI, coordinate conversion, and screenshot proof path.

## Tested baseline

| Item | Observed value |
| --- | --- |
| Host | `aarch64-apple-darwin`, Apple M1 Max, 10 cores, 64 GiB |
| OS | macOS 26.5.2, build `25F84`, Darwin 25.5.0 |
| Rust | `rustc 1.97.1 (8bab26f4f 2026-07-14)`, LLVM 22.1.6 |
| Cargo | `cargo 1.97.1 (c980f4866 2026-06-30)` |
| Bevy | exact `=0.19.0` |
| Native renderer | wgpu adapter `Apple M1 Max`, backend `Metal` |
| Installed targets | `aarch64-apple-darwin`, `x86_64-pc-windows-msvc` |

The prototype commits its own `rust-toolchain.toml` and `Cargo.lock`. The
production client should carry the same locks at the workspace root.

## Exact dependency and feature contract

```toml
[dependencies]
bevy = { version = "=0.19.0", default-features = false, features = ["3d", "ui"] }

[target.'cfg(target_os = "windows")'.dependencies]
blake3 = { version = "=1.8.5", features = ["pure"] }
```

The pinned Bevy manifest confirms that `3d` and `ui` are first-party feature
collections and that they bring in the default application/platform stack,
renderer, PBR, scene, UI renderer/widgets, and built-in font
([pinned feature definitions](https://github.com/bevyengine/bevy/blob/v0.19.0/Cargo.toml)).
No audio, 2D collection, third-party camera/controller, physics, external
asset, or `bevy_ci_testing` feature is required for this boundary.

The `blake3` entry is feature unification for a Bevy transitive dependency, not
a new learning-client API dependency. Without it, the macOS-to-MSVC check
failed in BLAKE3's build script with `failed to find tool "ml64.exe"`. BLAKE3
1.8.5 exposes `pure` and its build script selects Rust intrinsics when that
feature is active; the script still probes compiler support, so the exact Mac
command also selects Clang ([pinned BLAKE3 features](https://github.com/BLAKE3-team/BLAKE3/blob/1.8.5/Cargo.toml),
[pinned build logic](https://github.com/BLAKE3-team/BLAKE3/blob/1.8.5/build.rs)).

## Proof matrix

| Boundary | Command/evidence | Result |
| --- | --- | --- |
| Formatting | `cargo fmt --all -- --check` | Pass |
| Native compile surface | `cargo check --locked --all-targets` | Pass |
| Static quality | `cargo clippy --locked --all-targets -- -D warnings` | Pass |
| Pure model tests | two ordinary Rust unit tests | 2 passed |
| Headless Bevy adapter | `App::new()` + `MinimalPlugins`, scripted engine-free events, one `app.update()` | 1 passed in 0.07 s; no window/GPU initialized |
| Native rendered shell | forced Metal backend, real window, primitive ground/grid/avatar/markers, built-in UI, scripted domain movement, chase/orbit rig, screenshot | Pass |
| Windows target surface | library, binary, unit tests, and integration test compile-checked for `x86_64-pc-windows-msvc` | Pass |

The headless test follows Bevy's own documented `MinimalPlugins` application
test pattern ([official 0.19 test example](https://github.com/bevyengine/bevy/blob/v0.19.0/tests/how_to_test_apps.rs)).
The shell uses the same pinned primitive PBR, UI text, camera, and screenshot
APIs demonstrated by Bevy's 0.19 examples
([primitive scene](https://github.com/bevyengine/bevy/blob/v0.19.0/examples/3d/3d_scene.rs),
[orbit camera](https://github.com/bevyengine/bevy/blob/v0.19.0/examples/camera/camera_orbit.rs),
[screenshot](https://github.com/bevyengine/bevy/blob/v0.19.0/examples/window/screenshot.rs)).

## Exact verification commands

Run from `.scratch/learning-client/prototypes/bevy-shell-platform`:

```sh
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
WGPU_BACKEND=metal RUST_LOG=info \
  cargo run --locked -- --proof-output artifacts/macos-shell.png
CC_x86_64_pc_windows_msvc=clang \
  cargo check --locked --all-targets --target x86_64-pc-windows-msvc
```

The rendered artifact is [macos-shell.png](../prototypes/bevy-shell-platform/artifacts/macos-shell.png):

- PNG, 2560×1440 (the 1280×720 logical window at Retina scale)
- SHA-256 `c48adfc292496f5a1475c8aa52eff9aceed0df83aaf7dc20ffa4e788e43ffa97`
- renderer log: `AdapterInfo ... name: "Apple M1 Max" ... backend: Metal`
- the proof-mode event advances Rendered Pose to `(1.25, 0.75, 0.00)` while
  Submitted and Realm-observed remain at the anchor, visibly exercising the
  adapter and the selected three-pose diagnostic vocabulary

## Platform limitations

1. `cargo check` compiles Rust code and target-specific build dependencies but
   does not link a PE/COFF executable or run tests. It therefore proves that the
   project boundary is target-compilable, not that DirectX/window startup works.
2. Rust classifies `x86_64-pc-windows-msvc` as Tier 1 with host tools, but says
   cross-compilation to MSVC from a non-Windows host may be possible and is not
   supported ([Rust platform support](https://doc.rust-lang.org/rustc/platform-support/windows-msvc.html)).
   The Mac cross-check is an early portability tripwire, never the acceptance gate.
3. A real Windows machine or Windows-hosted CI must run `cargo test --locked`
   and build/link the shell with the Visual Studio C++ tools. When Windows enters
   acceptance scope, it must also run a bounded DirectX window/screenshot smoke.
4. Native debug links emit macOS ld's `__eh_frame section too large` warning for
   Bevy's full debug graph. It did not prevent tests or rendering. The production
   scaffold may choose reduced dependency debug info after measuring it; the
   proof does not hide the warning with an untested profile change.
5. The automated screenshot exits immediately after capture and produces a
   harmless winit `Destroyed for unknown Window Id` cleanup warning. Interactive
   window lifetime remains a separate manual acceptance scenario.
6. Screenshot dimensions and pixels vary with scale factor, GPU, backend, and
   driver. Keep screenshots as diagnostic artifacts, not exact cross-platform
   golden images.

## Contract for architecture and verification tickets

- Preserve the one-way crate boundary: only the outer client crate may depend
  on Bevy; protocol/session/domain types cannot contain Bevy resources, vectors,
  entities, schedules, messages, or tasks.
- Start the implementation workspace with exact Rust/Bevy pins, a committed
  lockfile, the `3d` and `ui` collections only, and the Windows BLAKE3 portability
  feature. Do not infer a larger default feature set from examples.
- Test protocol and session crates as ordinary Rust. Test the adapter with
  `MinimalPlugins` and explicit scripted domain events. Do not initialize a
  window or renderer for semantic tests.
- Keep a native Apple Silicon rendered gate for the primitive world, UI, input,
  chase/orbit camera, and screenshot/log artifact. Force `WGPU_BACKEND=metal` in
  that proof so the backend is evidence rather than assumption.
- Keep the macOS-to-MSVC all-target check as a fast portability tripwire, then
  add Windows-hosted test/build evidence before claiming Windows support and a
  real Windows rendered smoke before Windows becomes an acceptance platform.
- Keep the chase/orbit camera and placeholder locomotion project-owned and
  small. The proof found no reason to add a controller, physics, UI, or testing
  plugin.

## Fallback and reopening triggers

This proof does **not** trigger the Godot fallback. Reopen the engine direction
only if one of these becomes true during implementation:

- the pinned primitive/PBR/UI surface cannot render reliably on the supported
  Apple Silicon baseline or, when tested, the real Windows machine;
- the thin adapter cannot remain thin because required session/protocol behavior
  must move into Bevy schedules or types;
- substantial authored scenes, content workflows, or UI exceed the deliberate
  primitive/basic-UI boundary and the missing editor becomes a measured delivery
  blocker; or
- the exact pinned dependency graph has an unresolved security, build, or
  platform defect with no bounded project-owned workaround.

Long first builds, the documented debug-link warning, Bevy API unfamiliarity,
or an unsupported non-Windows MSVC link are not fallback triggers by themselves.
