# Map the loopback multi-client topology

Type: wayfinder:research
Status: open
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

