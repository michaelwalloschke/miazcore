# Build-12340 source-layout assessment

## Scope and method

This assessment inspected two user-supplied local fixtures, read filesystem
metadata and archive headers, compared selected core archives byte-for-byte, and
examined the Windows executable only for embedded build identification. It did
not launch, copy, import, modify, or upload any client file.

The symbolic fixture identifiers below are deliberately repository-safe:

- **Standard Fixture** — the candidate with the normal build-12340/enUS archive
  set.
- **Overlay Fixture** — the candidate with the same standard set plus custom
  top-level patch overlays.

Inspection tooling: macOS 26.5.2; `/usr/bin/shasum` 6.02 with SHA-256;
`/usr/bin/cmp` for byte equality; `/usr/bin/file` 5.41; and macOS `od` and
`strings` for the MPQ magic and embedded build string. SHA-256 is the integrity
identifier in this record. A previous MD5 comparison of the executables was
diagnostic only and is not used for validation.

## Candidate findings

| Fixture | Build evidence | Archive layout | Result |
| --- | --- | --- | --- |
| **Standard Fixture** | `Wow.exe` embeds `World of WarCraft (build 12340)` and `3.3.5` | Standard MPQ family: `common`, `common-2`, `expansion`, `lichking`, `patch`, `patch-2`, `patch-3`, plus the normal `enUS` locale/speech archives | **Technically usable baseline** |
| **Overlay Fixture** | Same build-12340 executable SHA-256 as Standard Fixture | Contains the same standard stack plus six extra uppercase archives: `Patch-F`, `Patch-G`, `Patch-H`, `Patch-S`, `Patch-T`, and `Patch-X` | **Reject**; custom overlay content must not enter the first importer |

The selected standard archives tested byte-identical across both folders:
`common.MPQ`, `common-2.MPQ`, `expansion.MPQ`, `lichking.MPQ`, `patch.MPQ`,
`patch-2.MPQ`, `patch-3.MPQ`, `enUS/locale-enUS.MPQ`, and
`enUS/patch-enUS-3.MPQ`. Their MPQ headers begin with `MPQ\x1a`, as expected.

Both fixtures contain a private-server `realmlist.wtf`; the Overlay Fixture also
has matching server-specific runtime configuration. Neither file is relevant to
archive reading and neither is an input the importer should read. The 32-bit
Windows `Wow.exe` is build-identification evidence only; the macOS Learning
Client must not invoke it.

## Redacted Standard Fixture manifest

This is the complete required input manifest for the first terrain route.
`Lookup precedence` is low to high: the importer opens the archives in this
order and resolves a duplicate pathname from the highest listed archive. The
manifest's relative paths intentionally omit all workstation paths and source
identity details.

| Lookup precedence | Relative path | Bytes | SHA-256 |
| ---: | --- | ---: | --- |
| 1 | `Data/common.MPQ` | 2,881,154,862 | `d850ffa5efd6a1ba845899a7d9f9ad27f6eea7ac606e95033506c72c86ee0236` |
| 2 | `Data/common-2.MPQ` | 1,810,430,636 | `9c605d95443172bdfd7c6936bc636684761241657935c22da25f955083809f43` |
| 3 | `Data/expansion.MPQ` | 1,921,219,911 | `34a19723d773997b31ec72a16361af8bdd8cc38d108274b9b8a96521ec2c4f4a` |
| 4 | `Data/lichking.MPQ` | 2,553,948,549 | `8456e92d47f2bc71efba30fcabd4fea8c5ef0d93feef320ba51ffeeacf78de9d` |
| 5 | `Data/patch.MPQ` | 4,004,713,057 | `92b4a94a6c7a23c0b9fd88c47823e41792879a7ee1f70c2349a255153ca25d54` |
| 6 | `Data/patch-2.MPQ` | 1,401,729,059 | `c8b78bb75bcf5773e9ae99e11bdfcabc2a37aed3da947367d8af808d4d3c23c6` |
| 7 | `Data/patch-3.MPQ` | 605,089,137 | `56dbbfc8f9ce7182ca88538d75284e738020138a7ca4217d72d8168865510dd3` |
| 8 | `Data/enUS/locale-enUS.MPQ` | 204,291,376 | `45f02a3bf3964b169f397cea58113cdce5fa7bdf58d4b63f68e46255bb3bd3ea` |
| 9 | `Data/enUS/expansion-locale-enUS.MPQ` | 17,389,181 | `c9f27ce716b7195776929e8e3a7faeeaa15522215be47611c101dd5b1d8845c1` |
| 10 | `Data/enUS/lichking-locale-enUS.MPQ` | 12,354,378 | `89b1eba015927e321683a3867be1b8a76e593c438527eb42f4934d03484f1b04` |
| 11 | `Data/enUS/patch-enUS.MPQ` | 296,616,080 | `c1c06b0d0c34c331b21df7cdd25cc422a650efbd477e42b80849ba30b2e5fc99` |
| 12 | `Data/enUS/patch-enUS-2.MPQ` | 225,570,171 | `ab9af8457f0d7b99a562f26b7d1651bbf68ebc37ddb249d01ac860ccc8746ff4` |
| 13 | `Data/enUS/patch-enUS-3.MPQ` | 100,373,935 | `d61a60297af9044d926754d997cd5aa630500cd003d41983a9ccb5b324d61299` |

Optional build evidence, not a runtime input: `Wow.exe` is 7,704,216 bytes,
has SHA-256 `aa63a5750d60ef16746c686b3d5e26876d98953eab08b1c026cd0faf78e88cb8`,
and contains the build-12340 string. A future macOS-only source may omit the
executable only when it presents a separately approved archive manifest.

## Deterministic source-validator contract

The source validator must:

1. Canonicalize the user-selected source root; refuse symlinks that escape it,
   non-regular archive files, missing required files, duplicate case-folded
   required paths, or any path traversal.
2. Require exactly the thirteen manifest paths above, their stated byte sizes,
   SHA-256 digests, and `MPQ\x1a` magic at byte offset zero. The supported first
   route is exactly build-12340/enUS, not a best-effort version family.
3. Open every required archive with a bounds-checking MPQ reader and reject a
   malformed header, invalid archive/table offsets, hash-table or block-table
   range outside the file, integer overflow, or a table entry that points beyond
   its archive. The reader must treat a missing internal `listfile` as normal;
   asset names are supplied by the version-pinned importer mapping, never by a
   broad archive enumeration.
4. Build the resolver from the listed low-to-high precedence order, never from
   directory enumeration. Locale speech and backup MPQs may remain on disk but
   are not resolver inputs for this terrain route.
5. Reject any additional top-level `Data/*.MPQ` or additional locale patch
   archive matching `Data/enUS/patch-enUS*.MPQ`; in particular, the Overlay
   Fixture's `Patch-F`, `Patch-G`, `Patch-H`, `Patch-S`, `Patch-T`, and
   `Patch-X` are an `unsupported-custom-patch-set` error. Ignore no archive that
   could change lookup precedence.

## Decision

Use the **Standard Fixture** as the development fixture for the next research
and prototype work. The eventual importer must accept only the manifest-defined
native MPQ layout, ignore executable/configuration/add-on/runtime folders, and
reject unexpected top-level patch archives until a later explicit compatibility
decision. It requires the build-12340 evidence and `enUS` locale stack for this
first route.

The **Overlay Fixture** is useful as a negative-validation fixture: its extra
`Patch-*.MPQ` overlays must cause a clear `unsupported-custom-patch-set` result,
not silently alter the imported terrain.

## Provenance limitation

This assessment establishes technical format compatibility, not legal origin or
redistribution rights. Folder names and embedded Blizzard copyright material do
not prove that either copy satisfies the map's **User-Supplied Client Asset
Set** provenance requirement. The separate private-use/provenance research must
define the user attestation and local-only controls before either fixture is
treated as an approved long-term source.

## Follow-on facts

- A first importer may select a local **Standard Fixture**-conformant source
  root, but must not copy source content into the repository.
- The manifest and validator contract above, rather than a broad "all MPQs"
  glob, are the authoritative source selection rule.
- The **Overlay Fixture** proves the validator detects unexpected overlays
  before cache generation.
