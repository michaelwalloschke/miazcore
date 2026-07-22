# Design the engine-independent Learning Client architecture

Type: wayfinder:grilling
Status: claimed
Blocked by: [Trace the minimal AzerothCore world-entry protocol](01-trace-world-entry-protocol.md), [Choose the Learning Client engine direction](03-choose-engine-direction.md), [Prototype the diagnostic World-entry experience](04-prototype-diagnostic-world-entry.md), [Decide the minimal networked movement contract](06-decide-networked-movement-contract.md), [Define the minimum authoritative self-state boundary](10-define-minimum-authoritative-self-state-boundary.md), [Prove the Bevy shell and platform test path](11-prove-bevy-shell-platform-path.md)

## Question

How should the engine-free Rust protocol and session/domain crates, their bounded command/event boundary, ordered I/O ownership, movement and correction state, configuration and diagnostics, and the outer Bevy plugin/application crate be separated so that packet/crypto/session behavior remains deterministic and testable while Bevy owns only presentation, input, camera, interpolation, and diagnostic projection?
