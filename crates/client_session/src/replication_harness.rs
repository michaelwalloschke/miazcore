//! Test-only deterministic observer harness for the Shared-Host slice.
//!
//! It deliberately models only what an observer may claim: accepted Remote
//! Avatar facts and the virtual instant at which they were received. It owns
//! neither a transport nor a Realm simulation and is compiled only for tests.

use std::time::Duration;

use client_protocol::{
    AcoreMovementInfo, HeaderCipher, HeaderDirection, IncrementalWorldServerDecoder,
    RemotePlayerRecord, SMSG_DESTROY_OBJECT, SMSG_TIME_SYNC_REQ, SMSG_UPDATE_OBJECT,
    decode_remote_player_frame,
};

use crate::{
    ClientEvent, ClientEventKind, RemoteAvatarChange, RemoteAvatarId, RemoteAvatarRemovalSource,
    SanitizedIdentity, WorldPose,
    boundary::{SessionClient, WorkerBoundary, new_boundary},
    runtime::retained_harness,
};

const FIRST_POSE_DEADLINE: Duration = Duration::from_millis(331);
const TERMINAL_POSE_DEADLINE: Duration = Duration::from_millis(508);
const REMOVAL_DEADLINE: Duration = Duration::from_millis(19_760);
const TERMINAL_TOLERANCE_METRES: f32 = 0.25;
const SYNTHETIC_WORLD_KEY: [u8; 40] = [0x5a; 40];
const SERVER_MOVE_STOP: u16 = 0x00b7;

#[derive(Clone, Debug, Eq, PartialEq)]
enum PollStep {
    Pending,
    Bytes(Vec<u8>),
    Eof,
    ReadError(std::io::ErrorKind),
}

fn consume_poll_script(script: Vec<PollStep>) -> Result<Vec<u8>, std::io::ErrorKind> {
    let mut bytes = Vec::new();
    for step in script {
        match step {
            PollStep::Pending => {}
            PollStep::Bytes(chunk) => bytes.extend(chunk),
            PollStep::Eof => return Err(std::io::ErrorKind::UnexpectedEof),
            PollStep::ReadError(kind) => return Err(kind),
        }
    }
    Ok(bytes)
}

#[derive(Debug)]
struct FakeClock {
    now: Duration,
    sleeps: Vec<Duration>,
}

impl FakeClock {
    const fn new() -> Self {
        Self {
            now: Duration::ZERO,
            sleeps: Vec::new(),
        }
    }

    fn advance_to(&mut self, instant: Duration) {
        assert!(instant >= self.now, "virtual clock cannot move backward");
        self.now = instant;
    }

    fn sleep(&mut self, duration: Duration) {
        self.sleeps.push(duration);
    }
}

struct FrameScript(Vec<(u16, Vec<u8>)>);

impl FrameScript {
    fn encrypted(&self) -> Vec<u8> {
        let mut cipher = HeaderCipher::new(HeaderDirection::ServerToClient, &SYNTHETIC_WORLD_KEY);
        let mut wire = Vec::new();
        for (opcode, payload) in &self.0 {
            let size = u16::try_from(payload.len() + 2).expect("synthetic frame fits header");
            let mut header = [size.to_be_bytes()[0], size.to_be_bytes()[1], 0, 0];
            header[2..].copy_from_slice(&opcode.to_le_bytes());
            cipher.apply(&mut header);
            wire.extend_from_slice(&header);
            wire.extend_from_slice(payload);
        }
        wire
    }
}

#[derive(Clone, Debug)]
struct ReceivedRemoteEvent {
    event: ClientEvent,
    received_at: Duration,
    snapshot_source_sequence: Option<u64>,
}

#[derive(Debug, Eq, PartialEq)]
enum ObserverError {
    NonRemoteFact,
    OutOfOrder,
    WrongGuid,
    Faulted,
    MissingCreated,
    MissingTerminalUpdate,
    MissingRemoval,
    UnorderedLifecycle,
    FirstPoseLate,
    TerminalPoseLate,
    RemovalLate,
    MapMismatch,
    TerminalMismatch,
    SnapshotSourceMismatch,
}

#[derive(Clone, Copy)]
enum LocalSubstitute {
    MoverLocal,
    Predicted,
    Submitted,
    Rendered,
    Database,
}

fn reject_local_substitute(_: LocalSubstitute) -> Result<(), ObserverError> {
    Err(ObserverError::NonRemoteFact)
}

/// Pure, one-observer oracle. It intentionally cannot receive local movement,
/// rendered pose, database state, or a mover session handle.
struct ObserverScenario {
    peer: RemoteAvatarId,
    target: WorldPose,
    facts: Vec<ReceivedRemoteEvent>,
    last_sequence: u64,
}

impl ObserverScenario {
    fn new(peer: RemoteAvatarId, target: WorldPose) -> Self {
        Self {
            peer,
            target,
            facts: Vec::new(),
            last_sequence: 0,
        }
    }

    fn record(&mut self, event: ClientEvent, received_at: Duration) -> Result<(), ObserverError> {
        let sequence = event.sequence;
        self.record_with_snapshot(event, received_at, Some(sequence))
    }

    fn record_with_snapshot(
        &mut self,
        event: ClientEvent,
        received_at: Duration,
        snapshot_source_sequence: Option<u64>,
    ) -> Result<(), ObserverError> {
        if event.sequence <= self.last_sequence {
            return Err(ObserverError::OutOfOrder);
        }
        let ClientEventKind::RemoteAvatar { change } = event.kind else {
            return Err(ObserverError::NonRemoteFact);
        };
        if remote_id(change) != self.peer {
            return Err(ObserverError::WrongGuid);
        }
        if matches!(change, RemoteAvatarChange::Faulted { .. }) {
            return Err(ObserverError::Faulted);
        }
        self.last_sequence = event.sequence;
        self.facts.push(ReceivedRemoteEvent {
            event,
            received_at,
            snapshot_source_sequence,
        });
        Ok(())
    }

    fn retry(&mut self) {
        self.facts.clear();
        self.last_sequence = 0;
    }

    fn assert_success(&self) -> Result<(), ObserverError> {
        let created = self.facts.iter().position(|fact| {
            matches!(
                fact.event.kind,
                ClientEventKind::RemoteAvatar {
                    change: RemoteAvatarChange::Created { .. }
                }
            )
        });
        let terminal = self.facts.iter().rposition(|fact| {
            matches!(
                fact.event.kind,
                ClientEventKind::RemoteAvatar {
                    change: RemoteAvatarChange::Updated { .. }
                }
            )
        });
        let removal = self.facts.iter().rposition(|fact| {
            matches!(
                fact.event.kind,
                ClientEventKind::RemoteAvatar {
                    change: RemoteAvatarChange::Removed { .. }
                }
            )
        });
        let created = created.ok_or(ObserverError::MissingCreated)?;
        let terminal = terminal.ok_or(ObserverError::MissingTerminalUpdate)?;
        let removal = removal.ok_or(ObserverError::MissingRemoval)?;
        if !(created < terminal && terminal < removal) {
            return Err(ObserverError::UnorderedLifecycle);
        }
        let created = &self.facts[created];
        let terminal = &self.facts[terminal];
        let removal = &self.facts[removal];
        if created.received_at > FIRST_POSE_DEADLINE {
            return Err(ObserverError::FirstPoseLate);
        }
        if terminal.received_at > TERMINAL_POSE_DEADLINE {
            return Err(ObserverError::TerminalPoseLate);
        }
        if removal.received_at > REMOVAL_DEADLINE {
            return Err(ObserverError::RemovalLate);
        }
        let pose = remote_pose(&terminal.event.kind).expect("terminal predicate provides pose");
        if terminal.snapshot_source_sequence != Some(terminal.event.sequence) {
            return Err(ObserverError::SnapshotSourceMismatch);
        }
        if pose.map_id != self.target.map_id {
            return Err(ObserverError::MapMismatch);
        }
        if planar_distance(pose, self.target) > TERMINAL_TOLERANCE_METRES {
            return Err(ObserverError::TerminalMismatch);
        }
        Ok(())
    }
}

/// Test-only driver for one real application boundary. It accepts already
/// decoded retained-session records, then stamps their FIFO events before any
/// later virtual-clock advance or drain.
struct RetainedObserverScenario {
    oracle: ObserverScenario,
    client: SessionClient,
    boundary: WorkerBoundary,
    clock: FakeClock,
}

impl RetainedObserverScenario {
    fn new(peer: RemoteAvatarId, target: WorldPose) -> Self {
        let identity = SanitizedIdentity::new(1, "Synthetic Realm", "Observer", 12_340)
            .expect("fixed synthetic identity is valid");
        let (client, boundary) = new_boundary(identity).expect("synthetic boundary is available");
        assert_eq!(client.drain_events().len(), 2);
        Self {
            oracle: ObserverScenario::new(peer, target),
            client,
            boundary,
            clock: FakeClock::new(),
        }
    }

    fn advance_to(&mut self, instant: Duration) {
        self.clock.advance_to(instant);
    }

    fn apply_and_poll(&mut self, record: RemotePlayerRecord) -> Result<(), ObserverError> {
        self.boundary
            .apply_remote_player_records(vec![record], Some(0))
            .expect("synthetic records must cross the session boundary");
        let snapshot_source_sequence = self
            .client
            .snapshot()
            .remote_avatar
            .map(|remote| remote.source_sequence);
        for event in self.client.drain_events() {
            if matches!(event.kind, ClientEventKind::RemoteAvatar { .. }) {
                self.oracle.record_with_snapshot(
                    event,
                    self.clock.now,
                    snapshot_source_sequence,
                )?;
            }
        }
        Ok(())
    }

    fn assert_success(&self) -> Result<(), ObserverError> {
        self.oracle.assert_success()
    }

    fn snapshot(&self) -> crate::ClientSnapshot {
        self.client.snapshot()
    }
}

fn remote_id(change: RemoteAvatarChange) -> RemoteAvatarId {
    match change {
        RemoteAvatarChange::Created { id, .. }
        | RemoteAvatarChange::Updated { id, .. }
        | RemoteAvatarChange::Removed { id, .. }
        | RemoteAvatarChange::Faulted { id, .. } => id,
    }
}

fn remote_pose(kind: &ClientEventKind) -> Option<WorldPose> {
    match kind {
        ClientEventKind::RemoteAvatar {
            change:
                RemoteAvatarChange::Created {
                    realm_observed_pose,
                    ..
                }
                | RemoteAvatarChange::Updated {
                    realm_observed_pose,
                    ..
                },
        } => Some(*realm_observed_pose),
        _ => None,
    }
}

fn planar_distance(left: WorldPose, right: WorldPose) -> f32 {
    (left.east - right.east).hypot(left.north - right.north)
}

fn remote_event(sequence: u64, change: RemoteAvatarChange) -> ClientEvent {
    ClientEvent {
        sequence,
        kind: ClientEventKind::RemoteAvatar { change },
    }
}

fn pose(east: f32, north: f32) -> WorldPose {
    WorldPose {
        map_id: 0,
        east,
        north,
        elevation: 1.0,
        orientation: 0.0,
    }
}

fn synthetic_movement(east: f32, north: f32) -> AcoreMovementInfo {
    AcoreMovementInfo::ground(7, [east, north, 1.0], 0.0, false)
}

fn packed_guid(output: &mut Vec<u8>, guid: u64) {
    let bytes = guid.to_le_bytes();
    let mask = bytes.iter().enumerate().fold(0_u8, |mask, (index, byte)| {
        mask | u8::from(*byte != 0) << index
    });
    output.push(mask);
    for byte in bytes {
        if byte != 0 {
            output.push(byte);
        }
    }
}

fn remote_movement_block(east: f32, north: f32) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&(0x0020_u16 | 0x0040_u16).to_le_bytes());
    body.extend_from_slice(&0_u32.to_le_bytes());
    body.extend_from_slice(&0_u16.to_le_bytes());
    body.extend_from_slice(&7_u32.to_le_bytes());
    for value in [east, north, 1.0, 0.0] {
        body.extend_from_slice(&value.to_le_bytes());
    }
    body.extend_from_slice(&0_u32.to_le_bytes());
    for _ in 0..9 {
        body.extend_from_slice(&0.0_f32.to_le_bytes());
    }
    body
}

fn remote_create_body(guid: u64, east: f32, north: f32) -> Vec<u8> {
    let mut body = 1_u32.to_le_bytes().to_vec();
    body.push(3);
    packed_guid(&mut body, guid);
    body.push(4);
    body.extend(remote_movement_block(east, north));
    body.push(0);
    body
}

fn remote_update_body(guid: u64, east: f32, north: f32) -> Vec<u8> {
    let mut body = 1_u32.to_le_bytes().to_vec();
    body.push(1);
    packed_guid(&mut body, guid);
    body.extend(remote_movement_block(east, north));
    body
}

fn remote_unusable_update_body(guid: u64) -> Vec<u8> {
    remote_update_body(guid, f32::NAN, 0.0)
}

#[test]
fn synthetic_encrypted_frame_script_survives_fragmented_pending_and_coalesced_polls() {
    let script = FrameScript(vec![
        (0x7fff, vec![1, 2]),
        (SMSG_TIME_SYNC_REQ, 0x1234_5678_u32.to_le_bytes().to_vec()),
        (0x7ffe, vec![3]),
    ]);
    let wire = script.encrypted();
    let mut decoder = IncrementalWorldServerDecoder::new(&SYNTHETIC_WORLD_KEY);
    let mut clock = FakeClock::new();
    let polls = vec![
        PollStep::Bytes(wire[..1].to_vec()),
        PollStep::Pending,
        PollStep::Bytes(wire[1..6].to_vec()),
        PollStep::Bytes(wire[6..].to_vec()),
    ];
    let mut frames = Vec::new();
    for step in polls {
        match step {
            PollStep::Pending => clock.sleep(Duration::from_millis(1)),
            PollStep::Bytes(bytes) => decoder.push_bytes(&bytes).unwrap(),
            PollStep::Eof | PollStep::ReadError(_) => unreachable!("success script is complete"),
        }
        while let Some(frame) = decoder.next_frame().unwrap() {
            frames.push((frame.opcode(), frame.payload().to_vec()));
        }
    }
    assert_eq!(clock.sleeps, vec![Duration::from_millis(1)]);
    assert_eq!(frames.len(), 3);
    assert_eq!(
        frames[1],
        (SMSG_TIME_SYNC_REQ, 0x1234_5678_u32.to_le_bytes().to_vec())
    );
}

#[test]
fn poll_script_models_eof_and_typed_read_errors_without_wall_clock_blocking() {
    assert_eq!(
        consume_poll_script(vec![PollStep::Pending, PollStep::Eof]),
        Err(std::io::ErrorKind::UnexpectedEof)
    );
    assert_eq!(
        consume_poll_script(vec![PollStep::ReadError(std::io::ErrorKind::BrokenPipe)]),
        Err(std::io::ErrorKind::BrokenPipe)
    );
}

#[test]
fn encrypted_remote_records_compose_decoder_boundary_fifo_and_snapshot() {
    let guid = 77_u64;
    let script = FrameScript(vec![
        (SMSG_UPDATE_OBJECT, remote_create_body(guid, 0.0, 0.0)),
        (0x7fff, vec![1, 2]),
        (SMSG_UPDATE_OBJECT, remote_update_body(guid, 3.0, -2.0)),
        (
            SMSG_DESTROY_OBJECT,
            [guid.to_le_bytes().as_slice(), &[0]].concat(),
        ),
    ]);
    let wire = script.encrypted();
    let identity = SanitizedIdentity::new(1, "Synthetic Realm", "Observer", 12_340).unwrap();
    let (client, mut boundary) = new_boundary(identity).unwrap();
    let _ = client.drain_events();
    let mut decoder = IncrementalWorldServerDecoder::new(&SYNTHETIC_WORLD_KEY);
    for chunk in wire.chunks(3) {
        decoder.push_bytes(chunk).unwrap();
        while let Some(frame) = decoder.next_frame().unwrap() {
            boundary
                .apply_remote_player_records(decode_remote_player_frame(&frame).unwrap(), Some(0))
                .unwrap();
        }
    }
    let events = client.drain_events();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event.kind, ClientEventKind::RemoteAvatar { .. }))
            .count(),
        3
    );
    assert!(client.snapshot().remote_avatar.is_none());
}

#[test]
fn retained_poll_adapter_drives_fragmented_time_sync_and_clears_remote_on_eof_or_error() {
    let key = SYNTHETIC_WORLD_KEY;
    let time_sync = FrameScript(vec![
        (SMSG_UPDATE_OBJECT, remote_create_body(77, 0.0, 0.0)),
        (SMSG_TIME_SYNC_REQ, 9_u32.to_le_bytes().to_vec()),
        (SMSG_UPDATE_OBJECT, remote_update_body(77, 3.0, -2.0)),
        (
            SMSG_DESTROY_OBJECT,
            [77_u64.to_le_bytes().as_slice(), &[0]].concat(),
        ),
    ])
    .encrypted();
    let identity = SanitizedIdentity::new(1, "Synthetic Realm", "Observer", 12_340).unwrap();
    let (client, mut boundary) = new_boundary(identity.clone()).unwrap();
    let _ = client.drain_events();
    let writes = retained_harness::poll(
        &mut boundary,
        vec![
            retained_harness::PollStep::Bytes(time_sync[..3].to_vec()),
            retained_harness::PollStep::Bytes(time_sync[3..].to_vec()),
        ],
        &key,
        Duration::from_millis(1),
    )
    .unwrap();
    let mut header = writes[..6].to_vec();
    HeaderCipher::new(HeaderDirection::ClientToServer, &key).apply(&mut header);
    assert_eq!(u32::from_le_bytes(header[2..6].try_into().unwrap()), 0x0391);
    assert_eq!(u32::from_le_bytes(writes[6..10].try_into().unwrap()), 9);
    assert_eq!(
        client
            .drain_events()
            .iter()
            .filter(|event| matches!(event.kind, ClientEventKind::RemoteAvatar { .. }))
            .count(),
        3
    );
    boundary
        .apply_remote_player_records(
            vec![RemotePlayerRecord::PlayerCreate {
                guid: 77,
                movement: synthetic_movement(0.0, 0.0),
            }],
            Some(0),
        )
        .unwrap();
    assert!(
        retained_harness::poll(
            &mut boundary,
            vec![retained_harness::PollStep::Eof],
            &key,
            Duration::ZERO,
        )
        .is_err()
    );
    assert!(client.snapshot().remote_avatar.is_none());
    boundary
        .apply_remote_player_records(
            vec![RemotePlayerRecord::PlayerCreate {
                guid: 77,
                movement: synthetic_movement(0.0, 0.0),
            }],
            Some(0),
        )
        .unwrap();
    assert!(
        retained_harness::poll(
            &mut boundary,
            vec![retained_harness::PollStep::ReadError(
                std::io::ErrorKind::BrokenPipe
            )],
            &key,
            Duration::ZERO,
        )
        .is_err()
    );
    assert!(client.snapshot().remote_avatar.is_none());
}

#[test]
fn retained_encrypted_unusable_movement_faults_the_accepted_remote_avatar() {
    let script = FrameScript(vec![
        (SMSG_UPDATE_OBJECT, remote_create_body(77, 0.0, 0.0)),
        (SMSG_UPDATE_OBJECT, remote_unusable_update_body(77)),
    ])
    .encrypted();
    let identity = SanitizedIdentity::new(1, "Synthetic Realm", "Observer", 12_340).unwrap();
    let (client, mut boundary) = new_boundary(identity).unwrap();
    let _ = client.drain_events();
    retained_harness::poll(
        &mut boundary,
        vec![retained_harness::PollStep::Bytes(script)],
        &SYNTHETIC_WORLD_KEY,
        Duration::ZERO,
    )
    .unwrap();
    assert!(matches!(
        client.drain_events().last().map(|event| &event.kind),
        Some(ClientEventKind::RemoteAvatar {
            change: RemoteAvatarChange::Faulted { .. }
        })
    ));
    assert!(client.snapshot().remote_avatar.is_none());
}

#[test]
fn observer_oracle_rejects_every_local_substitute_category() {
    for substitute in [
        LocalSubstitute::MoverLocal,
        LocalSubstitute::Predicted,
        LocalSubstitute::Submitted,
        LocalSubstitute::Rendered,
        LocalSubstitute::Database,
    ] {
        assert_eq!(
            reject_local_substitute(substitute),
            Err(ObserverError::NonRemoteFact)
        );
    }
}

#[test]
fn observer_oracle_accepts_terminal_tolerance_boundary_and_rejects_just_over() {
    let peer = RemoteAvatarId::from_realm_guid(77).unwrap();
    for (delta, expected) in [
        (0.25, Ok(())),
        (0.250_001, Err(ObserverError::TerminalMismatch)),
    ] {
        let mut scenario = ObserverScenario::new(peer, pose(3.0, 0.0));
        for (sequence, change) in [
            (
                1,
                RemoteAvatarChange::Created {
                    id: peer,
                    realm_observed_pose: pose(0.0, 0.0),
                },
            ),
            (
                2,
                RemoteAvatarChange::Updated {
                    id: peer,
                    realm_observed_pose: pose(3.0 + delta, 0.0),
                },
            ),
            (
                3,
                RemoteAvatarChange::Removed {
                    id: peer,
                    source: RemoteAvatarRemovalSource::DestroyObject,
                },
            ),
        ] {
            scenario
                .record(remote_event(sequence, change), Duration::ZERO)
                .unwrap();
        }
        assert_eq!(scenario.assert_success(), expected);
    }
}

#[test]
fn harness_backpressure_fence_clears_remote_truth_and_fresh_retry_has_no_old_facts() {
    let identity = SanitizedIdentity::new(1, "Synthetic Realm", "Observer", 12_340).unwrap();
    let (client, mut boundary) = new_boundary(identity.clone()).unwrap();
    let _ = client.drain_events();
    boundary
        .apply_remote_player_records(
            vec![RemotePlayerRecord::PlayerCreate {
                guid: 77,
                movement: synthetic_movement(0.0, 0.0),
            }],
            Some(0),
        )
        .unwrap();
    for index in 1..client_session_event_capacity() {
        boundary
            .apply_remote_player_records(
                vec![RemotePlayerRecord::PlayerMovement {
                    guid: 77,
                    movement: synthetic_movement(f32::from(u8::try_from(index).unwrap()), 0.0),
                    opcode: SERVER_MOVE_STOP,
                }],
                Some(0),
            )
            .unwrap();
    }
    assert!(
        boundary
            .apply_remote_player_records(
                vec![RemotePlayerRecord::PlayerMovement {
                    guid: 77,
                    movement: synthetic_movement(99.0, 0.0),
                    opcode: SERVER_MOVE_STOP
                }],
                Some(0)
            )
            .is_err()
    );
    let failed = client.snapshot();
    assert!(failed.remote_avatar.is_none());
    assert!(failed.remote_avatar_invalidated_through > 0);
    let (retry, mut retry_boundary) = new_boundary(identity).unwrap();
    let _ = retry.drain_events();
    assert!(retry.snapshot().remote_avatar.is_none());
    retry_boundary
        .apply_remote_player_records(
            vec![RemotePlayerRecord::PlayerCreate {
                guid: 78,
                movement: synthetic_movement(0.0, 0.0),
            }],
            Some(0),
        )
        .unwrap();
    assert_eq!(retry.snapshot().remote_avatar.unwrap().id.realm_guid(), 78);
}

const fn client_session_event_capacity() -> usize {
    crate::EVENT_CAPACITY
}

#[test]
fn retained_boundary_rejects_foreign_and_duplicate_remote_lifecycle_without_false_truth() {
    let first = 77_u64;
    let foreign = 78_u64;
    let identity = SanitizedIdentity::new(1, "Synthetic Realm", "Observer", 12_340).unwrap();
    let (client, mut boundary) = new_boundary(identity).unwrap();
    let _ = client.drain_events();
    boundary
        .apply_remote_player_records(
            vec![RemotePlayerRecord::PlayerCreate {
                guid: first,
                movement: synthetic_movement(0.0, 0.0),
            }],
            Some(0),
        )
        .unwrap();
    boundary
        .apply_remote_player_records(
            vec![
                RemotePlayerRecord::PlayerMovement {
                    guid: foreign,
                    movement: synthetic_movement(9.0, 9.0),
                    opcode: SERVER_MOVE_STOP,
                },
                RemotePlayerRecord::Destroy { guid: foreign },
                RemotePlayerRecord::PlayerCreate {
                    guid: foreign,
                    movement: synthetic_movement(9.0, 9.0),
                },
                RemotePlayerRecord::PlayerCreate {
                    guid: first,
                    movement: synthetic_movement(0.0, 0.0),
                },
            ],
            Some(0),
        )
        .unwrap();
    let events = client.drain_events();
    assert!(matches!(
        events.last().map(|event| &event.kind),
        Some(ClientEventKind::RemoteAvatar {
            change: RemoteAvatarChange::Faulted { .. }
        })
    ));
    assert!(client.snapshot().remote_avatar.is_none());
}

#[test]
fn observer_oracle_accepts_exact_deadlines_and_terminal_realm_pose_only() {
    let peer = RemoteAvatarId::from_realm_guid(77).unwrap();
    let target = pose(3.0, -2.0);
    let mut scenario = RetainedObserverScenario::new(peer, target);
    scenario.advance_to(FIRST_POSE_DEADLINE);
    scenario
        .apply_and_poll(RemotePlayerRecord::PlayerCreate {
            guid: peer.realm_guid(),
            movement: synthetic_movement(0.0, 0.0),
        })
        .unwrap();
    scenario.advance_to(TERMINAL_POSE_DEADLINE);
    scenario
        .apply_and_poll(RemotePlayerRecord::PlayerMovement {
            guid: peer.realm_guid(),
            movement: synthetic_movement(3.2, -2.0),
            opcode: SERVER_MOVE_STOP,
        })
        .unwrap();
    scenario.advance_to(REMOVAL_DEADLINE);
    scenario
        .apply_and_poll(RemotePlayerRecord::Destroy {
            guid: peer.realm_guid(),
        })
        .unwrap();
    assert_eq!(scenario.assert_success(), Ok(()));
    assert!(scenario.snapshot().remote_avatar.is_none());
}

#[test]
fn observer_oracle_rejects_late_or_non_observer_substitutes_and_resets_for_retry() {
    let peer = RemoteAvatarId::from_realm_guid(77).unwrap();
    let target = pose(3.0, -2.0);
    let mut scenario = ObserverScenario::new(peer, target);
    assert_eq!(
        scenario.record(
            ClientEvent {
                sequence: 1,
                kind: ClientEventKind::MovementSubmitted {
                    pose: target,
                    stopped: true,
                },
            },
            Duration::ZERO,
        ),
        Err(ObserverError::NonRemoteFact)
    );
    assert_eq!(
        scenario.record(
            remote_event(
                1,
                RemoteAvatarChange::Created {
                    id: peer,
                    realm_observed_pose: pose(0.0, 0.0),
                },
            ),
            FIRST_POSE_DEADLINE + Duration::from_millis(1),
        ),
        Ok(())
    );
    assert_eq!(
        scenario.assert_success(),
        Err(ObserverError::MissingTerminalUpdate)
    );
    scenario.retry();
    let wrong = RemoteAvatarId::from_realm_guid(78).unwrap();
    assert_eq!(
        scenario.record(
            remote_event(
                1,
                RemoteAvatarChange::Created {
                    id: wrong,
                    realm_observed_pose: pose(0.0, 0.0),
                },
            ),
            Duration::ZERO,
        ),
        Err(ObserverError::WrongGuid)
    );
    assert_eq!(scenario.facts.len(), 0);
}

#[test]
#[allow(clippy::too_many_lines)]
fn observer_oracle_rejects_every_deadline_plus_one_and_faulted_or_map_mismatched_truth() {
    let peer = RemoteAvatarId::from_realm_guid(77).unwrap();
    let target = pose(3.0, -2.0);
    for (created_at, terminal_at, removal_at, expected) in [
        (
            FIRST_POSE_DEADLINE + Duration::from_millis(1),
            TERMINAL_POSE_DEADLINE,
            REMOVAL_DEADLINE,
            ObserverError::FirstPoseLate,
        ),
        (
            FIRST_POSE_DEADLINE,
            TERMINAL_POSE_DEADLINE + Duration::from_millis(1),
            REMOVAL_DEADLINE,
            ObserverError::TerminalPoseLate,
        ),
        (
            FIRST_POSE_DEADLINE,
            TERMINAL_POSE_DEADLINE,
            REMOVAL_DEADLINE + Duration::from_millis(1),
            ObserverError::RemovalLate,
        ),
    ] {
        let mut scenario = ObserverScenario::new(peer, target);
        scenario
            .record(
                remote_event(
                    1,
                    RemoteAvatarChange::Created {
                        id: peer,
                        realm_observed_pose: pose(0.0, 0.0),
                    },
                ),
                created_at,
            )
            .unwrap();
        scenario
            .record(
                remote_event(
                    2,
                    RemoteAvatarChange::Updated {
                        id: peer,
                        realm_observed_pose: target,
                    },
                ),
                terminal_at,
            )
            .unwrap();
        scenario
            .record(
                remote_event(
                    3,
                    RemoteAvatarChange::Removed {
                        id: peer,
                        source: RemoteAvatarRemovalSource::OutOfRange,
                    },
                ),
                removal_at,
            )
            .unwrap();
        assert_eq!(scenario.assert_success(), Err(expected));
    }
    let mut scenario = ObserverScenario::new(peer, target);
    assert_eq!(
        scenario.record(
            remote_event(
                1,
                RemoteAvatarChange::Faulted {
                    id: peer,
                    category: crate::RemoteAvatarFaultCategory::InvalidPose,
                },
            ),
            Duration::ZERO,
        ),
        Err(ObserverError::Faulted)
    );
    let mut wrong_map = target;
    wrong_map.map_id = 1;
    let mut scenario = ObserverScenario::new(peer, target);
    for (sequence, change, instant) in [
        (
            1,
            RemoteAvatarChange::Created {
                id: peer,
                realm_observed_pose: pose(0.0, 0.0),
            },
            Duration::ZERO,
        ),
        (
            2,
            RemoteAvatarChange::Updated {
                id: peer,
                realm_observed_pose: wrong_map,
            },
            Duration::ZERO,
        ),
        (
            3,
            RemoteAvatarChange::Removed {
                id: peer,
                source: RemoteAvatarRemovalSource::DestroyObject,
            },
            Duration::ZERO,
        ),
    ] {
        scenario
            .record(remote_event(sequence, change), instant)
            .unwrap();
    }
    assert_eq!(scenario.assert_success(), Err(ObserverError::MapMismatch));
}

#[test]
fn observer_oracle_rejects_stale_and_unordered_lifecycle_facts() {
    let peer = RemoteAvatarId::from_realm_guid(77).unwrap();
    let target = pose(3.0, -2.0);
    let mut scenario = ObserverScenario::new(peer, target);
    scenario
        .record(
            remote_event(
                2,
                RemoteAvatarChange::Updated {
                    id: peer,
                    realm_observed_pose: target,
                },
            ),
            Duration::ZERO,
        )
        .unwrap();
    scenario
        .record(
            remote_event(
                3,
                RemoteAvatarChange::Created {
                    id: peer,
                    realm_observed_pose: pose(0.0, 0.0),
                },
            ),
            Duration::ZERO,
        )
        .unwrap();
    scenario
        .record(
            remote_event(
                4,
                RemoteAvatarChange::Removed {
                    id: peer,
                    source: RemoteAvatarRemovalSource::DestroyObject,
                },
            ),
            Duration::ZERO,
        )
        .unwrap();
    assert_eq!(
        scenario.assert_success(),
        Err(ObserverError::UnorderedLifecycle)
    );
    assert_eq!(
        scenario.record(
            remote_event(
                4,
                RemoteAvatarChange::Created {
                    id: peer,
                    realm_observed_pose: pose(0.0, 0.0),
                },
            ),
            Duration::ZERO,
        ),
        Err(ObserverError::OutOfOrder)
    );
}
