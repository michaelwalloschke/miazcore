# Reference Realm

This original, repository-owned Compose stack runs the external AzerothCore build locked in `artifacts.lock`. It does not vendor AzerothCore source, runtime configuration, databases, server data, or credentials.

Prerequisites on Apple Silicon are Docker Desktop using Apple Virtualization Framework with Rosetta enabled, Docker Compose, about 20 GB of free disk, and outbound access to Docker Hub and GitHub Releases.

```sh
infra/azerothcore/realm init-secrets
infra/azerothcore/realm up
infra/azerothcore/realm health
infra/azerothcore/realm smoke
infra/azerothcore/realm down
```

`reset-state` removes only the Compose-labeled database volume and keeps the verified server-data cache. `reset-all` also removes that cache. Both prompt unless passed `--yes`; both enumerate and re-check exact labels before deletion.

Only `127.0.0.1:3724` and `127.0.0.1:8085` are published. Database traffic and the approximately 1.2 GB compressed server-data asset remain internal. Override the advertised realm address with `MIAZCORE_REALM_ADDRESS` in an ignored `.env` only when deliberately testing a later remote-client path.

`smoke` performs a real build-12340 SRP6 login, authenticated realm discovery, world-session authentication, and exact single-character enumeration without printing credentials or session material. `tools/bootstrap_character.py --create` is reserved for regenerating the player-dump fixture against an empty character database.
