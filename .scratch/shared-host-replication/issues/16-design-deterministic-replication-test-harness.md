# Design the deterministic replication test harness

Type: wayfinder:grilling
Status: open
Blocked by: [Measure replication timeline and tolerances](09-measure-replication-timeline-and-tolerances.md), [Define the minimal remote-player protocol decoder](13-define-minimal-remote-protocol-decoder.md), [Design the Remote Avatar session-event boundary](14-design-remote-session-event-boundary.md)

## Question

What fake-clock, fragmented/coalesced encrypted-frame, and scripted-session
fixtures prove Remote Avatar create/update/destroy, GUID isolation, smoothing,
snap, malformed-record handling, and observer-only evidence without Docker,
Bevy rendering, or wall-clock tolerance?

## Decision boundaries

Define deterministic coverage only. The harness is not an emulator of arbitrary
multiplayer gameplay and cannot claim live Realm replication by itself.

