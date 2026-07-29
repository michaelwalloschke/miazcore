# Map the loopback multi-client topology

Type: wayfinder:research
Status: resolved
Blocked by: [Measure Reference Realm multi-session behavior](01-measure-reference-realm-multi-session-behavior.md)

## Question

What exact local process, port, lock, reset, and cleanup topology lets two
macOS clients independently reach the same Docker-hosted Reference Realm on
`127.0.0.1` without changing advertised addresses, firewall rules, or LAN
exposure?

## Answer criteria

Produce a redacted topology note that identifies exclusive resources and safe
orchestration ownership. It must make accidental concurrent realm resets or
shared-client state observable and fail closed.

## Answer

[Loopback multi-client topology](../research/06-loopback-multi-client-topology.md)
defines one canonical Docker Realm, two independent loopback client children,
the shared Realm lock, scoped reset ownership, and recovery that blocks unsafe
concurrent runs.

## Comments

- 2026-07-29: The existing atomic Realm-test lock is adopted as the one shared
  lock; pair-specific operations must extend it rather than create a second
  lock namespace.
