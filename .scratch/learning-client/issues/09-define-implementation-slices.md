# Define the implementation slices and scope gates

Type: wayfinder:grilling
Status: open
Blocked by: [Define the reproducible Reference Realm environment](02-define-reference-realm-environment.md), [Choose the Learning Client engine direction](03-choose-engine-direction.md), [Decide the minimal networked movement contract](06-decide-networked-movement-contract.md), [Design the engine-independent Learning Client architecture](07-design-client-architecture.md), [Specify the World-entry verification contract](08-specify-verification-contract.md), [Prove the Bevy shell and platform test path](11-prove-bevy-shell-platform-path.md)

## Question

In what order should the Reference Realm, protocol core, engine shell, Diagnostic World, movement loop, and verification layers be implemented, and what entry, exit, and explicit deferral conditions keep each slice independently learnable and testable?
