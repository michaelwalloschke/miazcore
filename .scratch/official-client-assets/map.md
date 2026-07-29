# Chart Official WoW Client Asset Integration

Type: wayfinder:map
Status: open

## Destination

Reach a decision-complete integration specification for a private macOS Learning
Client that imports a user-supplied, native-archive World of Warcraft 3.3.5a
(build 12340) asset installation from another supported platform and renders a
small terrain patch aligned with the Reference Realm Entry Anchor.

The route is clear when the legal/provenance boundary, supported source layout,
toolchain, importer/cache design, terrain-coordinate contract, and acceptance
evidence are settled well enough to create implementation work without material
product, architecture, or compliance questions.

## Notes

- This map is planning-only. It resolves decisions and links research assets;
  it does not implement an importer or add Blizzard content to the repository.
- Consult `CONTEXT.md`, `docs/agents/domain.md`, `/grilling`, and
  `/domain-modeling` in every session. Use **User-Supplied Client Asset Set**
  for the source installation.
- The source is a legally obtained native 3.3.5a/build-12340 data installation
  that may be copied from another supported platform. The macOS Learning Client
  does not launch or depend on Blizzard's executable.
- Asset access is private learning use only: raw data and every derived cache
  remain user-local, untracked, non-exported, and regenerable. Public
  distribution, repackaging, or publishing of raw or derived Blizzard content
  is outside this map.
- The importer reads only the native archive layout. Pre-extracted or
  repackaged asset folders are unsupported in this effort.
- The first presentation target is one Entry-Anchor-aligned terrain patch in
  the existing Diagnostic World, not a general Azeroth recreation or a WoW
  client replacement.
- AzerothCore remains an external Reference Realm. Its realm version supplies
  compatibility context but does not authorize client-content distribution.

## Decisions so far

<!-- Resolved child tickets are linked here, one gist per ticket. -->

- [Research the build-12340 native archive layout](issues/02-research-build-12340-native-archive-layout.md) — Use the Chromie build-12340/enUS folder as the technically clean development fixture; validate an explicit standard MPQ stack, reject Thera's custom overlays, and leave provenance approval to its dedicated research ticket.

## Not yet specified

- The exact required archive order and the smallest asset-specific subset remain
  open until the reader-toolchain and terrain-addressing research is complete.
- The terrain patch's exact visual composition, collision treatment, and how it
  coexists with project-owned Diagnostic World markers depend on the importer
  and coordinate-contract conclusions.
- Any later model, UI, audio, WMO, or multi-zone capability is intentionally
  not sliced until the terrain vertical route is understood.

## Out of scope

- Running, distributing, emulating, patching, or reverse-engineering Blizzard's
  executable; making a runnable macOS WoW client.
- Downloading or sourcing game files for the user; accepting pre-extracted,
  repackaged, or unprovenanced assets.
- Committing, uploading, publishing, sharing, or otherwise redistributing raw
  Blizzard data or derived cache/output.
- A full Azeroth recreation, broad terrain streaming, player models, UI, audio,
  WMOs, gameplay systems, or World of Warcraft client parity.
- Changing the Reference Realm, vendoring AzerothCore, LAN/public exposure,
  multiplayer scope, Windows runtime acceptance, or implementation itself.
