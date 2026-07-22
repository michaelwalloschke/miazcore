# Slice 13 exit-gate evidence

Recorded: `2026-07-22T17:12:36Z`

Exact passing candidate: `033585880c3eb65ef2f61b372a2ee9112429e672`

Exact predecessor: `c300c12101f2ed49df8919c9f5f9006a71f7ff4c`

The candidate was clean when every exit command below ran. The later evidence-only
commit changes only this record, the local ticket resolution, and the map gist.

## Deterministic and platform gates

`scripts/check.sh` passed on the exact candidate. It covered Rustfmt, locked native
workspace/all-target compilation, Clippy with warnings denied, 41 native tests,
the exact four-package one-way dependency graph and dependency pins, and the
`x86_64-pc-windows-msvc` all-target compile tripwire.

The native tests included five independent protocol golden/negative tests and 24
`client_session` unit tests. They exercised manifested synthetic login and header
crypto vectors, full SRP6 proof verification, the exact Wrath SRP group, fragmented
reads, malformed frames, rejection, timeout, entropy failure, success, active
cancellation, backpressure, shutdown, and credential/session-material redaction.

`scripts/render-smoke.sh /tmp/miazcore-slice13-0335858` passed on macOS arm64 using
Metal. Visual inspection confirmed the project-owned Diagnostic World remained
explicitly `Offline`, retained `NO SOCKETS / NO PACKETS`, and exposed no connection
action:

- screenshot: PNG, `1280x720`, SHA-256
  `264b08628e6380bc9c8fb9b9f1fab7621653aa9e66eea48e62a75904fbb01f4a`;
- semantic sidecar: SHA-256
  `63f460f5314c1392876aa4346b56b0da172f1ec702a5cb3a508b5aa60446ddf4`.

The Windows result remains a compile tripwire only. It is not Windows linking,
testing, runtime, or rendering evidence. Render artifacts are intentionally
ephemeral.

## Live Reference Realm gate

`scripts/live-realm-discovery.sh` passed against the healthy pinned Docker realm.
Health passed before and after the attempt, including orchestration, processes,
both host sockets, and the semantic fixture. The production worker reported only
sanitized evidence:

```text
realm discovery: PASS realm=1 name=Miazcore Reference Realm build=12340 endpoint=127.0.0.1:8085 semantic_events=8 disconnected=true
```

The worker performed one real login connection, authenticated the ignored fixture
account, verified the server proof, decoded and selected the exact realm, verified
its advertised world endpoint, closed the login transport, and stopped. It did not
open the world socket or make character-entry or movement claims.

## Secret and review gates

All four repository-local secret files were ignored, mode `0600`, and absent from
every line added since the predecessor. The committed fixture corpus contains only
the documented synthetic account, password, entropy, keys, and independently
calculated frames; it contains no live credential, session key, or authenticated
capture.

The required two-axis code review compared the candidate with the exact predecessor:

- Standards found no hard violation. Its duplicated bounded-diagnostic retention
  judgment was resolved by one helper; its low parameter-clump judgment was retained
  because the explicit private transport, clock, entropy, and boundary parameters
  are required dependency seams. The focused follow-up passed with no new
  medium/high smell.
- Spec found active cancellation and exact SRP-group validation incomplete. The
  final candidate checks cancellation at every protocol seam, drains the shutdown
  command, returns to `Offline`, closes the transport, and emits `Disconnected`;
  it also rejects any generator or prime other than the locked Wrath values. Both
  fixes have red-to-green deterministic regressions, and the spec follow-up marked
  both findings resolved.

## Remaining deferrals

- World authentication and live encrypted world-header integration.
- Character enumeration or selection and `CMSG_PLAYER_LOGIN`.
- Bevy-driven connection, prediction, movement submission, and Movement Proof.
- Native Windows build, test, runtime, and rendering acceptance.

`StartEntry` remains headless and engine-independent. The production Bevy
application still uses `OfflineSession`, so this intermediate capability cannot
become partially user-facing.
