# Define the Shared-Host Replication implementation slices and scope gates

Type: wayfinder:grilling
Status: resolved
Blocked by: [Specify the replication evidence contract](11-specify-replication-evidence-contract.md), [Define the minimal remote-player protocol decoder](13-define-minimal-remote-protocol-decoder.md), [Design the Remote Avatar session-event boundary](14-design-remote-session-event-boundary.md), [Design the Remote Avatar presentation boundary](15-design-remote-avatar-presentation-boundary.md), [Design the deterministic replication test harness](16-design-deterministic-replication-test-harness.md), [Design the Role-reversed live proof](17-design-role-reversed-live-proof.md), [Extend the manual two-window acceptance](18-extend-manual-two-window-acceptance.md)

## Question

What smallest cumulative implementation slices turn the accepted contracts into
production capability, and what entry gates, exit gates, verifications, and
deferrals keep each slice focused on Shared-Host Realm replication?

## Decision boundaries

Do not implement the slices. Preserve current World-entry Acceptance, keep
Windows/LAN/gameplay out of scope, and admit a new wire behaviour only when it
blocks an already accepted gate.

## Answer

[Shared-Host Replication implementation slices and scope gates](../research/19-implementation-slices-and-scope-gates.md)
defines seven cumulative ownership slices from the minimal remote decoder to
hash-bound manual finalization. Every slice has an entry gate, exit gate,
verification, and explicit deferrals; Ticket 21 remains an already-complete
Fixture Pair prerequisite rather than duplicated scope.
