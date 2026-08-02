# Implement minimal Remote-player protocol records

Type: implementation
Status: resolved
Blocked by: None — can start immediately

## Objective

Make the retained World path produce bounded semantic Remote-player records from
complete authenticated World frames, without broadening the client into a
general object decoder.

## Entry gate

The current World-entry codec/framing suite passes, the pinned build-12340
source provenance is unchanged, and synthetic fixtures remain project-owned.

## Scope

- Decode only the accepted player create, ordinary-ground movement,
  out-of-range, destroy, and unusable-movement record vocabulary.
- Consume supported compressed and uncompressed update containers exactly, then
  return semantic records keyed only by non-zero Realm GUID.
- Preserve the existing incremental frame/cipher boundary and use
  project-owned synthetic test bodies only.

## Out of scope

- Accepted Remote Avatar selection, session events, Bevy state, Fixture Pair
  runtime, names/models, generic object storage, or live proof orchestration.

## Acceptance

1. Complete valid player records become bounded semantic output; self, NPC,
   VALUES, NEAR_OBJECTS, unrelated GUID, and other complete frames are consumed
   safely without output.
2. Fragmentation/coalescing regression tests prove the existing incremental
   framing boundary preserves cipher alignment and never calls this decoder
   with a partial frame; malformed headers remain that boundary's failure.
   Malformed containers, compression, packed GUIDs, or trailing bytes fail
   closed in this decoder.
3. Unusable but structurally complete movement returns only a redacted
   per-GUID category; no raw values, names, paths, credentials, or cursor leak.
4. Focused protocol tests and the workspace check pass.

## Required evidence

- Golden/unit coverage for every accepted record, ignored complete
  VALUES/NEAR_OBJECTS input,
  compression, arbitrary chunk boundaries, and malformed failures.
- No new public generic decoder or raw-payload API.

## Answer

`client_protocol` now exposes `decode_remote_player_frame`, a complete-frame
only boundary that emits bounded GUID-keyed create, ordinary-ground movement,
out-of-range, destroy, and redacted unusable-movement records. It exactly
consumes supported compressed and uncompressed update containers; the prior
research tracer now adapts this same decoder rather than maintaining a second
structural walker. Synthetic tests cover all three direct movement opcodes,
container records and ignored blocks, compression, arbitrary fragmented and
coalesced encrypted-frame delivery, plus malformed container, compression,
packed-GUID, and trailing-byte failures.

`cargo test -p client_protocol`, `cargo clippy --workspace --all-targets -- -D
warnings`, and `scripts/check.sh` pass.
