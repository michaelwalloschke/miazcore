# Define the Remote Avatar fault boundary

Type: wayfinder:grilling
Status: resolved
Blocked by: [Trace the remote-player World-Update boundary](03-trace-remote-player-world-update-boundary.md), [Define the Realm-replicated Avatar contract](04-define-realm-replicated-avatar-contract.md)

## Question

Which malformed, partial, unsupported, or inconsistent remote-player updates
remove only the affected marker with a redacted diagnostic, and which framing
or cipher-integrity failures must fail-close the entire World session?

## Decision boundaries

No guessed poses, stale silent markers, or cipher drift recovery. Scope only
the Remote Avatar boundary; peer crash/reconnect policy remains deferred.

## Answer

[Remote Avatar fault boundary](../research/08-remote-avatar-fault-boundary.md)
defines the fail-closed World-session integrity cases, the marker-only
redacted-fault cases, and the valid-but-ignored records that preserve stream
alignment without mutating a Remote Avatar.

## Comments

- 2026-08-01: Decision accepted: framing/cipher and unwalkable-container
  failures stop the World session; semantically unusable data for the accepted
  Remote Avatar removes only that marker with redacted diagnostics.
