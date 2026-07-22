# Define the implementation slices and scope gates

Type: wayfinder:grilling
Status: resolved
Blocked by: [Define the reproducible Reference Realm environment](02-define-reference-realm-environment.md), [Choose the Learning Client engine direction](03-choose-engine-direction.md), [Decide the minimal networked movement contract](06-decide-networked-movement-contract.md), [Design the engine-independent Learning Client architecture](07-design-client-architecture.md), [Specify the World-entry verification contract](08-specify-verification-contract.md), [Prove the Bevy shell and platform test path](11-prove-bevy-shell-platform-path.md)

## Question

In what order should the Reference Realm, protocol core, engine shell, Diagnostic World, movement loop, and verification layers be implemented, and what entry, exit, and explicit deferral conditions keep each slice independently learnable and testable?

## Answer

Implement the Learning Client as eight capability-first vertical slices. A slice may touch `client_protocol`, `client_session`, `client_bevy`, the composition binary, fixtures, and verification tooling, but it implements only what its observable capability requires. Do not organize the work as “finish one crate, then the next”: every slice begins from the prior passing gate, adds its tests with its behavior, and ends in independently inspectable evidence. The Reference Realm is a proven prerequisite and recurring live gate, not a client implementation slice. The existing Bevy and browser implementations are disposable proofs, not production foundations.

### 1. Production scaffold and offline Diagnostic World

Create the production Cargo workspace containing `client_protocol`, `client_session`, `client_bevy`, and `learning_client`; pin Rust, Bevy, dependencies, and `Cargo.lock`; enforce the one-way dependency graph; and establish the routine verification commands. Implement immutable configuration, credential-file validation, redacted/zeroizing secret ownership, sanitized identity, the final command/event/snapshot types and bounded boundary, and a fake offline session source. Rewrite the accepted primitive world, ordered plugin composition, diagnostics, chase/orbit camera, and input mapping in production Bevy code rather than copying either prototype.

**Entry:** Reference Realm health/smoke and the disposable Bevy proof are green. **Exit:** the production app renders an accurate `Offline` Diagnostic World through Metal; formatting, locked native workspace/all-target checks, Clippy with warnings denied, native tests, `MinimalPlugins` adapter tests, the dependency-boundary assertion, redaction tests, scripted Metal smoke, and Windows compile tripwire pass. **Deferred:** every codec, socket, live entry, prediction, movement frame, and Movement Proof. Remove disposable Bevy code only after production evidence supersedes it; retain its research record and referenced evidence.

### 2. Authenticated realm discovery

Establish the fixture-manifest format and synthetic SRP/header-crypto vectors. Implement login framing, challenge/proof exchange, authentication rejection mapping, and realm-list decoding in `client_protocol`. Introduce the real session worker and private transport/clock/entropy ports, implement the login-side internal state path, and emit accepted redacted semantic stages through the final boundary. Add deterministic success, rejection, timeout, fragmented-read, malformed-frame, shutdown, and secrecy scenarios plus a headless live integration that authenticates the fixture account, selects realm ID `1` / `Miazcore Reference Realm`, verifies build `12340` and its advertised world endpoint, then disconnects.

**Entry:** slice 1's production scaffold and routine gate pass; realm health is green. **Exit:** independent golden tests and the real login/realm path pass through production codecs and runtime, secrets never leak, and worker shutdown is reliable. **Deferred:** world authentication/live header crypto, character enumeration, player login, Bevy-driven connection, and movement. The production application remains offline; the intermediate path is exercised through the engine-free integration harness so `StartEntry` is never partially user-facing.

### 3. Authenticated character selection

Implement world challenge/session authentication, exact build-12340 proof, independent inbound/outbound encrypted header streams, fragmented/coalesced world framing, and safe complete-unknown-opcode skipping. Decode complete character-enumeration records, select configured `Miaztest`, and extend deterministic session behavior through `Entering(CharacterSelection)`. Cover cipher drift, malformed headers, world-auth rejection, absent/duplicate configured character, timeout, EOF, cancellation, and redacted diagnostics. The live integration performs login, realm discovery, fresh world authentication, and exact character selection, then disconnects before `CMSG_PLAYER_LOGIN`.

**Entry:** slice 2 passes deterministically and against the realm. **Exit:** the real world session authenticates, encrypted framing stays aligned across multiple packets, exactly one configured character is selected, and the nonexistent-account and absent-character live failures produce the accepted categories. **Deferred:** player login, bootstrap consumption, world verification, self state, control/time sync, Bevy live entry, and movement.

### 4. Movement-ready world entry

Send `CMSG_PLAYER_LOGIN`; consume or safely skip initialization packets; decode `SMSG_LOGIN_VERIFY_WORLD`; implement bounded zlib handling and uncompressed/compressed update containers; locate exactly one matching player `CreateObject2` with `SELF | LIVING`; and consume opaque values without constructing a game-object/update-field model. Implement project-owned `AcoreMovementInfo`, all nine bootstrap speeds, positive run-speed selection, later run-speed changes/ACKs, initial time synchronization, and the no-flight ACK. Complete the public `StartEntry` path, Movement-ready invariant, input gating, semantic stages, clean failure/shutdown, and retry-from-fresh-session behavior. Capture and independently validate the sanitized self-update fixtures.

**Entry:** slice 3 passes deterministically and live. **Exit:** the headless production session reaches `MovementReady` against the real realm with matching selected/self GUID, agreeing entry/self poses, positive run speed, complete sync ACKs, no premature movement consumption, and clean disconnect; every malformed self-state/sync scenario passes deterministically. **Deferred:** prediction, movement packets, Movement Proof, live Bevy connection, rendering interpolation, and live corrections.

### 5. Live Diagnostic World entry

Wire **Connect & Enter Reference Realm** to the complete `StartEntry`. Project real stages, sanitized identity, run speed, Entry Anchor, Realm-observed Pose, queue counters, and semantic event history into Bevy; spawn the controlled placeholder and markers from real state; preserve `Ingress -> Input -> Presentation -> Camera -> Diagnostics`; keep camera/focus behavior responsive; and show redacted fail-closed diagnostics. Movement publication stays disabled. Add `MinimalPlugins` projection tests and a real Metal scenario connected to the realm.

**Entry:** slice 4 passes headlessly against the realm and the offline shell stays green. **Exit:** the real app reaches `MovementReady`, renders the placeholder at the actual Entry Anchor, displays matching diagnostics, emits no movement packets, remains responsive, and joins the worker cleanly; scripted window and Windows tripwire checks remain green. **Deferred:** movement intent, prediction, start/heartbeat/stop, pose divergence, live correction, and Movement Proof.

### 6. Predicted and submitted movement

Publish camera-relative intent through the latest-value mailbox; implement deterministic 60 Hz Heading-aligned Movement using current run speed; retain Entry Anchor height; and enforce the Reference Movement Envelope. Produce immediate ordered start/stop frames and 10 Hz coalescible heartbeats using `AcoreMovementInfo`. Advance Submitted Pose only after complete writes, keep Realm-observed Pose unchanged, interpolate Rendered Pose from Predicted Pose, and display their divergence. Implement queue/socket/write fail-closed behavior plus generic smooth/snap correction and scripted presentation. Add fake-clock tests for rates, elapsed-time bounds, transitions, coalescing, envelope edges, focus loss, failures, and correction thresholds.

**Entry:** slice 5 passes without emitting movement. **Exit:** the real app moves/turns smoothly, writes valid start/heartbeat/stop frames, visibly separates Rendered/Submitted from unchanged Realm-observed state, respects the envelope, and remains connected after stop; a scoped realm reset surrounds the smoke. **Deferred:** saving logout, reconnect comparison, database-based conclusions, and every claim of realm acceptance or persistence.

### 7. Movement Proof and recovery

Enable proof only after a successfully submitted stopped pose at least two metres from Entry Anchor. Wire **Verify persisted movement** to `BeginMovementProof`, freeze input/prediction, complete saving logout and offline wait, discard old transport/crypto, create fresh login/world sessions, and compare reconnect Realm-observed Pose with expected Submitted Pose. Pass only on the same map within `0.25 m`; show expected/observed map, poses, delta, and evidence source on success and failure; apply the accepted smooth/snap treatment; and complete visible explicit recovery for every accepted failure category. Cover eligibility, logout, offline wait, reconnect, comparison, timeouts, failures, shutdown, and retry deterministically, then run the isolated canonical live success and two live negative probes.

**Entry:** slice 6 passes deterministically and in its reset-scoped live smoke. **Exit:** the production app moves two to four metres, successfully submits stop, completes saving logout/fresh reconnect, and produces the sole protocol-visible Movement Proof within tolerance; every failure remains redacted and fail-closed, and database evidence cannot create success. **Deferred:** broader locomotion/control, terrain/collision, gameplay, multi-client interaction, Windows acceptance, and reconnect convenience beyond the accepted path.

### 8. World-entry Acceptance hardening

Add no product capability. Complete missing golden, malformed, property, fuzz-regression, deterministic session, Bevy adapter, dependency-boundary, and redaction tests; consolidate repository-owned gate commands and the exclusive realm lock; run the scripted Metal smoke and Windows tripwire; exercise the reset-scoped canonical live success and two negative probes; perform the full manual macOS checklist; and generate, validate, hash, and curate the Acceptance Evidence Bundle. Remove remaining disposable prototype code only after equivalent production evidence exists. Fix only defects required for an already-agreed gate.

**Entry:** slice 7 passes deterministically and through the real application. **Exit:** one clean `candidate_sha` passes all four World-entry Acceptance gates without skips, expected failures, automatic retries, warning suppression, leaks, or missing evidence; an evidence-only commit records it. **Deferred:** all new gameplay/multiplayer behavior, broader packet/movement coverage, authored content, general polish, unmeasured optimization, and Windows runtime/render acceptance.

### Cumulative scope gates

The main path starts a slice only after the preceding exit gate passes on the current branch. Tests and verification tooling ship with their capability. Parallel preparation may occur but cannot integrate past an unmet predecessor. A discovered requirement joins the active slice only when its exit capability cannot work correctly without it; useful non-blockers become explicit deferrals. Fix a failing gate in the active slice—never waive, mute, retry away, or postpone it to hardening. Record entry evidence, exit evidence, remaining deferrals, and the exact passing commit for every slice. Hardening closes evidence/coverage gaps only; it is not a destination for omitted functionality.

Maintain the Windows compile tripwire in every slice and fix regressions immediately, but branch native Windows build/test/render work only after World-entry Acceptance and before multiplayer requires Windows operationally. Gameplay/content, multiplayer/LAN exposure, and broader world state each require new post-acceptance maps. Add an unmodeled opcode only when the live realm proves it blocks the active exit gate; otherwise skip and defer it. Unsupported movement/control state fails the current slice rather than silently expanding it. Reopen Bevy only for an already-accepted platform reliability or substantial authored-content/UI trigger. Contain upstream protocol incompatibility behind `client_protocol`; do not expose generated types, fork dependencies, or widen scope without a new decision.
