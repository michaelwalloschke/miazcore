# Design the Remote Avatar session-event boundary

Type: wayfinder:grilling
Status: resolved
Blocked by: [Define the Realm-replicated Avatar contract](04-define-realm-replicated-avatar-contract.md), [Define the Remote Avatar fault boundary](08-define-remote-avatar-fault-boundary.md), [Define the minimal remote-player protocol decoder](13-define-minimal-remote-protocol-decoder.md)

## Question

How do retained World-session events publish remote lifecycle and pose evidence
to the presentation boundary while retaining bounded queues, redaction,
backpressure behaviour, and complete separation from local prediction and
Submitted Pose truth?

## Decision boundaries

Choose event ownership and snapshot shape without exposing transport,
credentials, generated protocol types, or ECS data through public APIs.

## Answer

[Remote Avatar session-event boundary](../research/14-remote-avatar-session-event-boundary.md)
defines the single-GUID lifecycle state machine, lossless bounded FIFO and
latest-snapshot publication, redaction, saturation failure behavior, and the
strict separation from local movement and Bevy state.
