# Decide machine-attempt curation for final evidence

Type: wayfinder:grilling
Status: open
Blocked by: [Specify the replication evidence contract](11-specify-replication-evidence-contract.md), [Design the Role-reversed live proof](17-design-role-reversed-live-proof.md), [Extend the manual two-window acceptance](18-extend-manual-two-window-acceptance.md)

## Question

What exact immutable staging and curation layout preserves every failed or
successful Role-reversed machine attempt—including parent command/control
records, redacted machine summary, and allowlisted diagnostics—while producing
the closed Ticket 11 final evidence bundle with no unlisted file, directory,
digest cycle, or mutable provenance gap?

## Decision boundaries

Decide only the machine-attempt versus final-bundle artifact boundary,
canonical file schemas, hash/provenance binding, failure retention, and
validator behavior. Do not implement the proof, change Remote Avatar protocol
or presentation, add retry, or broaden macOS loopback scope.
