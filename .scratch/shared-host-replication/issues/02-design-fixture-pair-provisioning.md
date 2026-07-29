# Design Fixture Pair provisioning

Type: wayfinder:grilling
Status: resolved
Blocked by: [Measure Reference Realm multi-session behavior](01-measure-reference-realm-multi-session-behavior.md)

## Question

How should the Reference Realm deterministically provision and reset two
separately authenticated Fixture Pair members so they can log in concurrently
on the same map, begin directly visible at a safe short distance, and remain
isolated from the existing single-client fixture?

## Decision boundaries

Choose account/Character naming, reset ownership, placement contract, and the
minimum test-only data changes. Do not introduce LAN exposure, ad hoc accounts,
or database-derived multiplayer success.

## Answer

[Fixture Pair provisioning contract](../research/02-fixture-pair-provisioning.md)
locks an isolated two-member Pair, its separate secret-file ownership, the
fixed three-metre eastward Fixture Pair Start Placement, reset responsibility,
and the paired health boundary. It preserves profile syntax and replication
evidence for their dependent tickets.
