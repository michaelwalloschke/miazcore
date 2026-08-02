# Specify the replication evidence contract

Type: wayfinder:grilling
Status: resolved
Blocked by: [Prototype the dual-window Diagnostic World experience](07-prototype-dual-window-diagnostic-experience.md), [Define the Remote Avatar fault boundary](08-define-remote-avatar-fault-boundary.md), [Measure replication timeline and tolerances](09-measure-replication-timeline-and-tolerances.md), [Design the Dual-Client Orchestrator](10-design-dual-client-orchestrator.md)

## Question

What exact machine and manual evidence proves the Role-reversed Replication
Proof: per-client sidecars, GUID and lifecycle assertions, raw-versus-rendered
remote pose provenance, controlled dual-window capture, redaction, hashes, and
the human-visible extension to the existing macOS checklist?

## Decision boundaries

Define the acceptance schema and fail-closed validation without changing the
completed World-entry Acceptance Bundle or treating screenshots as sufficient
semantic proof.

## Answer

[Role-reversed Replication Evidence Contract](../research/11-replication-evidence-contract.md)
defines the independent hash-bound bundle, exact Pair sidecars, serial
GUID/lifecycle/pose assertions, capture and manual evidence, fail-closed
validator behavior, and explicit scope boundary.
