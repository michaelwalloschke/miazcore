# Design the paired Fixture reset task

Type: wayfinder:task
Status: resolved
Blocked by: [Design Fixture Pair provisioning](02-design-fixture-pair-provisioning.md), [Map the loopback multi-client topology](06-map-loopback-multi-client-topology.md)

## Question

What repository and local-environment changes must be performed to make the
Fixture Pair resettable, isolated, inspectable, and safely usable by later
protocol experiments before an implementation decision can be verified?

## Answer criteria

Provide an exact AFK/HITL checklist with target resources, health assertions,
credential locations, and rollback/cleanup facts. This task unblocks decisions;
it must not deliver the full Shared-Host Multi-client Simulation.

## Answer

[Paired Fixture reset task](../research/12-paired-fixture-reset-task.md)
defines the Pair Pdump provenance checkpoint, exact reset targets and health
assertions, Placement Probe, redacted evidence, and human recovery boundary.

## Comments

- 2026-08-01: The accepted route is reset-owned Pair A/B provisioning from
  reviewed Pdump provenance, with a separate Placement Probe and no database
  claim of Remote Avatar replication.
