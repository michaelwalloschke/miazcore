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
- Asset access must be permission-gated. Private learning use, user-local data,
  untracked regenerable cache, and non-export are required exposure controls,
  but not an assumed grant to import or transform Blizzard content. No importer,
  cache, or rendered Blizzard content may run until **Establish asset-use
  authorization** is resolved. Public distribution, repackaging, or publishing
  of raw or derived Blizzard content is outside this map.
- The importer reads only the native archive layout. Pre-extracted or
  repackaged asset folders are unsupported in this effort.
- The first presentation target is one Entry-Anchor-aligned terrain patch in
  the existing Diagnostic World, not a general Azeroth recreation or a WoW
  client replacement.
- AzerothCore remains an external Reference Realm. Its realm version supplies
  compatibility context but does not authorize client-content distribution.

## Decisions so far

<!-- Resolved child tickets are linked here, one gist per ticket. -->

- [Research the build-12340 native archive layout](issues/02-research-build-12340-native-archive-layout.md) — Use the Standard Fixture's exact 13-archive build-12340/enUS SHA-256 manifest and fixed low-to-high precedence; reject custom overlays via the Overlay Fixture's `unsupported-custom-patch-set` case, while leaving provenance approval to its dedicated research ticket.
- [Research the private-use and provenance boundary](issues/01-research-private-use-and-provenance-boundary.md) — Public Blizzard terms do not affirmatively authorize an independent importer/cache, and neither local folder proves a usable license; private controls mitigate exposure but **Establish asset-use authorization** gates any asset-handling implementation.

## Not yet specified

- The smallest asset-specific subset beyond the fixed source-validation manifest
  remains open until the reader-toolchain and terrain-addressing research is
  complete.
- The exact permitted activity and cache scope can be specified only if the
  authorization task returns an affirmative, sufficiently specific result.
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
