# Slice 13 entry-gate evidence

Recorded: `2026-07-22T16:16:07Z`

Exact predecessor: `c300c12101f2ed49df8919c9f5f9006a71f7ff4c`

## Production scaffold

`scripts/check.sh` passed on the current branch with Rust `1.97.1 (8bab26f4f 2026-07-14)` and Cargo `1.97.1 (c980f4866 2026-06-30)`. The gate covered formatting, locked native workspace/all-target compilation, Clippy with warnings denied, all 26 native tests, the exact four-package one-way dependency graph and direct dependency pins, and the `x86_64-pc-windows-msvc` compile tripwire.

`scripts/render-smoke.sh /tmp/miazcore-slice13-entry` passed on macOS arm64 using Metal:

- screenshot: PNG, `2560x1440`, SHA-256 `27c8705ad4ef1ceedd64d472caff30b0f35d7534569be7d3d1998bc9e20a9f6d`;
- semantic sidecar: SHA-256 `63f460f5314c1392876aa4346b56b0da172f1ec702a5cb3a508b5aa60446ddf4`; and
- the offline proof retained its explicit `NO SOCKETS / NO PACKETS` claim.

The Windows result remains a compile tripwire only. It is not Windows linking, testing, runtime, or rendering evidence. Render artifacts are intentionally ephemeral.

## Reference Realm

`infra/azerothcore/realm health` passed orchestration, process, host-socket, realm-row, account, and exact fixture-character checks. The pinned authserver and worldserver containers were healthy, and host sockets `127.0.0.1:3724` and `127.0.0.1:8085` were reachable.

No production login socket was opened by the Learning Client during this entry gate. Authenticated realm discovery is the capability this slice must add.
