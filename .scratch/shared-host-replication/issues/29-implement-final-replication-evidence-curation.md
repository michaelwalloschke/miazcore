# Implement final replication evidence curation

Type: implementation
Status: open
Blocked by: [Implement role-reversed live machine proof](28-implement-role-reversed-live-machine-proof.md)

## Objective

Turn one successful immutable Machine Attempt and its bounded manual review into
a closed, independently validated Final Evidence Bundle.

## Entry gate

Ticket 28 has produced a retained immutable PASS Machine Attempt with canonical
sidecars, capture, versions, final Realm health, and valid pre-attestation
provenance digests.

## Scope

- Validate the closed manual attestation and the Machine Attempt provenance.
- Byte-copy and re-hash only canonical source files into the closed bundle,
  write the report and final manifest, and publish only a fresh complete bundle.
- Preserve failed finalization as a separate redacted sibling record without
  mutating the Machine Attempt.

## Out of scope

- New Remote Avatar behavior, a new live proof, retry/recovery policy, a second
  World-entry Acceptance, LAN, Windows runtime, or manual timing measurement.

## Acceptance

1. Finalization rejects missing/extra files or directories, symlinks/hard-links,
   mutable/mismatched provenance inputs, duplicate bundle IDs, wrong hashes,
   wrong attempt/candidate/capture binding, malformed/secret-bearing
   attestation, and digest cycles.
2. The final bundle contains exactly its approved closed files, including
   machine provenance, two semantic sidecars, capture, manual attestation,
   report, and manifest; runtime controls and diagnostics are absent.
3. A failed curation or attestation creates only the separate redacted
   finalization-failure record and cannot alter a closed Machine Attempt or
   publish a PASS bundle.
4. A fresh validator process accepts a successful bundle only after final Realm
   health and all automatic/manual gates validate.
5. Validator/curation tests and complete workspace check pass.

## Required evidence

- One independently revalidated Final Evidence Bundle bound to its Machine
  Attempt by candidate, attempt ID, provenance, capture, and hashes.
