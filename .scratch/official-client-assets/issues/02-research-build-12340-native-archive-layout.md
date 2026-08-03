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

Use the **Standard Fixture** as the technically usable build-12340/enUS
development fixture: its normal archive stack is byte-identical on the checked
archives to the standard portion of the **Overlay Fixture** and carries no extra
top-level custom patch archives. The linked assessment now defines the required
thirteen-archive low-to-high precedence manifest, exact byte sizes and SHA-256
identifiers, MPQ header/table-index checks, optional build evidence, and the
full fail-closed rules. The importer treats the Windows executable as build
evidence only and ignores runtime/configuration files.

The **Overlay Fixture** is not a first-choice source because its six extra
uppercase patch archives can override content; retain it only as a negative
fixture that proves the validator returns `unsupported-custom-patch-set`. This
is technical format evidence only, not proof of legal provenance.
