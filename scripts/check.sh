#!/usr/bin/env bash
set -euo pipefail

miazcore_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$miazcore_root"

cargo fmt --all -- --check
cargo check --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo metadata --locked --no-deps --format-version 1 | scripts/check_dependency_boundaries.py
CC_x86_64_pc_windows_msvc=clang \
  cargo check --locked --workspace --all-targets --target x86_64-pc-windows-msvc
