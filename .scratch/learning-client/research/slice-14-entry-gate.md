# Slice 14 entry-gate evidence

Recorded: `2026-07-22T17:24:51Z`

Exact predecessor: `7ee1770df7c469b769be5f8e27c092a698c4a578`

## Deterministic predecessor gate

`scripts/check.sh` passed on the current branch with only ticket 14's claim
uncommitted. The gate covered Rustfmt, locked native workspace/all-target
compilation, Clippy with warnings denied, all 41 native tests, the exact
four-package one-way dependency graph and dependency pins, and the
`x86_64-pc-windows-msvc` all-target compile tripwire.

The Windows result remains a compile tripwire only. It is not Windows linking,
testing, runtime, or rendering evidence.

## Live Reference Realm predecessor gate

`scripts/live-realm-discovery.sh` passed against the pinned Docker realm. Health
passed before and after the attempt, including orchestration, processes, both host
sockets, and the semantic fixture. The ticket 13 production worker reported:

```text
realm discovery: PASS realm=1 name=Miazcore Reference Realm build=12340 endpoint=127.0.0.1:8085 semantic_events=8 disconnected=true
```

No world socket was opened by the Learning Client during this entry gate. Fresh
world authentication and exact `Miaztest` selection are the capability ticket 14
must add.
