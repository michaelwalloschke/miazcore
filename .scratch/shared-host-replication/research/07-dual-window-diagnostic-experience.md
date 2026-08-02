# Dual-window Diagnostic World experience

Ticket: [07 – Prototype the dual-window Diagnostic World experience](../issues/07-prototype-dual-window-diagnostic-experience.md)

## Decision

The first Shared-Host Multi-client Simulation is observed through two
independent native Diagnostic World windows: one Learning Client process for
Fixture Profile A and one for Fixture Profile B. It is not a split view,
multi-viewport feature, or shared application process. Each window presents
its controlled local Character and, when Realm evidence exists, exactly one
Realm-replicated Avatar Marker.

The two windows use the same viewport-first cockpit grammar. A human can place
them side by side and identify both role direction and evidence source without
reading an account value, opening logs, or comparing a database row.

```text
┌─ PAIR A · LOCAL Miazpaira · LOOPBACK ─────────────────────────────────────┐
│  Diagnostic World viewport                  REMOTE AVATAR                 │
│                                                   GUID 0x…2                │
│       local capsule + heading                lifecycle  PRESENT            │
│                                                  observed  map / x y z / o  │
│       remote marker + heading ───────────►     rendered  map / x y z / o  │
│                                                                          │
├─ Recent semantic events ─────────────────────────────────────────────────┤
│  #42 remote Created 0x…2    #43 remote Updated    #44 remote Removed     │
└──────────────────────────────────────────────────────────────────────────┘
```

The Pair B window is symmetric: its local Character is B and its Remote Avatar
Marker identifies A's GUID. Profile tokens and configured Character names are
allowed sanitized role labels; Realm GUID is the only Remote Avatar identity.

## Minimal project-owned visual language

| Subject | Viewport representation | Inspector language |
| --- | --- | --- |
| Local Character | solid cool-cyan capsule with a short forward heading arrow and `LOCAL` ground ring | `LOCAL CHARACTER`; the existing controlled-character truth panel remains distinct from remote data |
| Remote Avatar Marker | warm-amber, faceted diamond-on-pedestal with a separate forward heading arrow and `REMOTE 0x…` ground label | `REMOTE AVATAR`; GUID shorthand, lifecycle, Realm-observed Pose, Rendered Pose |
| Absent Remote Avatar | no placeholder remains in the world | `REMOTE AVATAR / ABSENT` with no pose values |
| Remote Avatar fault | no stale marker remains | compact red `REMOTE AVATAR FAULT` state with redacted category/context only |

Colour is never the sole distinction: local uses a capsule and ring, remote
uses a faceted marker and label. Both heading arrows originate at their own
subject, so heading is visibly independent of camera orientation. The marker,
arrows, grid, labels, and inspector are project-owned primitives; no WoW
models, terrain, display IDs, NPCs, portraits, inventory, or gameplay UI enter
the prototype.

## Pose and lifecycle presentation contract

- `Realm-observed Pose` displays the exact latest Remote Avatar pose supplied
  by a `Created` or `Updated` session event. It is labelled `OBSERVED`, never
  `server-confirmed`, `actual`, or `rendered`.
- `Rendered Pose` displays only the Bevy projection's current marker pose. It
  may temporarily differ while smoothing, and it is never copied back into the
  session Snapshot or an evidence sidecar as Remote Avatar truth.
- Both pose rows show map, east, north, elevation, and orientation with the
  same precision. A same-map snap is visibly annotated as `PROJECTION SNAP`.
  A map mismatch instead hides the marker and both pose rows, visibly reports
  `PROJECTION SNAP / MAP CONTEXT UNAVAILABLE`, and never interpolates or
  implies remote prediction. Ticket 15 owns the exact mechanics.
- Lifecycle is `ABSENT`, `PRESENT`, or `FAULT`. `Created` establishes the
  marker at the observed pose; `Updated` moves its projection; `Removed`
  despawns it and clears both pose rows. A valid second GUID is shown only as a
  bounded capacity diagnostic, never as a second marker or replacement.
- The semantic-event strip retains a bounded chronological tail of sequence,
  `Created`/`Updated`/`Removed`, GUID shorthand, and redacted fault/category.
  It has no packet opcode, account, credential, path, session key, or payload
  content.

## Two-window operator experience

1. Start the two independent Pair Profile clients through the later
   orchestrator; confirm each title's profile token and sanitized local
   Character identity differ.
2. Place windows side by side. Confirm each has one cyan local capsule and,
   after a remote `Created`, one amber Remote Avatar Marker with a displayed
   GUID shorthand and `PRESENT` lifecycle.
3. During a serial role turn, observe the moving client's local capsule and the
   other window's amber marker. In the observer, `OBSERVED` updates first and
   `RENDERED` may visibly converge toward it; neither window labels a rendered
   value as observation.
4. After clean logout, confirm the observing window's `Removed` event, marker
   disappearance, and `ABSENT` inspector state. A fault must instead show the
   redacted fault panel with no stale marker.

This is a short manual extension for human understanding. It does not replace
the later per-client semantic sidecars, controlled dual-window capture, or
Role-reversed Replication Proof.

## Explicit deferrals

- Production Bevy entities, Remote Avatar decoder/session event wiring,
  smoothing constants, capture composition, and scripted test automation.
- More than one displayed Remote Avatar, name-query display names, chat,
  combat, terrain, collision, authored UI, and character models.
- A manual success claim, Windows runtime acceptance, or LAN exposure.

## Verification required by later implementation

1. A presentation test proves a `Created` event makes one remote marker at the
   exact observed pose; `Updated` changes only its projection; `Removed` clears
   both marker and pose rows.
2. Inspector tests prove `OBSERVED` and `RENDERED` labels/values remain
   distinct, including a smoothing and snap state, and local motion cannot
   change remote truth.
3. Visual regression/capture tests prove each client has one local and at most
   one remote primitive with non-colour-only distinction, GUID shorthand, and
   redacted fault state.
4. The role-reversed live proof retains the later automated evidence; the short
   two-window checklist remains an additional human observation only.
