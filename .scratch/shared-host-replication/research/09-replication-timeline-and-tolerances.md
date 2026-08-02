# Fixture Pair replication timeline and tolerances

Ticket: [09 – Measure replication timeline and tolerances](../issues/09-measure-replication-timeline-and-tolerances.md)

## Decision

The local Reference Realm's first Shared-Host Replication proof uses the
following calibrated limits for one serial two-to-four-metre `Replicated Move`.
They come from three reset-scoped Fixture Pair A/B measurements, not from a
network-latency assumption.

| Contract | Calibrated limit | Basis |
| --- | ---: | --- |
| terminal observer pose comparison | `<= 0.25 m` | fixed destination contract; all three terminal deltas were `0.000 m` |
| first remote-pose deadline | `331 ms` | observed maximum (`209 ms`) plus one complete observed update cadence (`122 ms`) |
| terminal remote-stop deadline | `508 ms` | observed maximum from mover command (`386 ms`) plus one cadence (`122 ms`) |
| clean-logout remote removal deadline | `19,760 ms` | observed maximum from logout request (`19,638 ms`) plus one cadence (`122 ms`) |
| remote projection snap distance | `>= 1.628 m` | twice the largest complete observed pose delta (`0.814 m`) |

The projection may smooth only a same-map update below `1.628 m`; a map change
or a delta at or above that value snaps. This is a presentation decision, not
remote prediction: its target remains the latest Realm-observed pose.

## Method and evidence

[`scripts/measure-fixture-pair-timeline.sh`](../../../scripts/measure-fixture-pair-timeline.sh)
acquires the canonical Realm lock, performs a fresh reset before every sample,
and launches Pair A as the observer and Pair B as the sole mover. A single
controller `Instant` timestamps the mover commands and Pair A's bounded,
semantic World-Update transcript. Pair B makes a `2.556 m` scripted movement,
then begins the existing saving-logout flow. The observer establishes the
actual lifecycle `CreateObject2 → movement → MSG_MOVE_STOP → Destroy`.

The passed redacted artifact is
[`summary.json`](../../../artifacts/shared-host-replication/20260802T123009Z-fixture-pair-timeline/summary.json),
bound to `79c5df928d643686ec16b0fde45578b08caf2266`. It records separate Pair
GUID shorthands only, finite poses, controller/observer timing, and final Realm
health. It contains no credentials, account values, session material, raw
frames, payloads, or client logs.

The individual results were:

| Sample | first remote pose | terminal stop | removal | max cadence | terminal delta |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1 | 22 ms | 379 ms | 19,638 ms | 97 ms | 0.000 m |
| 2 | 209 ms | 386 ms | 19,407 ms | 122 ms | 0.000 m |
| 3 | 123 ms | 372 ms | 19,094 ms | 107 ms | 0.000 m |

The script finishes with one more canonical reset and health check. A failed
sample writes only its stage/commit failure record and performs one same-owner
recovery; it never writes a passed calibration summary.

## Scope boundary

These values apply only to the local macOS loopback Reference Realm, exact
Fixture Pair profiles, serial role turns, and the measured ordinary-ground
movement. They are neither a latency benchmark nor an entitlement to LAN,
Windows, simultaneous movement, arbitrary Remote Avatars, remote reconnect,
or peer-crash recovery.

The observer's `Destroy` is the required Remote Avatar removal evidence. The
runner deliberately does not make the controlled character's later
persistence/reconnect comparison a nested replication requirement.
