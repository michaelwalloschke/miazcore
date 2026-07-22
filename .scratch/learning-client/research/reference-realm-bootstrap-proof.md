# Reference Realm bootstrap proof

Proof date: 2026-07-22

## Outcome

The digest-pinned Reference Realm passes the clean bootstrap contract on the current Apple Silicon development host when Docker Desktop uses Apple Virtualization Framework with Rosetta. The repository now owns only original orchestration, lifecycle/verification tooling, a protocol bootstrap probe, and one server-generated player dump; it does not contain AzerothCore source or downloaded server data.

## Host and emulation evidence

| Field | Observed value |
| --- | --- |
| Host | macOS 26.5.2, `arm64` |
| Docker Desktop engine | 29.6.2, `linux/arm64` daemon |
| Docker Compose | 5.3.1 |
| Docker settings | `UseVirtualizationFramework=true`, `UseVirtualizationFrameworkRosetta=true` |
| AzerothCore runtime platform | `linux/amd64` through Rosetta |
| MySQL runtime platform | native `linux/arm64` |

Under the original Docker Desktop settings (`UseVirtualizationFramework=false`, `UseVirtualizationFrameworkRosetta=false`), the imported pinned worldserver shell ran as `x86_64`, but the binary terminated before configuration or database access with `qemu: uncaught target signal 11 (Segmentation fault)`. After the user enabled Apple Virtualization Framework and Rosetta and restarted Docker Desktop, the same binary returned:

```text
AzerothCore rev. a4ab07218aa0+ ... (Unix, RelWithDebInfo, Static)
```

This setting is therefore a measured prerequisite for the current macOS slice, not an optional performance tweak.

## Effective artifact evidence

Every effective image ID matched its locked manifest digest:

| Component | Architecture | Effective image ID / locked digest |
| --- | --- | --- |
| MySQL 8.4 | `arm64` | `sha256:c592c15aaf4a1961e15d82eb31ea5987dda862d1c4b1e93424438c0e91dc1f8d` |
| Worldserver | `amd64` | `sha256:0a601595920e19c4af10679e4c01ac10f60569fc1e737db54aa6a5a07efb2455` |
| Authserver | `amd64` | `sha256:d5b017e40d256c7ce7bd16ad3a1127c985347431110243876048c48630a72492` |
| Database import | `amd64` | `sha256:a44c9e1f6cf491ef6ff728a1e433fa0acf09e339d6436a3f0dc7fe2bc74b7dbd` |
| Server-data downloader | `amd64` | `sha256:bc9dc009addafcb57f5a25787e85e7bf8c77571f813fe97e1f555ebe63d71dcc` |

The server-data wrapper downloaded exactly 1,196,168,257 bytes for `v20.0/Data.zip`, verified SHA-256 `a3d4df635ae6c2c8f08052c32a79e0f806955150ad36b014a823dd08a32a4610`, checked `dbc/`, `maps/`, `vmaps/`, and `mmaps/`, and only then wrote the volume marker. Later runs reported a verified cache hit against the same lock.

The rendered Compose model was checked to require `@sha256:` for every service image, publish exactly `127.0.0.1:3724` and `127.0.0.1:8085`, publish no database port, and define neither privileged services nor fixed container names.

## Fixture generation and invariant

The bootstrap probe performed real protocol-8/build-12340 SRP6 login, authenticated realm discovery, world authentication with encrypted headers, and `CMSG_CHAR_CREATE`. AzerothCore returned `0x2f` success. The pinned worldserver then generated `reference-character.pdump` through `pdump write`; no character-table insertion was used.

| Field | Verified value |
| --- | --- |
| Realm | ID `1`, `Miazcore Reference Realm`, address `127.0.0.1`, port `8085`, build `12340` |
| Account | ID `1`, `MIAZTEST`, expansion `2`, no IP/country lock conflict, no TOTP secret |
| Character | GUID `1`, `Miaztest`, human warrior, male, level `1`, offline |
| Entry Anchor | map `0`, zone `0`, `(-8949.95, -132.493, 83.5312)`, orientation `0` |
| Movement/transport baseline | transport GUID and transport position/orientation all zero |
| Player dump | 11,172 bytes, SHA-256 `030fcd8c563eedc14cc5bc2929427489178b910cc78f25e064198db1d7ea1e32` |

Provisioning creates a missing account in a dedicated short-lived worldserver run, restarts so AzerothCore's account cache can resolve it, resets the secret-derived password/addon level, loads the dump only when `Miaztest` is absent, and fails if ownership or uniqueness drifts. Console output is filtered in-process so passwords appear only as `[REDACTED]`; committed files, rendered Compose, repository scans, and persistent container logs contain no password secret.

## Lifecycle proof

All timings are wall-clock on this host. Network and registry performance are not contractual.

| Scenario | Result | Wall time |
| --- | --- | ---: |
| Final `reset-all --yes` | label-checked and replaced both volumes; re-downloaded/re-verified data; imported DBs; loaded fixture; recreated servers; semantic health passed | 178.96 s |
| `reset-state --yes` | removed only the labeled `state` volume; preserved verified server-data volume; reproduced fixture | 158.77 s |
| Ordinary idempotent `up` with final fresh-server rule | no duplicate account/character; cache hit; fresh auth/world processes; health passed | 25.19 s |
| `down` then `up` | preserved both volumes and the exact fixture invariant | 29.76 s before the final fresh-server hardening (approximately 10 s is now deliberately added) |

The final full reset created new labeled volumes at `2026-07-22T08:21:24Z`:

```text
miazcore-reference-realm_database-data  kind=state
miazcore-reference-realm_server-data    kind=server-data
```

Both reset commands enumerated exact volume names before deletion and inspected `com.miazcore.reference-realm=true` plus the expected `kind` label. No wildcard deletion, prune, unrelated container, unrelated network, or unrelated volume was used.

## Final health and protocol smoke

`realm health` passed all orchestration/process/semantic checks: healthy MySQL, completed client-data/import/provisioner jobs, healthy auth/world processes, reachable localhost ports, exact realm row, unrestricted fixture account, and one offline ordinary-ground character.

After the final clean bootstrap, `realm smoke` returned:

```text
protocol smoke: authenticated build 12340; realm 1 at 127.0.0.1:8085
protocol smoke: authenticated world session enumerated exactly one Miaztest
```

This proves credentials through real login and world authentication without claiming world entry or movement, which remain Learning Client work.

## Constraints for later tickets

- Apple Silicon acceptance requires Apple Virtualization Framework plus Rosetta for the currently locked AzerothCore images. QEMU alone is not a viable fallback for this build.
- The server-data volume is private server input. It is neither committed nor exposed to the Learning Client.
- The advertised address remains configurable; only `127.0.0.1` is accepted in this macOS slice. A later Windows client will require a deliberate LAN address/firewall exposure change.
- AzerothCore runtime warnings about unavailable process-priority elevation and the bundled optional ALE module configuration were non-fatal; the locked core reached healthy auth/world service and passed the protocol smoke.
