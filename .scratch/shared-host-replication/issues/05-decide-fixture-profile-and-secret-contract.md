# Decide the Fixture Profile and secret contract

Type: wayfinder:grilling
Status: resolved
Blocked by: [Design Fixture Pair provisioning](02-design-fixture-pair-provisioning.md)

## Question

How do named, non-sensitive Fixture Profiles select the two Fixture Pair
members and their separate private credential files while preserving the
Learning Client's existing redaction and configuration boundaries?

## Decision boundaries

Specify selection syntax, file ownership/permissions, invalid-profile failure,
and evidence-safe display identity. Do not pass credentials by CLI, introduce a
general account manager, or widen the public credential API.

## Answer

[Fixture Profile and secret contract](../research/05-fixture-profile-and-secret-contract.md)
defines the closed `--fixture-profile pair-a|pair-b` selector, its fixed
private file mapping, redacted failures, and the existing session
configuration boundary as the sole credential loader.

## Comments

- 2026-07-29: Decision accepted: profile tokens are non-sensitive closed CLI
  selectors; Credentials remain private files owned by fixture reset.
