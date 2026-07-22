# Reference character fixture provenance

`reference-character.pdump` was generated on 2026-07-22 by creating `Miaztest` through the real build-12340 login/world protocol and exporting it with the digest-pinned worldserver's `pdump write` command. It was not synthesized from the character schema.

| Field | Value |
| --- | --- |
| AzerothCore source | `a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f` |
| Worldserver platform manifest | `sha256:0a601595920e19c4af10679e4c01ac10f60569fc1e737db54aa6a5a07efb2455` (`linux/amd64`) |
| Dump SHA-256 | `030fcd8c563eedc14cc5bc2929427489178b910cc78f25e064198db1d7ea1e32` |
| Dump size | `11,172` bytes |
| Identity | GUID `1`, account ID `1`, human warrior, male, level `1` |
| Entry Anchor | map `0`, zone `0`, `(-8949.95, -132.493, 83.5312)`, orientation `0` |
| Ordinary-ground checks | offline; transport GUID and transport coordinates/orientation all zero |

Regeneration starts from an empty character database, brings up the pinned auth/world services with the fixture account, runs `tools/bootstrap_character.py --create`, and exports `Miaztest` through `provision.sh export`. The resulting dump must be copied from the stopped export container, hashed, reviewed for the same invariant, and then proven by `reset-state` before replacing this file. Hand-written character-table SQL is not an accepted generation path.
