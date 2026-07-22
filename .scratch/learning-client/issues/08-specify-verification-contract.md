# Specify the World-entry verification contract

Type: wayfinder:grilling
Status: resolved
Blocked by: [Trace the minimal AzerothCore world-entry protocol](01-trace-world-entry-protocol.md), [Define the reproducible Reference Realm environment](02-define-reference-realm-environment.md), [Prototype the diagnostic World-entry experience](04-prototype-diagnostic-world-entry.md), [Prove the Reference Realm bootstrap](05-prove-reference-realm-bootstrap.md), [Decide the minimal networked movement contract](06-decide-networked-movement-contract.md), [Design the engine-independent Learning Client architecture](07-design-client-architecture.md), [Prove the Bevy shell and platform test path](11-prove-bevy-shell-platform-path.md)

## Question

Which deterministic fixtures, live Reference Realm scenarios, failure diagnostics, evidence artifacts, and manual 3D checks are necessary and sufficient to prove authentication, world entry, rendering, and server-recognized movement without pulling later gameplay or multiplayer scope into the slice?

## Answer

**World-entry Acceptance** is conjunctive: the same candidate must pass the deterministic core, Bevy/platform, live Reference Realm, and manual macOS gates. The first two run routinely; the realm gate runs serially on the prepared acceptance host; the manual gate runs for a milestone candidate. A mock, screenshot, database query, compile check, or any other artifact from one gate cannot substitute for another gate.

### Deterministic core gate

Maintain a committed, versioned wire-fixture corpus whose expected bytes are independent of the codec under test. Small login, world, movement, ACK, and time-sync packets use manually specified bytes; SRP6 and header-crypto vectors use synthetic fixed credentials, entropy, and keys; complex compressed self-update fixtures may use sanitized decrypted bodies captured from the pinned Reference Realm. Every fixture manifest records build `12340`, direction, opcode, expected semantics, byte length, SHA-256, provenance, and relevant upstream pin. Live credentials, real session keys, authenticated transcripts, and server data are forbidden. Successful decoders consume the complete body and encoders match exact bytes; round-trip tests are supporting evidence only.

The protocol matrix covers:

- successful and rejected login/SRP exchanges, realm-list selection, and build handling;
- world authentication, independent directional header-cipher streams across multiple packets, fragmented/coalesced framing, unknown-opcode skipping, truncation, and invalid declared lengths;
- character enumeration, world verification, time sync, no-flight control ACK, and run-speed change/ACK;
- uncompressed and compressed self `CreateObject2`, exactly one matching self living block, opaque update-mask skipping, and complete consumption;
- exact start, heartbeat, and stop movement frames, packed GUID variants, a non-zero integer fall timer, distinct sine/cosine values, and finite-value/flag validation; and
- malformed decompression and update inputs including oversized declarations, trailing bytes, duplicates, bad masks, output overrun/underrun, unsupported flags, and non-finite values.

Routine CI runs all golden tests, bounded deterministic property cases, and every saved fuzz-regression input. Longer fuzz campaigns target framing, decompression, self-update, and movement decoders and contribute new regression fixtures, but elapsed fuzzing time is not itself an acceptance criterion.

Scripted session tests use only the private transport, monotonic-clock, and entropy ports—never Docker, Bevy, real sockets, sleeps, or timing tolerances. They prove the exact successful entry phase/event/write sequence and movement gating; 60 Hz prediction; 10 Hz heartbeats; immediate ordered start/stop; realm-speed, height, and five-metre-envelope bounds; heartbeat-only coalescing; lossless transition handling; queue-overflow observability; smooth/snap corrections; every Movement Proof eligibility and comparison outcome; timeout/EOF/rejection/malformed/write faults at each applicable external wait; clean disconnect and shutdown from every public phase; and retry with fresh transport, cipher, time, and entropy state. Every failure assertion checks its stable category, stage, redacted context, recommended recovery, stopped prediction, gated input, and absence of automatic retry. Formatting all public commands, events, snapshots, errors, and diagnostics must prove that credentials and session material cannot appear.

### Bevy/platform gate

Require formatting, locked native workspace/all-target compilation, Clippy with warnings denied, native workspace tests, and a dependency-boundary assertion that only `client_bevy` and the composition binary depend on Bevy. `MinimalPlugins` scenarios prove `Ingress -> Input -> Presentation -> Camera -> Diagnostics` ordering, Movement-ready input gating, event/snapshot projection, separation of Rendered/Submitted/Realm-observed poses, smooth and snap presentation, and visible fail-closed behavior.

A bounded scripted Apple Silicon window run uses fixed frame time, real Metal rendering, project-owned primitives, automatic exit, and a screenshot plus semantic sidecar. The screenshot must exist, have credible dimensions and content, and be hashed, but it is not a cross-platform pixel golden. Retain the established `x86_64-pc-windows-msvc` all-target check with the narrow BLAKE3/Clang workaround as a required portability tripwire. It is not Windows linking, testing, runtime, or rendering evidence.

### Live Reference Realm gate

Acquire an exclusive repository-scoped realm-test lock, verify the Docker/Rosetta prerequisites, locks, secret permissions, and effective Compose identity, then run the label-scoped `reset-state --yes` to preserve the verified server-data cache while restoring the account, character, and Entry Anchor. Require layered health before the client starts.

Run two non-destructive negative probes first. A deliberately nonexistent account supplied through temporary `0600` secret files must produce `Failed(Authentication)` with useful redacted diagnostics and no retry. Valid fixture credentials plus a deliberately absent character name must produce `Failed(Configuration)`, report the sanitized mismatch, avoid player login, and disconnect cleanly. More destructive transport, malformed-packet, control-state, backpressure, and proof faults remain scripted deterministic scenarios.

The canonical success scenario uses the real `client_session` path without Bevy. It authenticates build `12340`, selects `Miazcore Reference Realm`, selects exactly `Miaztest`, observes matching world-entry and self-living state, obtains a positive run speed, and proves input stayed gated through time/no-flight synchronization. It moves on a fixed heading to a successfully stopped Submitted Pose between two and four metres from the Entry Anchor, starts Movement Proof, completes saving logout, creates fresh login/world sessions, and passes only when the new Realm-observed Pose is on the same map within `0.25 m`. Reconnect is the sole success oracle; database queries and realm logs may diagnose failure only. Disconnect cleanly, release the lock, and restore state before the next run so movement never accumulates.

### Manual macOS gate

On the exact Apple Silicon candidate, a human verifies real Metal adapter/backend reporting, clear primitive-world/marker/diagnostic rendering at actual display scale, responsive phase progression, pre-ready input gating, right-mouse orbit, wheel zoom, focus behavior, and camera-relative `WASD`. Heading-aligned movement must turn and move smoothly without height drift, remain inside the visible envelope, advance Rendered and Submitted poses while Realm-observed stays at the Entry Anchor, freeze input through Movement Proof, and present the reconnect source plus expected/observed map, poses, and delta. Window close must complete bounded shutdown, leave the character offline, and retain realm health.

A separate scripted visual-diagnostics run at the Bevy boundary shows a sub-five-metre magenta correction target/vector with approximately 300 ms interpolation, a greater-than-five-metre or map-changing snap, and a representative visible failure with input gating, recovery guidance, semantic history, optional opcode provenance, and no secrets or raw packet dump. Every checklist item is pass/fail with human notes; screenshots support but do not replace the attestation of dynamic behavior.

### Evidence and finalization

Routine outputs are ephemeral. A milestone candidate produces one curated, redacted **Acceptance Evidence Bundle** containing a machine-readable manifest, deterministic/fuzz-regression summaries, semantic live-realm trace, Metal screenshot and sidecar, completed manual checklist, and a concise Markdown proof report. The manifest records the clean `candidate_sha`, toolchain/host versions, effective realm digests and fixture hashes, exact commands, results, durations, and hashes of retained artifacts. Do not commit raw packet bodies, authenticated transcripts, database dumps, server data, unrestricted logs, credentials, session keys, or unbounded output; an automated redaction scan is itself required.

Results are `PASS`, `FAIL`, or `NOT_RUN`; skipped, ignored, quarantined, expected-failure, or automatically retried required scenarios cannot count as passing. A rerun is a new recorded attempt with a reason. Any change to client code, tests, `Cargo.lock`, fixture corpus, realm infrastructure/locks, or acceptance tooling invalidates the bundle and requires all four gates again. The evidence-only commit made afterward names the tested clean candidate and does not invalidate it. Unexpected warnings, missing artifacts or hashes, dirty candidate state, and redaction failure invalidate acceptance. No numeric line-coverage threshold is required: the accepted behavioral matrices define sufficiency, while coverage reports remain advisory gap-finding evidence.
