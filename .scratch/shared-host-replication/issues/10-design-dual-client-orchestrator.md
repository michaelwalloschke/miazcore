# Design the Dual-Client Orchestrator

Type: wayfinder:grilling
Status: open
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

