# Define the minimal remote-player protocol decoder

Type: wayfinder:grilling
Status: open
Blocked by: [Trace the remote-player World-Update boundary](03-trace-remote-player-world-update-boundary.md), [Define the Remote Avatar fault boundary](08-define-remote-avatar-fault-boundary.md), [Measure replication timeline and tolerances](09-measure-replication-timeline-and-tolerances.md)

## Question

What project-owned decoder and test-fixture boundary consumes only the remote
player records needed for GUID identity, display metadata, create/update/destroy
and pose/heading while preserving incremental encrypted-frame alignment across
ignored records?

## Decision boundaries

Define strict malformed handling and test provenance. Defer general object
decoding, NPCs, movement modes beyond the Fixture Pair, and unsupported update
fields.

