# Slice 12 entry-gate evidence

Recorded: `2026-07-22T14:28:44Z`

Candidate before implementation: `1f09a44832c30d2244262571c55a31582e0ba146`

## Reference Realm

- `infra/azerothcore/realm health`: passed orchestration, process, host-socket, realm-row, account, and exact fixture-character checks.
- `infra/azerothcore/realm smoke`: authenticated build 12340, selected realm ID 1 at `127.0.0.1:8085`, authenticated the world session, and enumerated exactly one `Miaztest`.

## Disposable Bevy proof

- `cargo test --locked`: 2 engine-free model tests and 1 headless `MinimalPlugins` adapter test passed.
- `CC_x86_64_pc_windows_msvc=clang cargo check --locked --all-targets --target x86_64-pc-windows-msvc`: passed.
- `WGPU_BACKEND=metal RUST_LOG=info cargo run --locked -- --proof-output <temporary>/macos-shell.png`: passed with `Apple M1 Max` and backend `Metal`.
- Render proof: PNG, 2560×1440, SHA-256 `c48adfc292496f5a1475c8aa52eff9aceed0df83aaf7dc20ffa4e788e43ffa97`.
- The known macOS debug-link `__eh_frame` warning and post-capture winit destroyed-window warning remained non-fatal and unchanged from the recorded feasibility proof.

The rendered entry artifact and log were intentionally ephemeral; the committed disposable proof retains the identical image hash and its durable feasibility record.
