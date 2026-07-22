# Slice 14 exit-gate evidence

Recorded: `2026-07-22T18:19:08Z`

Exact passing candidate: `08472c59de9f6a23bc8d9bf54b5f32877d8e3bad`

Exact predecessor: `7ee1770df7c469b769be5f8e27c092a698c4a578`

The candidate was clean when every final exit command below ran. The later
evidence-only commit changes only this record, the local ticket resolution, and
the map gist.

## Deterministic and platform gates

`scripts/check.sh` passed on the exact candidate. It covered Rustfmt, locked
native workspace/all-target compilation, Clippy with warnings denied, 49 native
tests, the exact four-package one-way dependency graph and dependency pins, and
the `x86_64-pc-windows-msvc` all-target compile tripwire.

The protocol suite contained ten independent unit/golden tests. Its five new
versioned world fixtures and manifests proved exact challenge/auth-session bytes,
separate multi-packet inbound/outbound header streams, one-byte fragmentation,
coalescing, safe complete unknown-opcode handling, full character-record
consumption, malformed headers, cipher drift, truncation, and trailing data.

The 28 `client_session` unit tests covered the ordered login/world/character
stages, exact `Miaztest` projection, world-auth rejection, absent and duplicate
character outcomes, timeout, EOF, cipher drift, malformed frames, cancellation,
closure of both transports, clean failure disconnect, no retry, and structural
credential/session-material redaction. All predecessor login-only and offline
behavior remained green.

`scripts/render-smoke.sh /tmp/miazcore-slice14-08472c5` passed using Metal. The
proof capture waits 180 presented frames so a cold shader cache cannot produce a
black artifact. Visual inspection confirmed the project-owned Diagnostic World
remained explicitly `Offline`, retained `NO SOCKETS / NO PACKETS`, and exposed no
connection action:

- screenshot: PNG, `1080x720`, SHA-256
  `e7d2b8d716dc4e77f9631f5d51427bd035655d01f531d549aa430bd6069432ff`;
- semantic sidecar: SHA-256
  `63f460f5314c1392876aa4346b56b0da172f1ec702a5cb3a508b5aa60446ddf4`.

The validator retains a credible `1024x720` minimum because macOS may constrain
the obtained window width to the active display. PNG validity, content size,
Metal backend, exact semantic sidecar, redaction, hashing, and visual inspection
remain required. The Windows result remains a compile tripwire only.

## Live Reference Realm gate

`scripts/live-character-selection.sh` passed on the exact candidate while holding
the exclusive repository lock. Its label-checked `reset-state --yes` removed only
the managed database/config state, retained the verified server-data cache,
reprovisioned the fixture, and passed layered health before and after all client
attempts.

The required negative probes ran first and compared their complete stable
diagnostic contracts without printing credential material:

```text
character selection negative probe: PASS category=Authentication stage=login authentication configured_character=not-applicable recovery=CheckCredentials retries=0 disconnected=true player_login_sent=false
character selection negative probe: PASS category=Configuration stage=character selection configured_character=Miazmissing recovery=FixConfiguration retries=0 disconnected=true player_login_sent=false
```

The production success path then performed a fresh login, verified realm
discovery, world challenge/session authentication, encrypted header exchange,
complete enumeration, and exact single-character selection:

```text
character selection: PASS name=Miaztest level=1 map=0 area=0 semantic_events=11 disconnected=true player_login_sent=false
```

The exact semantic path ended at `Entering(CharacterSelection)`, emitted one
sanitized `CharacterSelected`, never entered `Bootstrap`, and disconnected before
`CMSG_PLAYER_LOGIN`.

## Secret and review gates

All four repository-local secret files were ignored and mode `0600`. No current
password value appeared in tracked content,
and the added-line private-key/session-material pattern scan passed. Committed
fixtures contain only the documented synthetic credentials, entropy, key, and
independently specified frames; no live session key or authenticated capture is
committed.

The required two-axis review compared the final candidate with the exact
predecessor. Standards found no hard violation. Its initial state-machine naming,
duplicated headless lifecycle, and duplicated transport/protocol error-mapping
judgments were resolved by `EntryMachine`, a shared private `HeadlessSession`, and
centralized mappings. Spec review's live-gate findings were resolved by the
label-scoped reset, negative-first order, exact diagnostic tuple assertions,
zero-retry/clean-disconnect checks, no-player-login stage checks, and final health.
Both final follow-ups returned `PASS`.

## Remaining deferrals

- `CMSG_PLAYER_LOGIN`, initialization/bootstrap consumption, and world verification.
- Authoritative self state, control/time synchronization, and run-speed handling.
- Bevy-driven live entry, prediction, movement submission, and Movement Proof.
- Native Windows build, test, runtime, and rendering acceptance.

The production Bevy application still uses `OfflineSession`; authenticated
character selection is available only through the engine-independent headless
harness.
