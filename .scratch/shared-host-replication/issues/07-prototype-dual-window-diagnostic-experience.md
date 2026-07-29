# Prototype the dual-window Diagnostic World experience

Type: wayfinder:prototype
Status: resolved
Blocked by: [Define the Realm-replicated Avatar contract](04-define-realm-replicated-avatar-contract.md)

## Question

What minimal project-owned two-window presentation makes the local Character,
one Remote Avatar Marker, identity label, heading, raw Realm-observed Pose,
smoothed Rendered Pose, lifecycle, and redacted failure state unmistakable to a
human observer?

## Decision boundaries

Use placeholders only. Settle marker distinction, inspector language, and the
short manual two-window checklist; do not build production presentation or add
models, terrain, gameplay UI, or arbitrary player lists.

## Answer

[Dual-window Diagnostic World experience](../research/07-dual-window-diagnostic-experience.md)
defines symmetric independent windows, project-owned Local/Remote primitives,
separate observed/rendered inspector language, redacted fault state, and the
short manual two-window checklist.

## Comments

- 2026-07-29: Decision accepted: local Character is a cyan capsule, Remote
  Avatar Marker is an amber faceted primitive, and colour is never the sole
  visual distinction.
