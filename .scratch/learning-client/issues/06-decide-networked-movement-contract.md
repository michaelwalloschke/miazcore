# Decide the minimal networked movement contract

Type: wayfinder:grilling
Status: resolved
Blocked by: [Trace the minimal AzerothCore world-entry protocol](01-trace-world-entry-protocol.md), [Prototype the diagnostic World-entry experience](04-prototype-diagnostic-world-entry.md), [Prove the Reference Realm bootstrap](05-prove-reference-realm-bootstrap.md), [Define the minimum authoritative self-state boundary](10-define-minimum-authoritative-self-state-boundary.md)

## Question

What is the smallest movement authority, prediction, acknowledgement, and correction contract that genuinely exercises AzerothCore netcode, remains visible in the Diagnostic World, and counts as server-recognized movement for the World-entry Slice?

## Answer

Use immediate client-side planar kinematic prediction while preserving the Diagnostic World's three separate pose truths. Camera-relative input becomes **Heading-aligned Movement**: normalize the input vector, orient the character along that world heading, and use only `MSG_MOVE_START_FORWARD`, `MSG_MOVE_HEARTBEAT`, and `MSG_MOVE_STOP`. Predict at a deterministic 60 Hz, interpolate rendering between fixed states, send heartbeats at 10 Hz, and send start/stop immediately on movement transitions. Clamp displacement by elapsed time times the current realm-provided run speed and to the five-metre **Reference Movement Envelope** around the Entry Anchor, retaining the anchor `z` because the slice has no terrain or collision data.

Rendered Pose advances through prediction. Submitted Pose advances only after a complete movement frame is successfully written to the world socket. Realm-observed Pose does not advance during ordinary movement because AzerothCore sends no sender echo or acknowledgement. Intermediate heartbeats may be coalesced under backpressure, but start and stop preserve order and may not be silently dropped. A failed transition write, queue failure, socket failure, or loss of movement-ready session state stops prediction, disables input, preserves the visible Rendered/Submitted divergence, and requires reconnect; verification is unavailable unless the final stop was successfully submitted.

The live slice does not implement mid-session teleport, knockback, root, transport, or other correction/control families. Its authoritative pose sources are the bootstrap self-create and `SMSG_LOGIN_VERIFY_WORLD`, including the verification reconnect. The session boundary remains able to express a generic correction event, and presentation tests retain the explicitly labelled scripted correction. Reconnect disagreement uses the selected magenta correction treatment: interpolate for approximately 300 ms, but snap on map changes or deltas greater than five metres.

**Movement Proof** is the sole claim of server-recognized movement. It becomes eligible only after a successfully submitted stopped pose at least two metres from the Entry Anchor. Starting proof freezes input, performs a saving logout, waits for completion/offline state, creates a fresh authenticated world session, and succeeds only when the next `SMSG_LOGIN_VERIFY_WORLD` reports the same map within 0.25 m of the expected Submitted Pose. Database inspection may assist diagnostics but cannot produce success, and no earlier state may be labelled accepted, acknowledged, or authoritative.
