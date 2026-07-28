# Design the Remote Avatar presentation boundary

Type: wayfinder:grilling
Status: open
Blocked by: [Define the Realm-replicated Avatar contract](04-define-realm-replicated-avatar-contract.md), [Prototype the dual-window Diagnostic World experience](07-prototype-dual-window-diagnostic-experience.md), [Design the Remote Avatar session-event boundary](14-design-remote-session-event-boundary.md)

## Question

How does the Bevy-only presentation map a GUID-keyed Remote Avatar Marker to a
placeholder, label, heading, inspector, smoothing, snaps, and removal while
keeping the existing controlled-character Diagnostic World semantics intact?

## Decision boundaries

Specify one accepted remote marker per client and its visible failure state.
Defer models, terrain, generic player lists, animation, and rendering claims on
Windows.

