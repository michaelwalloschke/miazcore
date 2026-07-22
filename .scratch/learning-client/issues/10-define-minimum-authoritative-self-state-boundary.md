# Define the minimum authoritative self-state boundary

Type: wayfinder:research
Status: resolved
Blocked by: [Trace the minimal AzerothCore world-entry protocol](01-trace-world-entry-protocol.md)

## Question

Which minimum subset of `SMSG_UPDATE_OBJECT`, initial speed/control packets, and AzerothCore-specific `MovementInfo` fields must the World-entry Slice parse or adapt to establish authoritative self state and ground speed without implementing the full update-field model, and how should the known `wow_world_messages` movement-layout discrepancies be handled?

## Answer

Gate movement on a bounded decode of `SMSG_UPDATE_OBJECT` or its mandatory-in-practice compressed form, then require exactly one selected-player `CreateObject2` carrying `SELF | LIVING`. Structurally consume every update block needed to find it, but treat update values as an opaque mask plus one skipped `u32` per set bit. The self living block confirms the selected mover, corroborates the `SMSG_LOGIN_VERIFY_WORLD` entry pose, and supplies all nine absolute speeds; retain the current run speed rather than hard-coding `7.0`, and keep it current through `SMSG_FORCE_RUN_SPEED_CHANGE` plus its ACK. Movement remains gated until time synchronization and the no-flight acknowledgement are sent; special movement/control states fail explicitly in this slice.

Use a project-owned, engine-free `AcoreMovementInfo` codec for self updates, outgoing ground movement, can-fly ACKs, and run-speed ACKs. It keeps `flags: u32` and `flags2: u16` separate, uses `fall_time_ms: u32`, and orders jump data as `z_speed, sin_angle, cos_angle, xy_speed`. Do not expose the pinned generated `wow_world_messages::MovementInfo`: it types the timer as `f32`, reverses sine/cosine, and also gives two AzerothCore update-flag bits different names. The exact byte boundary, decompression guardrails, control treatment, and required golden/live tests are recorded in [Minimum authoritative self-state boundary](../research/minimum-authoritative-self-state-boundary.md).
