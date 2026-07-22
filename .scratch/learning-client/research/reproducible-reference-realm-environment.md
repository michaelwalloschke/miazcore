# Reproducible Reference Realm environment

Research date: 2026-07-21

## Answer in one sentence

The Reference Realm should be a repository-owned, minimal Docker Compose stack that pins one internally consistent AzerothCore build by platform-manifest digest, verifies the separately downloaded server-data archive, provisions one account and one server-generated player-dump fixture through the pinned worldserver, publishes only auth and world ports, exposes layered health checks, and resets only resources carrying this Compose project's labels.

## Decision

Do not copy either AzerothCore's source-tree Compose file or the complete `acore-docker` stack into this repository. Use both as upstream evidence, then maintain a small original orchestration layer in this repository.

The source-tree Compose file contains build contexts that require an AzerothCore checkout. The reusable [`acore-docker` project](https://github.com/azerothcore/acore-docker/tree/dab87ea3647bb8c398ab904034eddf74adaed285) demonstrates the supported precompiled-image path, but its current stack also contains development conveniences that the Reference Realm does not need. Its Compose file is AGPL-covered, so an independently written minimal topology also gives this repository a cleaner provenance boundary than copying and pruning that file. This is an engineering constraint, not legal advice.

## Pinned upstream baseline

All AzerothCore component images must come from the same successful upstream build. The selected build is the successful [Docker workflow run](https://github.com/azerothcore/azerothcore-wotlk/actions/runs/29865181015) for source commit [`a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f`](https://github.com/azerothcore/azerothcore-wotlk/tree/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f). The upstream workflow builds and publishes the four components together ([workflow definition](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/.github/workflows/docker_build.yml#L65-L127)).

The repository should store these values in a human-readable artifact lock, including the source SHA, tag used for discovery, platform, digest, resolution date, and provenance URL. Compose must consume the digest, not the mutable tag.

| Component | Locked image reference | Platform | Purpose |
| --- | --- | --- | --- |
| MySQL | `mysql:8.4@sha256:c592c15aaf4a1961e15d82eb31ea5987dda862d1c4b1e93424438c0e91dc1f8d` | native `linux/amd64` or `linux/arm64/v8` selected from the index | Auth, characters, and world databases |
| Worldserver | `acore/ac-wotlk-worldserver:17.0.0-dev@sha256:0a601595920e19c4af10679e4c01ac10f60569fc1e737db54aa6a5a07efb2455` | `linux/amd64` | Realm process and authoritative provisioning CLI |
| Authserver | `acore/ac-wotlk-authserver:17.0.0-dev@sha256:d5b017e40d256c7ce7bd16ad3a1127c985347431110243876048c48630a72492` | `linux/amd64` | Login service |
| Database import | `acore/ac-wotlk-db-import:17.0.0-dev@sha256:a44c9e1f6cf491ef6ff728a1e433fa0acf09e339d6436a3f0dc7fe2bc74b7dbd` | `linux/amd64` | Schema and AzerothCore database content |
| Server data downloader | `acore/ac-wotlk-client-data:17.0.0-dev@sha256:bc9dc009addafcb57f5a25787e85e7bf8c77571f813fe97e1f555ebe63d71dcc` | `linux/amd64` | One-shot acquisition of maps, DBC, vmaps, and mmaps used by worldserver |

The AzerothCore digests above are the `linux/amd64` platform manifests, not the multi-platform attestation indexes. The upstream images currently publish no Arm64 runtime variant. On the current Apple M1 development machine, every AzerothCore service must therefore declare `platform: linux/amd64`; MySQL can remain native. This emulated path is a risk to be measured by the bootstrap proof, not treated as already portable.

Digest updates are deliberate changes. An update procedure must resolve all four AzerothCore images from one workflow/source SHA, verify the published provenance, update the lock atomically, and rerun the clean bootstrap proof. Floating `master`, `17.0.0-dev`, or `latest` tags are forbidden in the effective Compose model.

## Server-side data artifact

The worldserver still needs extracted server data even though the Learning Client uses no Blizzard client assets. The pinned downloader image currently fetches `wowgaming/client-data` release [`v20.0`](https://github.com/wowgaming/client-data/releases/tag/v20.0) at runtime ([download implementation](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/apps/installer/includes/functions.sh#L156-L184)). Pinning the image alone is therefore insufficient: its stock command does not verify the downloaded archive.

The repository-owned one-shot wrapper must instead enforce this lock:

| Field | Locked value |
| --- | --- |
| Release | `v20.0`, published 2026-07-19 and marked immutable by GitHub |
| Asset | `Data.zip` |
| Size | `1,196,168,257` bytes |
| SHA-256 | `a3d4df635ae6c2c8f08052c32a79e0f806955150ad36b014a823dd08a32a4610` |
| Destination | Docker named volume mounted at `/azerothcore/env/dist/data` |

The wrapper may reuse tools already present in the pinned downloader image, but it must download to a temporary filename, verify byte size and SHA-256 before extraction, write a lock/version marker only after successful extraction, and leave no partial installation on failure. Subsequent starts may reuse the volume only if the marker and required directory checks pass.

GitHub reports no asserted SPDX license for this data repository. The asset is accepted only as a server-local input for this private learning environment: keep it in a Docker volume, do not commit or redistribute it, and never expose it to the Learning Client. Public distribution would require a fresh provenance and licensing decision.

## Minimal service topology

| Service | Lifecycle | Dependencies | Required behavior |
| --- | --- | --- | --- |
| `database` | long-running | none | Pinned MySQL, private network, named database volume, native platform, health checked; no host-published database port. |
| `client-data` | one-shot | none | Populate the named server-data volume from the locked and checksum-verified archive; exit zero only when complete. |
| `db-import` | one-shot | healthy database | Run the pinned AzerothCore importer against the three internal databases; exit zero gates all later services. |
| `fixture-provisioner` | one-shot | database import and client data completed | Start the pinned worldserver in provisioning mode, establish the realm/account/character invariant, verify it, then exit zero. |
| `authserver` | long-running | database import and fixture provisioning completed | Publish host TCP `3724`; generate configuration from the pinned image on each container creation. |
| `worldserver` | long-running | database import, client data, and fixture provisioning completed | Mount server data read-only, publish host TCP `8085`, disable its detached interactive console, and generate configuration from the pinned image on each container creation. |

The upstream runtime targets, entrypoint, and data volume are visible in the pinned [Dockerfile](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/apps/docker/Dockerfile#L149-L249). The image entrypoint copies its pinned default configuration into a writable directory before launching the process ([entrypoint](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/apps/docker/entrypoint.sh#L36-L54)). Use per-container writable `tmpfs` mounts for `/azerothcore/env/dist/etc` and `/azerothcore/env/dist/logs`, with UID/GID permissions proven on Docker Desktop, so generated configuration neither dirties the repository nor survives an image update.

Do not include phpMyAdmin, Eluna tooling, the source build contexts, SOAP, a host database port, fixed `container_name` values, privileged containers, or AzerothCore source mounts. Only `127.0.0.1:3724` and `127.0.0.1:8085` should be published for the initial slice.

## Configuration contract

AzerothCore maps configuration keys to `AC_` environment variables by uppercasing and replacing punctuation with underscores ([configuration loader](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/common/Configuration/Config.cpp#L370-L443)). The effective controlled settings are:

- authserver: `AC_STRICT_VERSION_CHECK=0` and the internal auth database connection;
- worldserver: `AC_WARDEN_ENABLED=0`, `AC_CONSOLE_ENABLE=0`, and internal auth/world/character database connections;
- both: updates disabled because `db-import` owns migrations, no interactive prompts, and no idle database-connection closure, consistent with the published runtime image defaults;
- realm row ID `1`, name `Miazcore Reference Realm`, build `12340`, unlocked/online, port `8085`, and advertised/local address `127.0.0.1` for the macOS slice;
- one account with no TOTP secret, IP lock, country lock, or ban; and no login queue during acceptance;
- one fixture character named `Miaztest`, owned by that account, placed at a stable flat starting coordinate with no transport, vehicle, flight, root, hover, water-walk, feather-fall, or other special movement state.

The advertised realm address must be an input with `127.0.0.1` as this slice's checked default, rather than being hard-coded throughout the orchestration. That preserves a later Windows/remote-client path without making it part of current acceptance.

Use ignored local secret files, exposed to containers as Compose secrets, for the database password and fixture account credentials. Commit only an example manifest of required secret filenames. Repository-owned process wrappers should read secret files inside the container and construct AzerothCore database environment variables immediately before `exec`, without shell tracing or output. MySQL should use its `_FILE` password input. This keeps plaintext out of committed Compose files, process arguments, and a rendered `docker compose config`; scripts must never print credentials or session material.

## Fixture provisioning contract

Provision the account through the pinned worldserver console's `account create`, `account set password`, and `account set addon ... 2` commands. This delegates AzerothCore's SRP6 verifier and salt construction to the authoritative binary instead of reproducing its database internals ([account creation](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/scripts/Commands/cs_account.cpp#L279-L325), [password update](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/scripts/Commands/cs_account.cpp#L868-L924)). Do not insert password verifier columns through hand-written SQL.

Character creation is not exposed as an equivalent console command. Commit one minimal, server-generated player-dump fixture and load it with the pinned worldserver's `pdump load` command, whose importer remaps the dump to the selected account/name/GUID using current schema knowledge ([player-dump command](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/scripts/Commands/cs_character.cpp#L791-L848)). The fixture should represent only the agreed level-one, ordinary-ground test character and include its generation image digest, source SHA, expected identity/location, and regeneration procedure. The bootstrap proof must generate or validate this fixture against the locked stack before it becomes an accepted repository asset.

Provisioning is idempotent and fail-closed:

1. Set or verify the `realmlist` row exactly.
2. Create the account if absent; otherwise set the current secret-derived password and verify account restrictions/addon level.
3. Load the player dump only if the fixture character is absent.
4. If a character with the expected name or GUID exists under a different account, or any protected fixture invariant has drifted, fail with a diagnostic rather than overwriting it.
5. Verify all postconditions before returning success.

The provisioning wrapper may feed commands to a temporary console-enabled worldserver through standard input or a FIFO, but it must not enable command echoing. The normal detached worldserver has `AC_CONSOLE_ENABLE=0`: its CLI treats standard-input termination as a shutdown request ([CLI loop](https://github.com/azerothcore/azerothcore-wotlk/blob/a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f/src/server/apps/worldserver/CommandLine/CliRunnable.cpp#L190-L239)), so relying on an unattached interactive console is not a stable service lifecycle.

## Readiness and diagnostics

`realm up` succeeds only after all three layers pass within a bounded timeout:

1. **Orchestration:** database reports healthy; `client-data`, `db-import`, and `fixture-provisioner` each completed with exit code zero.
2. **Processes:** authserver and worldserver report healthy after an in-container TCP connect to their own listening ports; host checks can connect to `127.0.0.1:3724` and `127.0.0.1:8085`.
3. **Semantic fixture:** required server-data directories and the locked version marker exist; the realm row matches ID/name/address/port/build; the fixture account has no TOTP or locks; exactly one expected character belongs to it; and the character is offline and in the agreed ordinary-ground state.

Health commands need stable nonzero exit statuses and concise, credential-free diagnostics identifying the failed layer and check. Container liveness alone is not readiness. Conversely, full login/world entry is not an environment health check; that protocol smoke test belongs to the bootstrap proof and later client integration.

## Lifecycle and reset contract

Repository commands should expose four deliberate operations:

- `realm up`: validate Docker/Compose and secret files, validate the artifact lock, pull the exact digests, populate/check server data, import databases, provision fixtures, start the two servers, and wait for readiness;
- `realm down`: stop and remove this project's containers and network while preserving state volumes;
- `realm reset-state`: after an interactive confirmation, or explicit `--yes`, remove only this Compose project's database/config state and rebuild/reprovision it while preserving the verified approximately 1.2 GB server-data cache;
- `realm reset-all`: perform the scoped state reset and also remove/re-download/re-verify the server-data volume.

Use a stable default Compose project name with an explicit override for worktrees. Before deletion, enumerate exact volume names and require the expected Compose project labels. Never use `docker system prune`, wildcard volume deletion, or an unresolved environment variable as a destructive target. An interrupted reset must be recoverable by rerunning `realm up`.

## Repository-owned deliverables

The implementation ticket should create an equivalent of this layout; exact script names may follow repository conventions discovered during implementation:

```text
infra/azerothcore/
  compose.yaml
  artifacts.lock
  .env.example
  README.md
  fixtures/
    reference-character.pdump
    PROVENANCE.md
  scripts/
    up
    down
    health
    provision
    reset
    verify-client-data
```

No file in this directory may contain AzerothCore source, a copied upstream Compose file, downloaded server data, generated runtime configuration, database state, or real credentials.

## Required bootstrap proof

The linked bootstrap task must record evidence for all of the following on the current Apple Silicon development machine:

- Docker Desktop can run every digest-pinned AzerothCore image under `linux/amd64` emulation;
- a clean `realm up` downloads and verifies the locked server-data archive, imports databases, provisions the realm/account/player-dump character, and reaches layered health;
- the build-12340 auth and world sockets are reachable and the provisioned credentials complete at least the existing minimal protocol smoke path;
- a second `realm up` is idempotent and does not duplicate or silently mutate the fixture;
- `down`/`up` preserves expected state;
- `reset-state` deletes only labeled state resources, keeps the verified data cache, and produces the same fixture invariant;
- `reset-all` also replaces and re-verifies server data;
- recorded timings distinguish first download/import, cached reset, and ordinary restart;
- effective image IDs/digests, Compose version, host architecture, fixture identifiers, and any emulation constraints are captured without secrets.

Only after that proof should later client tickets treat this environment as the Reference Realm. Windows remains a design-preserved path, not an acceptance platform for this slice.

## Consequences for later decisions

- The environment is reproducible by content lock and verified runtime inputs, not by vendoring AzerothCore.
- The Learning Client has no dependency on the server-data volume or Blizzard client files.
- Apple Silicon performance and compatibility remain empirical until the bootstrap task passes.
- Account cryptography and character schema stay owned by the pinned AzerothCore binary; this repository owns only secrets, orchestration, a minimal dump fixture, and invariant checks.
- Local reset is safe and scoped enough to become a routine integration-test prerequisite.
- A future remote Windows client changes advertised address and host firewall/TCP exposure, not the client protocol or core service topology.
