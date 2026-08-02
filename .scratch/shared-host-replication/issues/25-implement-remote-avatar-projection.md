# Implement Diagnostic World Remote Avatar projection

Type: implementation
Status: open
Blocked by: [Implement retained-session Remote Avatar truth](24-implement-remote-avatar-session-truth.md)

## Objective

Make the Diagnostic World visibly and safely project one Realm-replicated Avatar
from session truth, with readable identity, lifecycle, observed-versus-rendered
poses, and no feedback into networking.

## Entry gate

Ticket 24 is resolved with a stable public session vocabulary and
source-sequence fence, and existing offline/live local Diagnostic World tests
pass.

## Scope

- Add the private Remote Avatar presentation resource, ingress ordering,
  invalidation fence handling, one project-owned amber marker, heading, GUID
  shorthand, inspector rows, and redacted fault states.
- Implement deterministic smooth/snap/map-context projection according to the
  accepted threshold and local entry-map context.

## Out of scope

- Models/assets, a second marker, remote prediction, capture orchestration,
  manual proof controls, or Windows render acceptance.

## Acceptance

1. Created/hydrated state displays one marker at exact Realm-observed Pose;
   same-map small deltas smooth, boundary/large deltas snap, and heading takes
   the short arc.
2. Removed, Faulted, map-context unavailable, session failure/offline, and a
   fence advance clear marker and remote pose rows; valid matching-map recovery
   hydrates exact current truth.
3. An ingress batch larger than the visible event tail reaches projection in
   sequence; only the diagnostic tail truncates.
4. Local camera, local capsule, and all controlled-character pose truths are
   unchanged by remote input.
5. Pure/headless Bevy tests, the Windows compile tripwire, and workspace check
   pass.

## Required evidence

- Unit and headless schedule tests for lifecycle, smoothing/snap, map context,
  fence recovery, diagnostic redaction, and local-state isolation.
