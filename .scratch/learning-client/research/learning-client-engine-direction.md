# Learning Client engine direction

Research date: 2026-07-21

## Answer in one sentence

Use **Bevy 0.19.0 on Rust 1.97.1** as a code-first, replaceable presentation shell around pure Rust protocol and session crates: the initial Diagnostic World is simple enough that Bevy's missing editor and immature higher-level widgets are tolerable, while one Rust workspace removes a language/FFI boundary, preserves the strongest protocol-library reuse path, and gives the World-entry Slice the most deterministic automated-test topology.

## Decision

Choose Bevy for the World-entry Slice, with these limits:

- Bevy owns the window, renderer, input sampling, placeholder scene, camera, diagnostic UI, and conversion between engine coordinates and domain values.
- Engine-independent Rust crates own packet codecs, cryptography, TCP sessions, protocol state, timing, reconnectable domain state, and the command/event API.
- No `bevy::*` type crosses into those core crates.
- Use only built-in primitive meshes, `StandardMaterial`, basic UI/text, input, and diagnostics. Do not add audio, a physics engine, a third-party character controller, or community UI/gameplay plugins to the initial slice.
- Freeze the engine version until World-entry acceptance. An engine upgrade is a deliberate migration, never an incidental dependency update.

This is a choice for this private learning client, not a claim that Bevy is the generally most mature engine. Godot 4.7.1 with C# is the closest alternative and would be preferred if editor-led content creation or short-term UI productivity became more important than a single-language protocol/testing workspace.

## Weighted comparison

Scores are a decision aid rather than benchmarks: `1` is a poor fit and `5` is a strong fit. The weighting reflects this slice's unusually protocol-heavy and test-heavy destination. The editor matters less because the Diagnostic World is project-owned primitive geometry rather than authored game content.

| Candidate | Core boundary and protocol reuse (25%) | Automation and deterministic tests (20%) | Initial 3D/UI/input productivity (20%) | Apple Silicon and Windows path (15%) | Stability and tooling maturity (10%) | Dependency/build/license simplicity (10%) | Weighted result |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| **Bevy 0.19.0 + Rust** | 5 | 5 | 4 | 5 | 2 | 5 | **90/100** |
| **Godot 4.7.1 .NET + pure C# core** | 4 | 4 | 5 | 5 | 4 | 4 | **87/100** |
| Fyrox 1.0.0 + Rust | 5 | 4 | 4 | 4 | 2 | 4 | **81/100** |
| Godot 4.7.1 Standard + Rust/GDExtension | 5 | 3 | 5 | 3 | 3 | 2 | **76/100** |

Bevy wins narrowly because its weaker editor and widget maturity do not block a plane, a capsule/cube, a chase camera, and a telemetry overlay. In exchange, the protocol trace, candidate Rust dependencies, engine adapter, test harnesses, and application share Cargo, types, tooling, and one memory-safety model. Bevy's own documentation is candid that it remains in an experimentation phase with breaking changes, so the score depends on strict version freezing rather than an expectation of API stability ([release cadence and maturity warning](https://bevy.org/learn/quick-start/introduction/), [0.18 to 0.19 migration guide](https://bevy.org/learn/migration-guides/0-18-to-0-19/)).

## Pinned baseline

Commit the following tool and dependency locks when implementation begins:

| Item | Pin | Reason/evidence |
| --- | --- | --- |
| Rust | `1.97.1` in `rust-toolchain.toml` | Current stable point release on the research date; it fixes an LLVM miscompilation ([Rust release announcement](https://blog.rust-lang.org/2026/07/16/Rust-1.97.1/)). |
| Bevy | `=0.19.0` | Current stable Bevy release, published 2026-06-19 ([release](https://bevy.org/news/bevy-0-19/)); upstream tag commit [`c6f634ca9f406d68ba5109d921247b654cb42c10`](https://github.com/bevyengine/bevy/tree/c6f634ca9f406d68ba5109d921247b654cb42c10). |
| Cargo graph | committed `Cargo.lock` | Prevents compatible-version resolution from changing the tested dependency graph. |
| Bevy features | `default-features = false`, features `3d` and `ui` | Keeps the first slice free of unused 2D/audio systems; Bevy documents its feature collections in the pinned [`Cargo.toml`](https://github.com/bevyengine/bevy/blob/c6f634ca9f406d68ba5109d921247b654cb42c10/Cargo.toml#L133-L196). |
| Initial native targets | `aarch64-apple-darwin`, `x86_64-pc-windows-msvc` | Native current Mac development and the later Windows path. |

Do not use a Git dependency, `main`, a version range such as `0.19`, nightly Rust, or unpinned community plugins. Bevy's examples explicitly warn that `main` can differ incompatibly from a release ([pinned examples guidance](https://github.com/bevyengine/bevy/blob/c6f634ca9f406d68ba5109d921247b654cb42c10/examples/README.md#L1-L15)). An engine bump must update the exact pin and lockfile together and pass every core, adapter, realm-integration, macOS-rendered, and Windows-build gate before merging.

## Why Bevy fits the Diagnostic World

The required visual surface is already inside the engine:

- Built-in primitive meshes, PBR `StandardMaterial`, lights, and `Camera3d` can produce the whole placeholder scene without external assets ([official 3D scene example](https://github.com/bevyengine/bevy/blob/c6f634ca9f406d68ba5109d921247b654cb42c10/examples/3d/3d_scene.rs#L1-L38)).
- Keyboard and mouse input plus fixed schedules support a clean movement loop. Follow Bevy's official pattern: sample input, advance the domain pose in `FixedUpdate`, interpolate the visual transform, and update the camera in rendered frames ([fixed-timestep example](https://github.com/bevyengine/bevy/blob/c6f634ca9f406d68ba5109d921247b654cb42c10/examples/movement/physics_in_fixed_timestep.rs#L33-L83)).
- Bevy 0.19 includes a diagnostics overlay that can show built-in and project-defined metrics ([0.19 diagnostics overlay](https://bevy.org/news/bevy-0-19/#diagnostics-overlay)). Its core UI supports flex/grid layout, text, images, and buttons ([Bevy UI API](https://docs.rs/bevy/0.19.0/bevy/ui/)).
- The higher-level widget crate is explicitly experimental and heavily changeable ([widget API warning](https://docs.rs/bevy/0.19.0/bevy/ui_widgets/index.html)). The slice must therefore keep UI to connection state, realm/session phase, map/position/orientation, movement state, errors, and simple controls.
- Bevy does not supply the needed third-person player controller. Its first-party camera-controller crate currently exposes free and pan controllers ([pinned source](https://github.com/bevyengine/bevy/blob/c6f634ca9f406d68ba5109d921247b654cb42c10/crates/bevy_camera_controller/src/lib.rs#L1-L43)). Implement the small chase/orbit camera and plane-bounded placeholder locomotion in this repository instead of importing a version-coupled plugin.

Apple Silicon is a proven upstream development path rather than an inferred cross-compile: Bevy runs graphical example and screenshot tests on `macos-14` M1 using Metal ([upstream workflow](https://github.com/bevyengine/bevy/blob/c6f634ca9f406d68ba5109d921247b654cb42c10/.github/workflows/example-run.yml#L21-L61)). The same workflow exercises Windows with DirectX 12 ([Windows jobs](https://github.com/bevyengine/bevy/blob/c6f634ca9f406d68ba5109d921247b654cb42c10/.github/workflows/example-run.yml#L185-L219)), and the normal CI matrix includes macOS and Windows ([CI matrix](https://github.com/bevyengine/bevy/blob/c6f634ca9f406d68ba5109d921247b654cb42c10/.github/workflows/ci.yml#L27-L59)). The project must still compile on Windows from its first client milestone and perform a rendered smoke test on actual Windows when Windows enters acceptance scope.

## Binding topology

Use a one-way Cargo workspace dependency graph:

```text
client_bevy  --->  client_session  --->  client_protocol
     |                   |                    |
 Bevy only        sockets/state/events    codecs/crypto
```

### `client_protocol`

- Owns typed login/world packets, framing, SRP6/header crypto, movement encoding, golden transcripts, and malformed-input handling.
- Has no async runtime, OS socket, Bevy, or UI dependency.
- May adapt pinned `wow_srp`/`wow_messages` code only after the compatibility issues in the protocol research are covered by local wire tests.

### `client_session`

- Owns the ordered login and world state machines, sockets, monotonic time, reconnect/logout behavior, and conversion from protocol messages into project domain events.
- Exposes bounded `ClientCommand` and `ClientEvent` channels. Commands include connect, authenticate, select configured character, enter world, movement intent/pose sample, and disconnect. Events include phase changes, world entry, authoritative pose, correction/error, and diagnostics.
- Uses project-owned scalar/domain types such as `WorldPose { map, x, y, z, orientation }`; it never imports Bevy vectors, entities, resources, or schedules.
- Owns its I/O task/thread so packet ordering never depends on Bevy scheduling. Bevy's task module expressly makes no fairness or ordering guarantee even though its I/O pool can wait on external I/O ([task documentation](https://docs.rs/bevy/0.19.0/bevy/tasks/index.html)).

### `client_bevy`

- Is the only crate allowed to depend on Bevy.
- Maps `Input` actions into domain commands and maps domain poses/events into ECS components, transforms, camera state, and diagnostic UI.
- Drains a bounded event queue at a defined schedule point; it does not parse packets or own socket state in ECS systems.
- Keeps local visual interpolation separate from the last server-observed/persisted pose, so the Diagnostic World can show disagreement and correction explicitly.

This is engine independence through a compile-time crate boundary, not through a process or ABI boundary. A later engine replacement supplies another outer adapter while retaining the tested protocol/session crates.

## Test and CI contract

1. **Protocol tests:** ordinary Rust unit, golden-byte, property, and fuzz tests cover codecs, crypto, framing, state transition guards, and the known AzerothCore movement-layout discrepancy without initializing Bevy.
2. **Session tests:** scripted transports and clocks drive success, timeout, malformed frame, disconnect, and reconnect sequences deterministically. The real Reference Realm integration test also runs at this layer without a window or GPU.
3. **Bevy adapter tests:** instantiate `App` with `MinimalPlugins`, inject input/domain events, call `app.update()`, and assert resulting commands, ECS state, transforms, and diagnostics. Bevy's own test suite documents this exact application-test pattern ([official example](https://github.com/bevyengine/bevy/blob/c6f634ca9f406d68ba5109d921247b654cb42c10/tests/how_to_test_apps.rs#L1-L64)).
4. **Windowed automation:** use Bevy's first-party CI testing support for a fixed frame time, scripted actions, bounded exit, and screenshot artifacts ([configuration source](https://github.com/bevyengine/bevy/blob/c6f634ca9f406d68ba5109d921247b654cb42c10/crates/bevy_dev_tools/src/ci_testing/config.rs#L5-L64)). Treat screenshots as diagnostic evidence, not exact cross-platform pixel equality.
5. **Rendered acceptance:** retain a manual M1 scenario for mouse capture, camera behavior, GPU rendering, movement visualization, and diagnostic legibility. Add a real Windows rendered/exported smoke when that platform becomes in scope.

The minimal Bevy plugin group supplies a schedule runner without a window ([`MinimalPlugins`](https://docs.rs/bevy/0.19.0/bevy/prelude/struct.MinimalPlugins.html)). Semantic tests should prefer it; renderer/device tests require a graphical runner.

## Godot language-boundary comparison

### Godot 4.7.1 .NET with a pure C# core

This is the strongest rejected option. Godot 4.7.1 is the current stable maintenance release and reports no known incompatibilities with 4.7 ([release](https://godotengine.org/article/maintenance-release-godot-4-7-1/)). Its official macOS binary is native Universal 2, including Apple Silicon ([macOS export documentation](https://docs.godotengine.org/en/4.7/tutorials/export/exporting_for_macos.html)), and C# is officially supported on macOS and Windows desktop ([C# platform support](https://docs.godotengine.org/en/4.7/tutorials/scripting/c_sharp/index.html)). `CharacterBody3D`, `SpringArm3D`, `InputMap`, Control nodes, profiler/debugger, and command-line headless/export support make its engine shell more mature than Bevy's ([third-person camera](https://docs.godotengine.org/en/4.7/tutorials/3d/spring_arm.html), [command line](https://docs.godotengine.org/en/4.7/tutorials/editor/command_line_tutorial.html)).

A pure C# class library for protocol/session logic plus a thin Godot C# assembly would preserve engine independence and use `dotnet test` without a project-owned FFI layer or separately compiled native extension. Godot still performs managed/native interop internally, and its documentation warns about marshalling costs and C# growing pains, minimal built-in editor support, and gaps in examples ([C# basics and warnings](https://docs.godotengine.org/en/4.7/tutorials/scripting/c_sharp/c_sharp_basics.html)). Godot itself recommends GDScript to newcomers while recognizing C# as official support ([language guidance](https://docs.godotengine.org/en/4.7/about/faq.html)).

The default Godot 4.7.1 project generator still targets `net8.0` ([pinned source](https://github.com/godotengine/godot/blob/a13da4feb8d8aefc283c3763d33a2f170a18d541f/modules/mono/editor/GodotTools/GodotTools.ProjectEditor/ProjectGenerator.cs#L13-L30)), but .NET 8 support ends 2026-11-10 while .NET 10 LTS is supported until 2028-11-14 ([Microsoft support policy](https://dotnet.microsoft.com/en-us/platform/support/policy/dotnet-core)). Choosing this route would therefore require explicitly pinning a tested .NET 10 SDK/target rather than silently accepting the soon-to-expire default.

Godot/C# loses narrowly here because no equivalent already-traced C# protocol library has been established, so it would either reimplement the WotLK codec/SRP path or add another language boundary. It also introduces parallel Godot/.NET build and test conventions. Those costs outweigh its editor advantage for a code-generated placeholder world, but this conclusion should be revisited if the project begins authoring substantial scenes or UI.

### Godot Standard with Rust through `godot-rust`

This keeps Rust protocol reuse and Godot's mature shell, but creates the most operationally fragile boundary of the finalists. GDExtension selects separate native libraries per operating system, architecture, and debug/release variant ([`.gdextension` format](https://docs.godotengine.org/en/4.7/engine_details/engine_api/gdextension/gdextension_file.html)). Every macOS and Windows export must first build and package the matching Rust dynamic library, and native panics or boundary mistakes can terminate the editor/client.

The current `godot-rust` release is v0.5.4, commit [`4396536a7e3eb01403ceb452f2d1253293817188`](https://github.com/godot-rust/gdext/tree/4396536a7e3eb01403ceb452f2d1253293817188). It is a community-maintained MPL-2.0 binding, not one of Godot's official languages; upstream describes it as usable but still subject to occasional breaking changes ([project status and license](https://github.com/godot-rust/gdext/tree/4396536a7e3eb01403ceb452f2d1253293817188#development-status)). Version 0.5.4 exposes an exact Godot 4.7 API feature ([pinned features](https://github.com/godot-rust/gdext/blob/4396536a7e3eb01403ceb452f2d1253293817188/godot/Cargo.toml#L34-L45)) and also documents backward-compatible API/runtime combinations ([compatibility](https://godot-rust.github.io/book/toolchain/compatibility.html)); this removes an immediate version-lag concern but still adds a separately moving compatibility and native-packaging layer.

If Godot were selected for this slice, prefer the official .NET build with a pure C# core over Standard plus Rust/GDExtension. Existing Rust code reuse is not large enough yet to justify per-platform native binaries, FFI lifecycle/threading rules, and a community binding. Godot/Rust becomes attractive only after a substantial, proven Rust protocol core exists and reimplementing it is demonstrably more costly than owning that native boundary.

## Other credible and excluded alternatives

### Fyrox 1.0.0

Fyrox is a credible all-Rust alternative: it is MIT-licensed, has an integrated scene editor and UI, supports both the engine and editor on macOS and Windows, and states that M1+ works well ([repository and release](https://github.com/FyroxEngine/Fyrox), [supported platforms](https://fyrox-book.github.io/introduction/requirements.html)). It can run without a graphics context and documents a CLI export/CI path ([headless graphics context](https://fyrox-book.github.io/engine/graphics_context.html), [CI/CD](https://fyrox-book.github.io/shipping/ci_cd.html)).

It ranks below Bevy because its smaller ecosystem and newly reached 1.0 baseline provide less evidence for long-term API/tool stability, while its macOS renderer baseline remains OpenGL 3.3 rather than Bevy's upstream-tested Metal path. Its book even recommends current repository code to obtain bug fixes ([manual installation guidance](https://fyrox-book.github.io/beginning/manual_installation.html)), which conflicts with this project's reproducibility policy. Reconsider it only if an integrated Rust editor becomes essential and Bevy's missing editor becomes the actual blocker.

### Explicit exclusions

- O3DE is Apache-2.0 and feature-rich, but its official documentation still marks macOS host/target support experimental and requires Intel `x86_64`; it is not a credible Apple Silicon-first baseline ([requirements](https://docs.o3de.org/docs/welcome-guide/requirements/), [platform status](https://www.docs.o3de.org/docs/welcome-guide/supported-platforms/)).
- Unity and Unreal are not candidates under the requirement for an open-source engine: access to source under proprietary terms or an EULA is not the same as an OSI-style open-source dependency.
- A custom renderer/framework stack is outside the settled requirement to use an existing engine and would shift the learning slice into engine construction.

## License and provenance boundary

Bevy is dual-licensed MIT OR Apache-2.0. Its example assets can carry separate licenses and are not included in the published crates ([upstream licensing note](https://github.com/bevyengine/bevy/blob/c6f634ca9f406d68ba5109d921247b654cb42c10/README.md#L103-L123)). Use only project-created primitives/material parameters and retain the applicable dependency notices. Godot is MIT, `godot-rust` is MPL-2.0, and Fyrox is MIT. These licenses do not relax the existing rule that no Blizzard client content enters the Learning Client.

## Failure modes and required guardrails

| Failure mode | Guardrail |
| --- | --- |
| A Bevy upgrade consumes the slice in migration work | Exact `=0.19.0` pin, committed lockfile, tagged docs/examples only, and no upgrade before acceptance. |
| Community plugin versions drag the engine forward | No community gameplay/UI/physics plugins in the initial slice. Implement the tiny camera/controller locally. |
| First builds and links become slow | Minimal feature set, no audio/2D, incremental local builds; keep release and CI builds statically linked. Bevy's setup guide warns that initial builds are substantial ([setup](https://bevy.org/learn/quick-start/getting-started/setup/)). |
| Metal-specific rendering limitations appear | Use primitive meshes and `StandardMaterial`; avoid bindless/custom render features. Bevy documents remaining Metal bindless limits ([0.19 rendering note](https://bevy.org/news/bevy-0-19/#partial-bindless--reduced-bind-group-overhead)). |
| Protocol ordering leaks into ECS scheduling | Session-owned ordered I/O worker plus bounded command/event queues; never let independent ECS systems mutate socket/cipher state. |
| Headless tests give false confidence | Separate semantic headless tests from M1 rendered acceptance and the later real Windows rendered smoke. |
| Windows portability remains theoretical | Compile the Windows target in CI from the first client milestone; require hardware/rendered proof before Windows becomes an acceptance platform. |
| The missing editor becomes costly as content grows | Keep the core engine-independent. If authored scenes/UI exceed the Diagnostic World, reopen the engine choice with Godot .NET/C# as the default challenger. |

## Consequences for later planning

- The build topology is now specifiable as one pinned Rust workspace plus the separately pinned Docker Reference Realm.
- The engine shell can be proven before protocol integration with a primitive world, input-to-command mapping, diagnostics overlay, headless `App` test, M1 rendered smoke, and Windows compile job.
- The client architecture must define the exact `ClientCommand`/`ClientEvent` contract and scheduling boundary before implementing networking.
- Reconnect, corrections, and telemetry remain domain/session questions; they must not be implemented as Bevy-specific state transitions.
- A later move from Diagnostic World to authored content is the explicit re-evaluation trigger, not ordinary Bevy inconvenience or the existence of a newer release.
