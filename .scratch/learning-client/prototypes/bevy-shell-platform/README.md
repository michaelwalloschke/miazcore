# Bevy shell and platform proof

This is a disposable feasibility prototype for learning-client ticket 11. It is
not the production client scaffold. Delete this directory once the durable
architecture and verification tickets have absorbed the proof.

The prototype deliberately contains two boundaries:

- `src/model.rs` is an engine-free state and event model. It has no Bevy imports.
- `src/adapter.rs` and `src/main.rs` are a thin Bevy adapter and primitive shell.

Run the native interactive shell:

```sh
cargo run --locked
```

Move with `WASD`. Orbit with the arrow keys or by dragging with the left mouse
button. Zoom with `Q` and `E`.

Produce the native rendered proof artifact and exit automatically:

```sh
WGPU_BACKEND=metal cargo run --locked -- --proof-output artifacts/macos-shell.png
```

Exercise the engine-free scripted events through a headless Bevy app:

```sh
cargo test --locked
```

From macOS, compile-check the same library, binary, and test boundary for
Windows MSVC:

```sh
CC_x86_64_pc_windows_msvc=clang \
  cargo check --locked --all-targets --target x86_64-pc-windows-msvc
```

The target-specific `blake3` `pure` feature and explicit Clang selection keep
its build script from requiring Microsoft's `ml64.exe` on the Mac host. The
final command proves Rust/Bevy target compilation only. It does not link or run
a Windows executable from macOS and does not replace `cargo test` plus a
rendered smoke test on a real Windows machine. On Windows with the Visual Studio
C++ tools installed, omit the `CC_...=clang` prefix.
