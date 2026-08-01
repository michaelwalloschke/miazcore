# Design the Dual-Client Orchestrator

Type: wayfinder:grilling
Status: resolved
Blocked by: [Decide the Fixture Profile and secret contract](05-decide-fixture-profile-and-secret-contract.md), [Map the loopback multi-client topology](06-map-loopback-multi-client-topology.md)

## Question

How does one repository-owned Dual-Client Orchestrator launch both Fixture
Profiles, await independent readiness, coordinate serial role turns, preserve
per-client evidence, and always cleanly stop both processes without hiding a
failure?

## Decision boundaries

Choose process ownership, readiness markers, lock discipline, timeout/failure
behaviour, and evidence paths. It must not use fake peers, automatic reruns, or
a shared client process.

## Answer

[Dual-Client Orchestrator](../research/10-dual-client-orchestrator.md) defines
one foreground parent with exact Pair Profile admission, atomic sidecars,
serial role turns, bounded cleanup, and retained failure recovery.

## Comments

- 2026-08-01: Decision accepted: a synchronous repository-owned shell parent
  owns the canonical Realm lock and exactly two direct Pair Profile children;
  it has no fake peer, automatic rerun, daemon, or shared client process.
