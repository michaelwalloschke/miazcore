# Define the reproducible Reference Realm environment

Type: wayfinder:research
Status: resolved

## Question

Which pinned AzerothCore Docker artifacts and repository-owned orchestration, configuration, health checks, reset flow, and test-account provisioning are needed to start a reproducible Reference Realm without including AzerothCore source code in this repository?

## Answer

Use an original, repository-owned six-service Compose topology—MySQL, verified server-data acquisition, database import, fixture provisioning, authserver, and worldserver—rather than copying AzerothCore's source-tree or reusable Compose files. Lock all four AzerothCore `linux/amd64` images to platform-manifest digests from one successful source build, lock MySQL by multi-platform index digest, and independently verify the runtime server-data archive before extracting it into a Docker volume. Provision credentials through ignored secrets, create the account through the pinned worldserver CLI, load one provenance-recorded player-dump character idempotently, publish only localhost auth/world ports, require layered semantic health, and reset only resources bearing the expected Compose project labels. The exact artifacts, topology, configuration, fixture, lifecycle, licensing boundary, and Apple Silicon bootstrap proof are recorded in [Reproducible Reference Realm environment](../research/reproducible-reference-realm-environment.md).
