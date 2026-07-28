# Trace the remote-player World-Update boundary

Type: wayfinder:research
Status: open
Blocked by: [Measure Reference Realm multi-session behavior](01-measure-reference-realm-multi-session-behavior.md)

## Question

Which build-12340 World-Update records from the local Reference Realm establish
a remote player's GUID identity, display metadata, map/pose/heading updates,
and clean removal, and which decoder boundaries can safely ignore unrelated
objects without disturbing encrypted framing?

## Answer criteria

Produce a project-owned, provenance-recorded transcript analysis and a precise
minimal decoding contract. Keep raw authenticated traffic out of repository
artifacts and defer arbitrary object, NPC, combat, and teleport support.

