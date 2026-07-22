# Prove the Bevy shell and platform test path

Type: wayfinder:task
Status: resolved
Blocked by: [Choose the Learning Client engine direction](03-choose-engine-direction.md)

## Question

Can the exactly pinned Rust 1.97.1 and Bevy 0.19.0 baseline produce a throwaway project-owned primitive scene with basic built-in UI and a small local chase/orbit camera on the current Apple Silicon machine, exercise scripted engine-free events through a headless `MinimalPlugins` `App`, and compile the same boundary for `x86_64-pc-windows-msvc`; and what exact feature flags, commands, artifacts, platform limitations, or fallback triggers must the architecture and verification tickets rely on?

## Answer

Yes. The [Bevy shell and platform path proof](../research/bevy-shell-platform-proof.md)
demonstrates the exact Rust 1.97.1 / Bevy 0.19.0 baseline with
`default-features = false` and only Bevy's `3d` and `ui` feature collections.
Its disposable [prototype](../prototypes/bevy-shell-platform/README.md) renders a
project-owned primitive world, built-in diagnostic UI, and a local chase/orbit
camera through Metal on the Apple M1 Max; proof mode sends an engine-free domain
movement event through the thin adapter before saving a real screenshot.

The same engine-free events pass through a headless `MinimalPlugins` `App` in an
integration test, alongside two pure-model tests. Formatting, native all-target
checking, and clippy with warnings denied pass. The library, binary, unit tests,
and integration test also compile-check for `x86_64-pc-windows-msvc` from macOS
when the Windows target enables BLAKE3 1.8.5's `pure` feature and the command sets
`CC_x86_64_pc_windows_msvc=clang`; without that bounded workaround BLAKE3's build
script incorrectly requires Microsoft's unavailable `ml64.exe` on the Mac host.

The architecture may therefore keep the selected Bevy direction and the strict
engine-free core/thin-adapter boundary. Verification must distinguish the green
Mac-to-MSVC `cargo check` portability tripwire from actual Windows evidence:
non-Windows-to-MSVC cross-compilation is unsupported by Rust, the check does not
link or run, and Windows still needs native tests/build plus a rendered smoke
before it becomes an acceptance platform. The proof records the exact commands,
artifact hash, warnings, platform limits, and narrow Godot/reopening triggers.
