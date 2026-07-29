# Research the build-12340 native archive layout

Type: wayfinder:research
Status: resolved

## Question

Which on-disk files, locale data, archive indices, and integrity/version facts
must an importer discover and validate in a user-supplied native World of
Warcraft 3.3.5a/build-12340 installation copied from another supported
platform, and what must fail closed?

## Answer

The candidate assessment is recorded in
[Build-12340 source-layout assessment](../research/02-build-12340-source-layout.md).

Use `~/Downloads/ChromieWowClient` as the technically usable build-12340/enUS
development fixture: its normal archive stack is byte-identical on the checked
archives to the standard portion of the Thera copy and carries no extra
top-level custom patch archives. The importer must validate an explicit native
archive allowlist and order, treat the Windows executable as build evidence only,
ignore runtime/configuration files, and fail closed on unexpected overlays.

`~/Downloads/TheraWowClient` is not a first-choice source because its six extra
uppercase patch archives can override content; retain it only as a negative
fixture that proves the validator rejects custom overlay sets. This is technical
format evidence only, not proof of legal provenance.
