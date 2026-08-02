# Design the Role-reversed live proof

Type: wayfinder:grilling
Status: resolved
Blocked by: [Design the Dual-Client Orchestrator](10-design-dual-client-orchestrator.md), [Specify the replication evidence contract](11-specify-replication-evidence-contract.md), [Design the paired Fixture reset task](12-design-paired-fixture-reset-task.md), [Measure replication timeline and tolerances](09-measure-replication-timeline-and-tolerances.md), [Design the Remote Avatar presentation boundary](15-design-remote-avatar-presentation-boundary.md), [Design the deterministic replication test harness](16-design-deterministic-replication-test-harness.md)

## Question

What reset-scoped live script contract proves serial A-to-B then B-to-A
Replicated Moves, Remote Avatar create/update/remove, independent sessions,
dual-window capture, redacted sidecars, cleanup, and Realm health—without
repeating the prior saving-reconnect proof?

## Decision boundaries

Specify one no-retry gate and explicit fail-closed outcomes. Defer peer crashes,
reconnect convenience, LAN, and multi-peer scaling.

## Answer

[Role-reversed live proof](../research/17-role-reversed-live-proof.md) defines
the reset-scoped no-retry machine gate, serial role turns and measured
observer-only assertions, two-window capture, cleanup/recovery, and the
pending-manual boundary for Ticket 18.
