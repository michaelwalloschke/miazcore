# Prove the Reference Realm bootstrap

Type: wayfinder:task
Status: resolved
Blocked by: [Define the reproducible Reference Realm environment](02-define-reference-realm-environment.md)

## Question

Can the digest-pinned minimal Reference Realm run on the current Apple Silicon development machine under `linux/amd64` emulation, verify its locked server-data archive, import its databases, idempotently provision the exact realm/account/player-dump character invariant, pass layered health and a build-12340 protocol smoke, preserve state across restart, and reproduce the invariant after label-scoped `reset-state` and `reset-all`—and what commands, timings, digests, identifiers, or emulation constraints must later decisions rely on?

## Answer

Yes, with Docker Desktop using Apple Virtualization Framework plus Rosetta: the original six-service Compose stack now consumes only exact image digests, verifies the separately locked `v20.0/Data.zip` before extraction, imports all three databases, provisions a secret-derived account and server-generated player-dump character idempotently, exposes only localhost auth/world ports, passes semantic health plus real build-12340 login/world authentication and character enumeration, preserves state across `down`/`up`, and safely reproduces the invariant through label-checked `reset-state` and `reset-all`. QEMU without Rosetta crashes this pinned worldserver binary and is not a supported fallback for the macOS slice. Exact artifacts, identifiers, commands, timings, fixture provenance, reset evidence, and remaining constraints are recorded in [Reference Realm bootstrap proof](../research/reference-realm-bootstrap-proof.md); the runnable environment is documented under `infra/azerothcore/`.

## Comments

### Bootstrap checkpoint — 2026-07-22

- All image references and exact digests from Ticket 02 resolve in their registries. The AzerothCore service images are `linux/amd64`; the pinned MySQL image includes a native `linux/arm64` manifest.
- Docker's daemon-side pull path is currently blocked resolving its configured `http.docker.internal:3128` proxy. As a non-mutating diagnostic workaround, the pinned world image was fetched host-side with verified `crane` v0.21.7 and imported into Docker.
- The imported image reports `linux/amd64`; its shell executes successfully as `x86_64`, and the upstream entrypoint reaches `Starting worldserver...` with writable configuration paths.
- The pinned `worldserver` executable then terminates immediately with `qemu: uncaught target signal 11 (Segmentation fault) - core dumped`, including for `worldserver --version`. This occurs before configuration parsing, database access, or server-data loading, so it blocks every later bootstrap acceptance check.
- Docker Desktop is currently configured with both `UseVirtualizationFramework: false` and `UseVirtualizationFrameworkRosetta: false`. Docker documents QEMU execution of Intel containers on Apple Silicon as best-effort and potentially crash-prone; its supported Rosetta path requires the Apple Virtualization framework.
- Fifteen unrelated containers are running on the shared Docker daemon. Switching the virtual-machine backend and enabling Rosetta requires restarting Docker Desktop, so no host setting or running container was changed without explicit approval.

### Resolution — 2026-07-22

- After the user enabled Apple Virtualization Framework and Rosetta and restarted Docker Desktop, the exact pinned `worldserver --version` probe passed.
- Final-code `reset-all --yes` passed in 178.96 seconds, `reset-state --yes` passed with the data volume preserved, ordinary `up` passed idempotently, and `down`/`up` preserved the fixture.
- Final semantic health and real build-12340 login/world authentication plus exact `Miaztest` enumeration passed. Full evidence is linked in the Answer.
