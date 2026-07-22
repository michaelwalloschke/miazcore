# Slice 15 sanitized live self projection

Date: 2026-07-22  
Source commit: `a4ab07218aa0a7a4ff7b1a2c259bcead0bdfa61f`  
Worldserver image: `acore/ac-wotlk-worldserver:17.0.0-dev@sha256:0a601595920e19c4af10679e4c01ac10f60569fc1e737db54aa6a5a07efb2455`

After `scripts/live-movement-ready.sh` reset and passed, a second production
session projected only the decoded authoritative self semantics before any
movement was enabled. It observed ordinary ground flags `0x00000000`, secondary
flags `0x0000`, pose `[-8949.95, -132.493, 83.5312, 0.0]`, fall time `0`, and
all nine absolute speeds:

```text
[2.5, 7.0, 4.5, 4.722222, 2.5, 7.0, 4.5, 3.141594, 3.14]
```

No authenticated packet body, update-field value, credential, key, or real GUID
was retained. The committed `world-entry-live-self-projection-body` fixture was
independently rebuilt with Python `struct` primitives: the observed pose and
speeds were retained, while GUID `0x1234`, timestamp `0`, and zero opaque values
were substituted. Its manifest SHA-256 and the Rust golden decoder independently
validate that sanitized projection.
