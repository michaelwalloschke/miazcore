# Fixture Pair provisioning contract

Ticket: [02 – Design Fixture Pair provisioning](../issues/02-design-fixture-pair-provisioning.md)

## Decision

The Reference Realm has one deterministic Fixture Pair, isolated from the
existing single-client fixture and owned exclusively by a future
`infra/azerothcore/realm reset-state` implementation.

| Subject | Account and Character | Credential files | Fixture source | Start placement |
| --- | --- | --- | --- | --- |
| existing single-client fixture | existing `MIAZTEST` / `Miaztest` | existing `fixture-account`, `fixture-password` | existing `reference-character.pdump` | unchanged and outside the Fixture Pair |
| Fixture Pair A | `MIAZPAIRA` / `Miazpaira` | new ignored `fixture-pair-a-account`, `fixture-pair-a-password` | new provenance-checked `reference-pair-a-character.pdump` | canonical Fixture Pair Start Placement |
| Fixture Pair B | `MIAZPAIRB` / `Miazpairb` | new ignored `fixture-pair-b-account`, `fixture-pair-b-password` | new provenance-checked `reference-pair-b-character.pdump` | Pair A's map, `east = A.east + 3.0 m`, same north, height, and heading |

The existing single-client fixture remains unchanged and outside the Fixture
Pair. Both Pair members have separate accounts and separate private credential
files; no credential reaches a CLI argument, evidence artifact, or Git.

## Reset and provenance contract

- The future `realm reset-state` implementation must create or restore the
  existing single-client fixture and both Pair accounts/Characters from an
  empty state volume. Client processes and later orchestration scripts may only
  select an existing Fixture Profile; they never provision accounts or
  Characters.
- Each Pair Pdump follows the existing fixture provenance process: generate
  against an empty character database, export through the controlled
  Worldserver workflow, hash and review it, then prove it through
  `reset-state`. Hand-written character-table SQL is not an accepted placement
  path.
- Before either Pair Pdump is accepted, a local Placement Probe must prove that
  both Pair members reach `MovementReady` on the same map, with matching height
  and heading and the exact three-metre eastward relation. This establishes a
  safe Fixture Pair Start Placement, not remote-avatar visibility.
- The future reset health gate verifies both Pair account IDs are distinct from
  each other and from the existing fixture; exact names; map `0`; the fixed
  three-metre placement relation; `online=0`; and no transport attachment.
  Those checks establish fixture readiness only. They do not substitute for a
  client-observed remote Avatar or a replication proof.
- A future live gate must separately compare each Realm-observed Entry Anchor
  with its Fixture Pair Start Placement. Provisioning does not relabel stored
  placement data as an Entry Anchor.
- A duplicate Fixture Profile launch is invalid operationally. Ticket 01
  observed the first same-fixture session become `failed` while the second
  reached `movement-ready`; future orchestration must prevent that condition
  before opening a second session.

## Scope boundary

This decision creates no LAN exposure, no ad-hoc account path, and no
database-derived multiplayer success claim. It does not decide profile launch
syntax or public credential APIs (Ticket 05), World-Update visibility
(Ticket 03), Remote Avatar presentation (Ticket 07), or the eventual live
replication proof (Ticket 17).

## Verification required by the implementation ticket

1. A clean `realm reset-state` provisions the existing single-client fixture
   plus exactly the two Pair members and passes the paired health assertions.
2. The Placement Probe authenticates the two Pair profiles concurrently on
   loopback and proves `MovementReady`, distinct GUIDs, and the contractual
   start-placement relation.
3. Reset removes temporary or stale Pair state and re-establishes only the
   declared three fixtures.
4. Remote-avatar visibility is proven separately by Ticket 03 rather than by
   a database or placement assertion.
5. All generated logs and evidence remain credential-free.
