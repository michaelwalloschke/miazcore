# Choose the Learning Client engine direction

Type: wayfinder:research
Status: resolved

## Question

Which open-source engine best supports a macOS-first Diagnostic World, a credible later Windows build, third-person 3D controls and diagnostics, an engine-independent protocol core, and automated integration testing—and which language and binding trade-offs should determine the choice?

## Answer

Use Bevy `0.19.0` on Rust `1.97.1`, pinned exactly with a committed `Cargo.lock`, as a code-first and replaceable presentation shell. Keep protocol codecs/crypto and ordered network/session state in engine-free Rust crates, and make a thin outer Bevy crate the only layer allowed to own ECS types, rendering, input, camera, primitive Diagnostic World geometry, and UI. This single-language, one-way Cargo dependency graph removes an FFI and packaging boundary and enables deterministic core, session, and headless `App` tests; Bevy's missing editor and immature higher-level widgets are acceptable because this slice deliberately uses project-created primitives and basic diagnostics. Freeze Bevy through World-entry acceptance, omit audio, third-party physics/controllers/UI plugins, and require macOS rendering plus a Windows compile path from the first client milestone. Godot 4.7.1 with a pure C# core is the fallback if authored scenes or substantial UI—not ordinary engine inconvenience—become necessary. The weighted comparison, exact topology, test contract, pinning policy, alternatives, licenses, and re-evaluation triggers are recorded in [Learning Client engine direction](../research/learning-client-engine-direction.md).
