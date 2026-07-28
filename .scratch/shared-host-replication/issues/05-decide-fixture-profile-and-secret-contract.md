# Decide the Fixture Profile and secret contract

Type: wayfinder:grilling
Status: open
Blocked by: [Design Fixture Pair provisioning](02-design-fixture-pair-provisioning.md)

## Question

How do named, non-sensitive Fixture Profiles select the two Fixture Pair
members and their separate private credential files while preserving the
Learning Client's existing redaction and configuration boundaries?

## Decision boundaries

Specify selection syntax, file ownership/permissions, invalid-profile failure,
and evidence-safe display identity. Do not pass credentials by CLI, introduce a
general account manager, or widen the public credential API.

