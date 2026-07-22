# Trace the minimal AzerothCore world-entry protocol

Type: wayfinder:research
Status: resolved

## Question

What exact 3.3.5a protocol messages, cryptographic/session transitions, data dependencies, and observable server responses are required for the Learning Client to authenticate a configured account, select its pre-provisioned character, enter the Reference Realm, and send movement that AzerothCore accepts—and what source provenance and license constraints govern that understanding?

## Answer

Implement two TCP state machines: the build-12340 login/SRP6/realm-list exchange, then world authentication with encrypted headers, mandatory character enumeration, character login, world verification, time synchronization, no-flight control acknowledgement, and ground movement. AzerothCore does not echo ordinary accepted movement to its sender, so the black-box acceptance oracle is a server-completed save followed by reconnect and a moved position in `SMSG_LOGIN_VERIFY_WORLD`. The controlled Reference Realm must disable executable-integrity proof and Warden, use a no-TOTP account, and provision a stable ground character. The pinned message layouts, crypto transitions, configuration/data contract, license boundary, and a candidate-library compatibility discrepancy are recorded in [Minimal AzerothCore world-entry protocol](../research/minimal-azerothcore-world-entry-protocol.md).
