# Build-12340 source-layout assessment

## Scope and method

This assessment inspected the two folders supplied by the user, read filesystem
metadata and archive headers, compared selected core archives byte-for-byte, and
examined the Windows executable only for embedded build identification. It did
not launch, copy, import, modify, or upload any client file.

## Candidate findings

| Candidate | Build evidence | Archive layout | Result |
| --- | --- | --- | --- |
| `~/Downloads/ChromieWowClient` | `Wow.exe` embeds `World of WarCraft (build 12340)` and `3.3.5` | Standard MPQ family: `common`, `common-2`, `expansion`, `lichking`, `patch`, `patch-2`, `patch-3`, plus the normal `enUS` locale/speech archives | **Technically usable baseline** |
| `~/Downloads/TheraWowClient` | The same build-12340 executable (identical MD5 to Chromie) | Contains the same standard stack plus six extra uppercase archives: `Patch-F`, `Patch-G`, `Patch-H`, `Patch-S`, `Patch-T`, and `Patch-X` | **Not a first-choice source**; custom overlay content must not enter the first importer |

The selected standard archives tested byte-identical across both folders:
`common.MPQ`, `common-2.MPQ`, `expansion.MPQ`, `lichking.MPQ`, `patch.MPQ`,
`patch-2.MPQ`, `patch-3.MPQ`, `enUS/locale-enUS.MPQ`, and
`enUS/patch-enUS-3.MPQ`. Their MPQ headers begin with `MPQ\x1a`, as expected.

Both folders contain a private-server `realmlist.wtf`; Thera also has matching
Thera-specific runtime configuration. Neither file is relevant to archive
reading and neither is an input the importer should read. The 32-bit Windows
`Wow.exe` is build-identification evidence only; the macOS Learning Client must
not invoke it.

## Decision

Use `~/Downloads/ChromieWowClient` as the development fixture for the next
research and prototype work. The eventual importer must accept only the native
MPQ layout, ignore executable/configuration/add-on/runtime folders, and reject
unexpected top-level patch archives until a later explicit compatibility
decision. It should require the build-12340 evidence and an `enUS` locale stack
for this first route.

`~/Downloads/TheraWowClient` is useful as a negative-validation fixture: its
extra `Patch-*.MPQ` overlays must cause a clear "unsupported custom patch set"
result, not silently alter the imported terrain.

## Provenance limitation

This assessment establishes technical format compatibility, not legal origin or
redistribution rights. Folder names and embedded Blizzard copyright material do
not prove that either copy satisfies the map's **User-Supplied Client Asset
Set** provenance requirement. The separate private-use/provenance research must
define the user attestation and local-only controls before either fixture is
treated as an approved long-term source.

## Follow-on facts

- A first importer may use a source-root selector pointing at
  `ChromieWowClient`, but must not copy source content into the repository.
- The source validator needs an explicit archive allowlist/order rather than a
  broad "all MPQs" glob.
- `TheraWowClient` demonstrates why the validator must detect unexpected
  overlays before cache generation.
