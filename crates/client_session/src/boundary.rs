use std::{
    error::Error,
    fmt,
    path::PathBuf,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    time::Duration,
};

use client_protocol::{RemotePlayerRecord, RemotePlayerUnusableCategory};

use crate::{
    CONTROL_CAPACITY, ClientEvent, ClientEventKind, ClientFailure, ClientPhase, ClientSnapshot,
    CommandKind, ControlCommand, CorrectionTarget, DiscoveredRealm, EVENT_CAPACITY,
    FailureCategory, MovementIntent, MovementProofEvidence, PoseSource, ProofStage, QueueCounters,
    Recovery, RecoveryAction, RemoteAvatarChange, RemoteAvatarFaultCategory, RemoteAvatarId,
    RemoteAvatarRemovalSource, RemoteAvatarSnapshot, SanitizedIdentity, SelectedCharacter,
    SemanticDiagnostic, WorldPose,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoundaryError {
    ControlBackpressure,
    EventBackpressure,
    InputGated,
    WorkerStopped,
    WorkerPanicked,
}

impl fmt::Display for BoundaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ControlBackpressure => {
                formatter.write_str("control queue reached its lossless capacity")
            }
            Self::EventBackpressure => {
                formatter.write_str("event queue reached its lossless capacity")
            }
            Self::InputGated => {
                formatter.write_str("movement input is gated while a proof is in progress")
            }
            Self::WorkerStopped => formatter.write_str("session worker has stopped"),
            Self::WorkerPanicked => formatter.write_str("session worker panicked"),
        }
    }
}

impl Error for BoundaryError {}

#[derive(Default)]
pub(crate) struct BoundaryCounters {
    control_queued: AtomicUsize,
    event_queued: AtomicUsize,
    movement_revision: AtomicU64,
    snapshot_revision: AtomicU64,
}

impl BoundaryCounters {
    fn snapshot(&self) -> QueueCounters {
        QueueCounters {
            control_queued: self.control_queued.load(Ordering::Acquire),
            event_queued: self.event_queued.load(Ordering::Acquire),
            movement_revision: self.movement_revision.load(Ordering::Acquire),
            snapshot_revision: self.snapshot_revision.load(Ordering::Acquire),
        }
    }
}

pub(crate) struct SessionClient {
    control: SyncSender<ControlCommand>,
    events: Mutex<Receiver<ClientEvent>>,
    movement: Arc<Mutex<MovementIntent>>,
    proof_input_frozen: Arc<AtomicBool>,
    snapshot: Arc<RwLock<ClientSnapshot>>,
    counters: Arc<BoundaryCounters>,
    shutdown: Arc<AtomicBool>,
    worker_stopped: Arc<AtomicBool>,
}

impl SessionClient {
    pub(crate) fn send_control(&self, command: ControlCommand) -> Result<(), BoundaryError> {
        if self.shutdown.load(Ordering::Acquire) || self.worker_stopped.load(Ordering::Acquire) {
            return Err(BoundaryError::WorkerStopped);
        }
        self.counters.control_queued.fetch_add(1, Ordering::AcqRel);
        match self.control.try_send(command) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => {
                self.counters.control_queued.fetch_sub(1, Ordering::AcqRel);
                record_backpressure_failure(
                    &self.snapshot,
                    &self.counters,
                    "control FIFO reached capacity",
                );
                self.shutdown.store(true, Ordering::Release);
                Err(BoundaryError::ControlBackpressure)
            }
            Err(TrySendError::Disconnected(_)) => {
                self.counters.control_queued.fetch_sub(1, Ordering::AcqRel);
                Err(BoundaryError::WorkerStopped)
            }
        }
    }

    pub(crate) fn publish_movement_intent(
        &self,
        intent: MovementIntent,
    ) -> Result<(), BoundaryError> {
        if self.proof_input_frozen.load(Ordering::Acquire) {
            return Err(BoundaryError::InputGated);
        }
        let mut movement = self.movement.lock().expect("movement mailbox poisoned");
        // The proof worker sets this gate before it drains the FIFO. Checking
        // under the same mailbox lock makes a racing input either visible to
        // that drain or rejected without publishing a new transition.
        if self.proof_input_frozen.load(Ordering::Acquire) {
            return Err(BoundaryError::InputGated);
        }
        let was_engaged = movement.engaged();
        *movement = intent;
        drop(movement);
        self.counters
            .movement_revision
            .fetch_add(1, Ordering::AcqRel);
        if was_engaged != intent.engaged() {
            if self.proof_input_frozen.load(Ordering::Acquire) {
                return Err(BoundaryError::InputGated);
            }
            self.send_control(ControlCommand::MovementTransition {
                engaged: intent.engaged(),
            })?;
        }
        Ok(())
    }

    pub(crate) fn drain_events(&self) -> Vec<ClientEvent> {
        let receiver = self.events.lock().expect("event receiver poisoned");
        let mut drained = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            self.counters.event_queued.fetch_sub(1, Ordering::AcqRel);
            drained.push(event);
        }
        drained
    }

    pub(crate) fn snapshot(&self) -> ClientSnapshot {
        let mut snapshot = self
            .snapshot
            .read()
            .expect("client snapshot poisoned")
            .clone();
        snapshot.queue_counters = self.counters.snapshot();
        snapshot
    }

    pub(crate) fn request_shutdown(&self) {
        self.shutdown.store(true, Ordering::Release);
        self.counters.control_queued.fetch_add(1, Ordering::AcqRel);
        if self.control.try_send(ControlCommand::Disconnect).is_err() {
            self.counters.control_queued.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

pub(crate) struct WorkerBoundary {
    control: Receiver<ControlCommand>,
    events: SyncSender<ClientEvent>,
    snapshot: Arc<RwLock<ClientSnapshot>>,
    counters: Arc<BoundaryCounters>,
    shutdown: Arc<AtomicBool>,
    worker_stopped: Arc<AtomicBool>,
    movement: Arc<Mutex<MovementIntent>>,
    proof_input_frozen: Arc<AtomicBool>,
    last_submitted_was_stop: bool,
    event_sequence: u64,
    accepted_remote_id: Option<RemoteAvatarId>,
    proof_stage_output: Option<PathBuf>,
}

impl WorkerBoundary {
    pub(crate) fn receive_control(
        &self,
        timeout: Duration,
    ) -> Result<ControlCommand, mpsc::RecvTimeoutError> {
        self.control.recv_timeout(timeout)
    }

    pub(crate) fn control_consumed(&self) {
        self.counters.control_queued.fetch_sub(1, Ordering::AcqRel);
    }

    pub(crate) fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }

    /// Remove stale movement edges after a proof has frozen input. A queued
    /// disconnect is never input: retain its semantic effect by setting the
    /// shutdown boundary so the active loop exits before it can begin a new
    /// proof/reconnect operation.
    pub(crate) fn discard_stale_movement_controls(&self) {
        while let Ok(command) = self.control.try_recv() {
            self.counters.control_queued.fetch_sub(1, Ordering::AcqRel);
            if command == ControlCommand::Disconnect {
                self.shutdown.store(true, Ordering::Release);
            }
        }
    }

    pub(crate) fn latest_movement_intent(&self) -> MovementIntent {
        *self.movement.lock().expect("movement mailbox poisoned")
    }

    pub(crate) fn begin_movement_proof(&mut self) -> Option<WorldPose> {
        // Freeze before examining the oracle. A concurrent publisher either
        // observes this gate or is reduced to idle before this function exits.
        self.proof_input_frozen.store(true, Ordering::Release);
        let mut current = self.snapshot.write().expect("client snapshot poisoned");
        let anchor = current.entry_anchor?;
        let expected = current.submitted_pose?;
        let displacement = (expected.east - anchor.east).hypot(expected.north - anchor.north);
        if displacement < 2.0
            || !self.last_submitted_was_stop
            || self.latest_movement_intent().engaged()
        {
            self.proof_input_frozen.store(false, Ordering::Release);
            return None;
        }
        current.movement_proof = Some(MovementProofEvidence {
            expected,
            observed: None,
            tolerance_metres: 0.25,
        });
        current.phase = ClientPhase::ProvingMovement(ProofStage::SavingLogout);
        self.counters
            .snapshot_revision
            .fetch_add(1, Ordering::AcqRel);
        drop(current);
        *self.movement.lock().expect("movement mailbox poisoned") = MovementIntent::idle();
        // Movement edges after BeginMovementProof belong to a now-invalid
        // input epoch. A concurrent disconnect remains a shutdown request,
        // which the caller observes before it starts logout or reconnect.
        self.discard_stale_movement_controls();
        self.publish(ClientEventKind::PhaseChanged {
            phase: ClientPhase::ProvingMovement(ProofStage::SavingLogout),
        })
        .then_some(expected)
    }

    pub(crate) fn proof_stage(&mut self, stage: ProofStage) -> bool {
        if stage == ProofStage::Reconnecting
            && let Some(path) = &self.proof_stage_output
        {
            let _ = std::fs::write(path, "reconnecting\n");
        }
        self.transition(ClientPhase::ProvingMovement(stage))
    }

    /// Return the test-only external release marker associated with a
    /// repository-local proof-stage marker. Normal sessions have no such
    /// marker and therefore cannot be paused by this adapter.
    pub(crate) fn reconnect_release_marker(&self) -> Option<PathBuf> {
        self.proof_stage_output
            .as_ref()
            .map(|stage| stage.with_extension("ack"))
    }

    pub(crate) fn observe_reconnect_pose(&mut self, pose: WorldPose) -> bool {
        {
            let mut current = self.snapshot.write().expect("client snapshot poisoned");
            current.realm_observed_pose = Some(pose);
            if let Some(proof) = current.movement_proof.as_mut() {
                proof.observed = Some(pose);
            }
            self.counters
                .snapshot_revision
                .fetch_add(1, Ordering::AcqRel);
        }
        self.publish(ClientEventKind::PoseObserved {
            source: PoseSource::ReconnectObservation,
            pose,
        })
    }

    /// Finish the sole accepted persistence comparison.  It reads only the
    /// frozen submitted oracle and the fresh reconnect observation already
    /// projected into the snapshot; database or log evidence cannot reach it.
    pub(crate) fn complete_movement_proof(&mut self, expected: WorldPose) -> bool {
        let proof = self
            .snapshot
            .read()
            .expect("client snapshot poisoned")
            .movement_proof;
        proof.is_some_and(|proof| proof.expected == expected && proof.passed())
    }

    pub(crate) fn transition(&mut self, phase: ClientPhase) -> bool {
        {
            let mut current = self.snapshot.write().expect("client snapshot poisoned");
            current.phase = phase.clone();
            self.counters
                .snapshot_revision
                .fetch_add(1, Ordering::AcqRel);
        }
        self.publish(ClientEventKind::PhaseChanged { phase })
    }

    pub(crate) fn discovered(&mut self, realm: DiscoveredRealm) -> bool {
        {
            let mut current = self.snapshot.write().expect("client snapshot poisoned");
            current.discovered_realm = Some(realm.clone());
            current.latest_failure = None;
            self.counters
                .snapshot_revision
                .fetch_add(1, Ordering::AcqRel);
        }
        self.publish(ClientEventKind::RealmDiscovered { realm })
    }

    pub(crate) fn selected(&mut self, character: SelectedCharacter) -> bool {
        {
            let mut current = self.snapshot.write().expect("client snapshot poisoned");
            current.selected_character = Some(character.clone());
            current.latest_failure = None;
            self.counters
                .snapshot_revision
                .fetch_add(1, Ordering::AcqRel);
        }
        self.publish(ClientEventKind::CharacterSelected { character })
    }

    pub(crate) fn observe_entry_anchor(&mut self, pose: WorldPose) -> bool {
        {
            let mut current = self.snapshot.write().expect("client snapshot poisoned");
            current.entry_anchor = Some(pose);
            // The entry baseline is the initial submitted truth. It is not a
            // movement publication and therefore deliberately emits no
            // `MovementSubmitted` event or movement revision.
            current.submitted_pose = Some(pose);
            current.submitted_pose_is_stopped = false;
            current.realm_observed_pose = Some(pose);
            self.counters
                .snapshot_revision
                .fetch_add(1, Ordering::AcqRel);
        }
        self.publish(ClientEventKind::PoseObserved {
            source: PoseSource::EntryObservation,
            pose,
        })
    }

    pub(crate) fn movement_ready(&mut self, run_speed: f32) -> bool {
        // A reconnect reaches MovementReady before the outer proof worker
        // compares its fresh pose. Keep its input gate closed throughout that
        // interval; only explicit retry (or a newly ordinary session) opens it.
        let proof_active = self
            .snapshot
            .read()
            .expect("client snapshot poisoned")
            .movement_proof
            .is_some();
        if !proof_active {
            self.proof_input_frozen.store(false, Ordering::Release);
        }
        {
            let mut current = self.snapshot.write().expect("client snapshot poisoned");
            current.run_speed = Some(run_speed);
            current.latest_failure = None;
            self.counters
                .snapshot_revision
                .fetch_add(1, Ordering::AcqRel);
        }
        self.transition(ClientPhase::MovementReady)
    }

    pub(crate) fn predict_movement(&mut self, pose: WorldPose) {
        let mut current = self.snapshot.write().expect("client snapshot poisoned");
        current.predicted_pose = Some(pose);
        self.counters
            .snapshot_revision
            .fetch_add(1, Ordering::AcqRel);
    }

    pub(crate) fn movement_submitted_state(&mut self, pose: WorldPose, stopped: bool) -> bool {
        self.last_submitted_was_stop = stopped;
        {
            let mut current = self.snapshot.write().expect("client snapshot poisoned");
            current.submitted_pose = Some(pose);
            current.submitted_pose_is_stopped = stopped;
            self.counters
                .snapshot_revision
                .fetch_add(1, Ordering::AcqRel);
        }
        self.publish(ClientEventKind::MovementSubmitted { pose, stopped })
    }

    /// Publish a scripted correction target without mutating realm-observed
    /// truth.  The retained wire protocol deliberately has no correction
    /// decoder in this slice.
    #[allow(dead_code)] // exercised by the internal scripted-correction test boundary
    pub(crate) fn scripted_correction_target(&mut self, target: CorrectionTarget) -> bool {
        {
            let mut current = self.snapshot.write().expect("client snapshot poisoned");
            current.correction_target = Some(target);
            self.counters
                .snapshot_revision
                .fetch_add(1, Ordering::AcqRel);
        }
        self.publish(ClientEventKind::ScriptedCorrection { target })
    }

    pub(crate) fn reset_for_retry(&mut self) {
        self.last_submitted_was_stop = false;
        self.proof_input_frozen.store(false, Ordering::Release);
        let mut current = self.snapshot.write().expect("client snapshot poisoned");
        current.phase = ClientPhase::Offline;
        current.discovered_realm = None;
        current.selected_character = None;
        current.entry_anchor = None;
        current.predicted_pose = None;
        current.submitted_pose = None;
        current.submitted_pose_is_stopped = false;
        current.realm_observed_pose = None;
        current.correction_target = None;
        current.remote_avatar = None;
        current.movement_proof = None;
        current.run_speed = None;
        current.latest_failure = None;
        self.counters
            .snapshot_revision
            .fetch_add(1, Ordering::AcqRel);
        self.accepted_remote_id = None;
    }

    pub(crate) fn reject(&mut self, command: CommandKind, failure: ClientFailure) -> bool {
        {
            let mut current = self.snapshot.write().expect("client snapshot poisoned");
            current.latest_failure = Some(failure.clone());
            self.counters
                .snapshot_revision
                .fetch_add(1, Ordering::AcqRel);
        }
        self.publish(ClientEventKind::CommandRejected { command, failure })
    }

    pub(crate) fn fail(&mut self, command: CommandKind, failure: ClientFailure) {
        let recovery = Recovery {
            category: failure.category(),
            action: failure.recommended_recovery(),
        };
        {
            let mut current = self.snapshot.write().expect("client snapshot poisoned");
            current.phase = ClientPhase::Failed(recovery);
            current.remote_avatar = None;
            current.latest_failure = Some(failure.clone());
            push_diagnostic(&mut current, |sequence| {
                SemanticDiagnostic::from_failure(sequence, &failure)
            });
            self.counters
                .snapshot_revision
                .fetch_add(1, Ordering::AcqRel);
        }
        self.accepted_remote_id = None;
        let _ = self.publish(ClientEventKind::PhaseChanged {
            phase: ClientPhase::Failed(recovery),
        }) && self.publish(ClientEventKind::CommandRejected { command, failure })
            && self.publish(ClientEventKind::Disconnected);
    }

    pub(crate) fn disconnect(&mut self) {
        let _ =
            self.transition(ClientPhase::Offline) && self.publish(ClientEventKind::Disconnected);
    }

    pub(crate) fn mark_stopped(&self) {
        self.worker_stopped.store(true, Ordering::Release);
    }

    pub(crate) fn clear_remote_avatar_for_session_failure(&mut self) {
        let mut current = self.snapshot.write().expect("client snapshot poisoned");
        current.remote_avatar = None;
        self.counters
            .snapshot_revision
            .fetch_add(1, Ordering::AcqRel);
        self.accepted_remote_id = None;
    }

    pub(crate) fn apply_remote_player_records(
        &mut self,
        records: Vec<RemotePlayerRecord>,
        entry_map: Option<u32>,
    ) -> Result<(), BoundaryError> {
        for record in records {
            self.apply_remote_player_record(record, entry_map)?;
        }
        Ok(())
    }

    fn apply_remote_player_record(
        &mut self,
        record: RemotePlayerRecord,
        entry_map: Option<u32>,
    ) -> Result<(), BoundaryError> {
        let id =
            RemoteAvatarId::from_realm_guid(record.guid()).ok_or(BoundaryError::WorkerStopped)?;
        match record {
            RemotePlayerRecord::PlayerCreate { movement, .. } => match self.accepted_remote_id {
                None => {
                    let map_id = entry_map.ok_or_else(|| self.remote_map_missing())?;
                    self.publish_remote(RemoteAvatarChange::Created {
                        id,
                        realm_observed_pose: remote_pose(map_id, movement),
                    })?;
                    self.accepted_remote_id = Some(id);
                }
                Some(current) if current == id => {
                    self.publish_remote(RemoteAvatarChange::Faulted {
                        id,
                        category: RemoteAvatarFaultCategory::InconsistentLifecycle,
                    })?;
                    self.accepted_remote_id = None;
                }
                Some(_) => {}
            },
            RemotePlayerRecord::PlayerMovement { movement, .. }
                if self.accepted_remote_id == Some(id) =>
            {
                let map_id = entry_map.ok_or_else(|| self.remote_map_missing())?;
                self.publish_remote(RemoteAvatarChange::Updated {
                    id,
                    realm_observed_pose: remote_pose(map_id, movement),
                })?;
            }
            RemotePlayerRecord::OutOfRange { .. } if self.accepted_remote_id == Some(id) => {
                self.publish_remote(RemoteAvatarChange::Removed {
                    id,
                    source: RemoteAvatarRemovalSource::OutOfRange,
                })?;
                self.accepted_remote_id = None;
            }
            RemotePlayerRecord::Destroy { .. } if self.accepted_remote_id == Some(id) => {
                self.publish_remote(RemoteAvatarChange::Removed {
                    id,
                    source: RemoteAvatarRemovalSource::DestroyObject,
                })?;
                self.accepted_remote_id = None;
            }
            RemotePlayerRecord::UnusableMovement { category, .. }
                if self.accepted_remote_id == Some(id) =>
            {
                let category = match category {
                    RemotePlayerUnusableCategory::InvalidPose => {
                        RemoteAvatarFaultCategory::InvalidPose
                    }
                    RemotePlayerUnusableCategory::UnsupportedMovement => {
                        RemoteAvatarFaultCategory::UnsupportedMovement
                    }
                };
                self.publish_remote(RemoteAvatarChange::Faulted { id, category })?;
                self.accepted_remote_id = None;
            }
            _ => {}
        }
        Ok(())
    }

    fn remote_map_missing(&mut self) -> BoundaryError {
        let failure = ClientFailure::new(
            FailureCategory::ProtocolIncompatibility,
            "remote avatar publication",
            "remote avatar requires entry map",
            RecoveryAction::CheckReferenceRealm,
        );
        let mut current = self.snapshot.write().expect("client snapshot poisoned");
        current.remote_avatar = None;
        current.phase = ClientPhase::Failed(Recovery {
            category: FailureCategory::ProtocolIncompatibility,
            action: RecoveryAction::CheckReferenceRealm,
        });
        current.latest_failure = Some(failure.clone());
        push_diagnostic(&mut current, |sequence| {
            SemanticDiagnostic::from_failure(sequence, &failure)
        });
        self.counters
            .snapshot_revision
            .fetch_add(1, Ordering::AcqRel);
        self.accepted_remote_id = None;
        self.shutdown.store(true, Ordering::Release);
        BoundaryError::WorkerStopped
    }

    fn publish_remote(&mut self, change: RemoteAvatarChange) -> Result<(), BoundaryError> {
        let sequence = self.event_sequence.saturating_add(1);
        let event = ClientEvent {
            sequence,
            kind: ClientEventKind::RemoteAvatar { change },
        };
        let mut current = self.snapshot.write().expect("client snapshot poisoned");
        match emit_event(&self.events, &self.counters, event) {
            Ok(()) => {
                self.event_sequence = sequence;
                match change {
                    RemoteAvatarChange::Created {
                        id,
                        realm_observed_pose,
                    }
                    | RemoteAvatarChange::Updated {
                        id,
                        realm_observed_pose,
                    } => {
                        current.remote_avatar = Some(RemoteAvatarSnapshot {
                            id,
                            realm_observed_pose,
                            source_sequence: sequence,
                        });
                    }
                    RemoteAvatarChange::Removed { .. } | RemoteAvatarChange::Faulted { .. } => {
                        current.remote_avatar = None;
                    }
                }
                self.counters
                    .snapshot_revision
                    .fetch_add(1, Ordering::AcqRel);
                Ok(())
            }
            Err(BoundaryError::EventBackpressure) => {
                drop(current);
                self.handle_event_backpressure();
                Err(BoundaryError::EventBackpressure)
            }
            Err(error) => Err(error),
        }
    }

    fn publish(&mut self, kind: ClientEventKind) -> bool {
        let sequence = self.event_sequence.saturating_add(1);
        let event = ClientEvent { sequence, kind };
        match emit_event(&self.events, &self.counters, event) {
            Ok(()) => {
                self.event_sequence = sequence;
                true
            }
            Err(BoundaryError::EventBackpressure) => {
                self.clear_remote_avatar_for_session_failure();
                record_backpressure_failure(
                    &self.snapshot,
                    &self.counters,
                    "event FIFO reached capacity",
                );
                self.shutdown.store(true, Ordering::Release);
                false
            }
            Err(
                BoundaryError::WorkerStopped
                | BoundaryError::WorkerPanicked
                | BoundaryError::ControlBackpressure
                | BoundaryError::InputGated,
            ) => false,
        }
    }

    fn handle_event_backpressure(&mut self) {
        {
            let mut current = self.snapshot.write().expect("client snapshot poisoned");
            current.remote_avatar = None;
            current.remote_avatar_invalidated_through = self.event_sequence;
        }
        self.accepted_remote_id = None;
        record_backpressure_failure(
            &self.snapshot,
            &self.counters,
            "event FIFO reached capacity",
        );
        self.shutdown.store(true, Ordering::Release);
    }
}

pub(crate) fn new_boundary(
    identity: SanitizedIdentity,
) -> Result<(SessionClient, WorkerBoundary), BoundaryError> {
    new_boundary_with_proof_stage(identity, None)
}

pub(crate) fn new_boundary_with_proof_stage(
    identity: SanitizedIdentity,
    proof_stage_output: Option<PathBuf>,
) -> Result<(SessionClient, WorkerBoundary), BoundaryError> {
    let (control_sender, control_receiver) = mpsc::sync_channel(CONTROL_CAPACITY);
    let (event_sender, event_receiver) = mpsc::sync_channel(EVENT_CAPACITY);
    let counters = Arc::new(BoundaryCounters::default());
    let movement = Arc::new(Mutex::new(MovementIntent::idle()));
    let proof_input_frozen = Arc::new(AtomicBool::new(false));
    let shutdown = Arc::new(AtomicBool::new(false));
    let worker_stopped = Arc::new(AtomicBool::new(false));
    let snapshot = Arc::new(RwLock::new(ClientSnapshot::offline(identity.clone())));

    emit_event(
        &event_sender,
        &counters,
        ClientEvent {
            sequence: 1,
            kind: ClientEventKind::IdentityConfigured { identity },
        },
    )?;
    emit_event(
        &event_sender,
        &counters,
        ClientEvent {
            sequence: 2,
            kind: ClientEventKind::PhaseChanged {
                phase: ClientPhase::Offline,
            },
        },
    )?;

    Ok((
        SessionClient {
            control: control_sender,
            events: Mutex::new(event_receiver),
            movement: Arc::clone(&movement),
            proof_input_frozen: Arc::clone(&proof_input_frozen),
            snapshot: Arc::clone(&snapshot),
            counters: Arc::clone(&counters),
            shutdown: Arc::clone(&shutdown),
            worker_stopped: Arc::clone(&worker_stopped),
        },
        WorkerBoundary {
            control: control_receiver,
            events: event_sender,
            snapshot,
            counters,
            shutdown,
            worker_stopped,
            movement,
            proof_input_frozen,
            last_submitted_was_stop: false,
            event_sequence: 2,
            accepted_remote_id: None,
            proof_stage_output,
        },
    ))
}

fn remote_pose(map_id: u32, movement: client_protocol::AcoreMovementInfo) -> WorldPose {
    let [east, north, elevation] = movement.position();
    WorldPose {
        map_id,
        east,
        north,
        elevation,
        orientation: movement.orientation(),
    }
}

fn emit_event(
    sender: &SyncSender<ClientEvent>,
    counters: &BoundaryCounters,
    event: ClientEvent,
) -> Result<(), BoundaryError> {
    counters.event_queued.fetch_add(1, Ordering::AcqRel);
    match sender.try_send(event) {
        Ok(()) => Ok(()),
        Err(TrySendError::Full(_)) => {
            counters.event_queued.fetch_sub(1, Ordering::AcqRel);
            Err(BoundaryError::EventBackpressure)
        }
        Err(TrySendError::Disconnected(_)) => {
            counters.event_queued.fetch_sub(1, Ordering::AcqRel);
            Err(BoundaryError::WorkerStopped)
        }
    }
}

fn record_backpressure_failure(
    snapshot: &RwLock<ClientSnapshot>,
    counters: &BoundaryCounters,
    context: &'static str,
) {
    let failure = ClientFailure::new(
        FailureCategory::InternalBackpressure,
        "application boundary",
        context,
        RecoveryAction::RestartClient,
    );
    let mut current = snapshot.write().expect("client snapshot poisoned");
    current.phase = ClientPhase::Failed(Recovery {
        category: FailureCategory::InternalBackpressure,
        action: RecoveryAction::RestartClient,
    });
    current.latest_failure = Some(failure);
    push_diagnostic(&mut current, |sequence| {
        SemanticDiagnostic::new(sequence, context)
    });
    counters.snapshot_revision.fetch_add(1, Ordering::AcqRel);
}

fn push_diagnostic(snapshot: &mut ClientSnapshot, create: impl FnOnce(u64) -> SemanticDiagnostic) {
    let diagnostic_sequence = snapshot
        .diagnostics
        .last()
        .map_or(1, |diagnostic| diagnostic.sequence().saturating_add(1));
    snapshot.diagnostics.push(create(diagnostic_sequence));
    if snapshot.diagnostics.len() > 8 {
        snapshot.diagnostics.remove(0);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::mpsc::{self, TrySendError},
        time::Duration,
    };

    use crate::{
        ClientEventKind, ClientPhase, ControlCommand, EVENT_CAPACITY, FailureCategory,
        MovementIntent, Recovery, RecoveryAction, RemoteAvatarChange, RemoteAvatarFaultCategory,
        RemoteAvatarId, RemoteAvatarRemovalSource, SanitizedIdentity,
    };
    use client_protocol::{AcoreMovementInfo, RemotePlayerRecord, RemotePlayerUnusableCategory};

    use super::{BoundaryError, CONTROL_CAPACITY, new_boundary};

    #[test]
    fn lossless_control_fifo_rejects_the_seventeenth_queued_command() {
        let (sender, _receiver) = mpsc::sync_channel(CONTROL_CAPACITY);
        for _ in 0..CONTROL_CAPACITY {
            sender.try_send(ControlCommand::StartEntry).unwrap();
        }
        assert!(matches!(
            sender.try_send(ControlCommand::StartEntry),
            Err(TrySendError::Full(ControlCommand::StartEntry))
        ));
    }

    #[test]
    fn lossless_event_fifo_rejects_the_sixty_fifth_event_slot() {
        let (sender, _receiver) = mpsc::sync_channel::<u8>(EVENT_CAPACITY);
        for value in 0..EVENT_CAPACITY {
            sender.try_send(u8::try_from(value).unwrap()).unwrap();
        }
        assert!(matches!(sender.try_send(255), Err(TrySendError::Full(255))));
    }

    #[test]
    fn latest_movement_mailbox_replaces_steady_intent() {
        let (client, _worker) = new_boundary(identity()).unwrap();
        client
            .publish_movement_intent(MovementIntent::planar(1.0, 0.0).unwrap())
            .unwrap();
        client
            .publish_movement_intent(MovementIntent::planar(0.0, -1.0).unwrap())
            .unwrap();
        assert_eq!(client.snapshot().queue_counters.movement_revision, 2);
    }

    #[test]
    fn scripted_correction_never_relabels_realm_observed_pose() {
        let (client, mut worker) = new_boundary(identity()).unwrap();
        let entry = crate::WorldPose {
            map_id: 0,
            east: 1.0,
            north: 2.0,
            elevation: 3.0,
            orientation: 0.0,
        };
        let target = crate::WorldPose { east: 4.0, ..entry };
        assert!(worker.observe_entry_anchor(entry));
        assert!(worker.scripted_correction_target(crate::CorrectionTarget::scripted(target)));

        let snapshot = client.snapshot();
        assert_eq!(snapshot.realm_observed_pose, Some(entry));
        assert_eq!(snapshot.correction_target.unwrap().pose(), target);
        assert!(
            client
                .drain_events()
                .iter()
                .any(|event| matches!(event.kind, ClientEventKind::ScriptedCorrection { .. }))
        );
    }

    #[test]
    fn remote_avatar_lifecycle_is_lossless_and_foreign_guid_cannot_mutate_it() {
        let (client, mut worker) = new_boundary(identity()).unwrap();
        let _ = client.drain_events();
        let first = 0x0100_0000_0000_0001;
        let foreign = 0x0100_0000_0000_0002;
        let created = AcoreMovementInfo::ground(1, [4.0, 5.0, 6.0], 0.1, false);
        let updated = AcoreMovementInfo::ground(2, [7.0, 8.0, 9.0], 0.2, true);
        worker
            .apply_remote_player_records(
                vec![RemotePlayerRecord::PlayerCreate {
                    guid: first,
                    movement: created,
                }],
                Some(530),
            )
            .unwrap();
        worker
            .apply_remote_player_records(
                vec![RemotePlayerRecord::PlayerMovement {
                    guid: foreign,
                    movement: updated,
                    opcode: 0x00ee,
                }],
                Some(530),
            )
            .unwrap();
        worker
            .apply_remote_player_records(
                vec![RemotePlayerRecord::PlayerMovement {
                    guid: first,
                    movement: updated,
                    opcode: 0x00ee,
                }],
                Some(530),
            )
            .unwrap();
        worker
            .apply_remote_player_records(
                vec![RemotePlayerRecord::Destroy { guid: first }],
                Some(530),
            )
            .unwrap();
        worker
            .apply_remote_player_records(
                vec![RemotePlayerRecord::PlayerCreate {
                    guid: first,
                    movement: created,
                }],
                Some(530),
            )
            .unwrap();
        worker
            .apply_remote_player_records(
                vec![RemotePlayerRecord::OutOfRange { guid: first }],
                Some(530),
            )
            .unwrap();

        assert!(client.snapshot().remote_avatar.is_none());
        let events: Vec<_> = client
            .drain_events()
            .into_iter()
            .filter(|event| matches!(event.kind, ClientEventKind::RemoteAvatar { .. }))
            .collect();
        assert_eq!(events.len(), 5);
        assert_eq!(events[0].sequence + 1, events[1].sequence);
        assert_eq!(events[1].sequence + 1, events[2].sequence);
        assert!(matches!(
            events[4].kind,
            ClientEventKind::RemoteAvatar {
                change: RemoteAvatarChange::Removed {
                    source: RemoteAvatarRemovalSource::OutOfRange,
                    ..
                }
            }
        ));
        assert!(matches!(
            events[2].kind,
            ClientEventKind::RemoteAvatar {
                change: RemoteAvatarChange::Removed {
                    source: RemoteAvatarRemovalSource::DestroyObject,
                    ..
                }
            }
        ));
    }

    #[test]
    fn remote_avatar_fault_and_backpressure_clear_the_snapshot_with_a_fence() {
        let (client, mut worker) = new_boundary(identity()).unwrap();
        let _ = client.drain_events();
        let guid = 0x0100_0000_0000_0001;
        let movement = AcoreMovementInfo::ground(1, [4.0, 5.0, 6.0], 0.1, false);
        worker
            .apply_remote_player_records(
                vec![RemotePlayerRecord::PlayerCreate { guid, movement }],
                Some(0),
            )
            .unwrap();
        worker
            .apply_remote_player_records(
                vec![RemotePlayerRecord::UnusableMovement {
                    guid,
                    category: RemotePlayerUnusableCategory::InvalidPose,
                }],
                Some(0),
            )
            .unwrap();
        assert!(client.snapshot().remote_avatar.is_none());
        assert!(client.drain_events().iter().any(|event| matches!(
            event.kind,
            ClientEventKind::RemoteAvatar {
                change: RemoteAvatarChange::Faulted {
                    category: RemoteAvatarFaultCategory::InvalidPose,
                    ..
                }
            }
        )));

        worker
            .apply_remote_player_records(
                vec![RemotePlayerRecord::PlayerCreate { guid, movement }],
                Some(0),
            )
            .unwrap();
        for _ in 1..EVENT_CAPACITY {
            super::emit_event(
                &worker.events,
                &worker.counters,
                crate::ClientEvent {
                    sequence: 999,
                    kind: ClientEventKind::PhaseChanged {
                        phase: ClientPhase::MovementReady,
                    },
                },
            )
            .unwrap();
        }
        assert!(
            worker
                .apply_remote_player_records(
                    vec![RemotePlayerRecord::PlayerMovement {
                        guid,
                        movement,
                        opcode: 0x00ee
                    }],
                    Some(0),
                )
                .is_err()
        );
        let snapshot = client.snapshot();
        assert!(snapshot.remote_avatar.is_none());
        assert!(snapshot.remote_avatar_invalidated_through > 0);
        assert!(matches!(snapshot.phase, ClientPhase::Failed(_)));
        assert_eq!(RemoteAvatarId::from_realm_guid(0), None);
        assert_eq!(
            RemoteAvatarId::from_realm_guid(guid)
                .unwrap()
                .display_shorthand(),
            "0100000000000001"
        );
    }

    #[test]
    fn remote_avatar_requires_the_authenticated_entry_map_before_publication() {
        let (client, mut worker) = new_boundary(identity()).unwrap();
        let movement = AcoreMovementInfo::ground(1, [1.0, 2.0, 3.0], 0.0, false);
        let result = worker.apply_remote_player_records(
            vec![RemotePlayerRecord::PlayerCreate { guid: 1, movement }],
            None,
        );
        assert_eq!(result, Err(BoundaryError::WorkerStopped));
        assert!(client.snapshot().remote_avatar.is_none());
        assert!(matches!(
            client.snapshot().phase,
            ClientPhase::Failed(crate::Recovery {
                category: FailureCategory::ProtocolIncompatibility,
                ..
            })
        ));

        let (client, mut worker) = new_boundary(identity()).unwrap();
        worker
            .apply_remote_player_records(
                vec![RemotePlayerRecord::PlayerCreate { guid: 1, movement }],
                Some(0),
            )
            .unwrap();
        assert!(client.snapshot().remote_avatar.is_some());
        assert!(
            worker
                .apply_remote_player_records(
                    vec![RemotePlayerRecord::PlayerMovement {
                        guid: 1,
                        movement,
                        opcode: 0x00ee,
                    }],
                    None,
                )
                .is_err()
        );
        assert!(client.snapshot().remote_avatar.is_none());
    }

    #[test]
    fn clean_retry_clears_remote_state_without_advancing_the_invalidation_fence() {
        let (client, mut worker) = new_boundary(identity()).unwrap();
        worker.reset_for_retry();
        assert_eq!(client.snapshot().remote_avatar_invalidated_through, 0);
    }

    #[test]
    fn non_remote_backpressure_does_not_fabricate_a_remote_invalidation_fence() {
        let (client, mut worker) = new_boundary(identity()).unwrap();
        let _ = client.drain_events();
        let movement = AcoreMovementInfo::ground(1, [1.0, 2.0, 3.0], 0.0, false);
        worker
            .apply_remote_player_records(
                vec![RemotePlayerRecord::PlayerCreate { guid: 1, movement }],
                Some(0),
            )
            .unwrap();
        let _ = client.drain_events();
        for _ in 0..EVENT_CAPACITY {
            super::emit_event(
                &worker.events,
                &worker.counters,
                crate::ClientEvent {
                    sequence: 999,
                    kind: ClientEventKind::PhaseChanged {
                        phase: ClientPhase::MovementReady,
                    },
                },
            )
            .unwrap();
        }
        assert!(!worker.publish(ClientEventKind::PhaseChanged {
            phase: ClientPhase::Offline,
        }));
        assert!(client.snapshot().remote_avatar.is_none());
        assert_eq!(client.snapshot().remote_avatar_invalidated_through, 0);
    }

    #[test]
    fn persisted_movement_proof_freezes_the_submitted_stop_and_accepts_only_fresh_reconnect_pose() {
        let (client, mut worker) = new_boundary(identity()).unwrap();
        let anchor = crate::WorldPose {
            map_id: 0,
            east: 10.0,
            north: -4.0,
            elevation: 83.5,
            orientation: 0.0,
        };
        let submitted = crate::WorldPose {
            east: 12.25,
            ..anchor
        };
        assert!(worker.observe_entry_anchor(anchor));
        assert!(worker.movement_submitted_state(submitted, true));
        let oracle = worker
            .begin_movement_proof()
            .expect("two metre stopped move is eligible");
        assert_eq!(oracle, submitted);
        assert!(matches!(
            client.snapshot().phase,
            ClientPhase::ProvingMovement(crate::ProofStage::SavingLogout)
        ));
        assert!(worker.observe_reconnect_pose(crate::WorldPose {
            east: 12.4,
            ..anchor
        }));
        assert!(worker.complete_movement_proof(oracle));
        let proof = client.snapshot().movement_proof.unwrap();
        assert_eq!(proof.expected, submitted);
        assert!(proof.passed());

        assert!(worker.observe_reconnect_pose(crate::WorldPose {
            map_id: 1,
            ..submitted
        }));
        assert!(!worker.complete_movement_proof(oracle));
    }

    #[test]
    fn persisted_movement_proof_rejects_short_or_still_moving_submission() {
        let (client, mut worker) = new_boundary(identity()).unwrap();
        let anchor = crate::WorldPose::origin(0);
        assert!(worker.observe_entry_anchor(anchor));
        assert!(worker.movement_submitted_state(
            crate::WorldPose {
                east: 1.99,
                ..anchor
            },
            true
        ));
        assert!(worker.begin_movement_proof().is_none());
        assert!(client.snapshot().movement_proof.is_none());
    }

    #[test]
    fn persisted_movement_proof_gates_new_input_at_the_session_boundary() {
        let (client, mut worker) = new_boundary(identity()).unwrap();
        let anchor = crate::WorldPose::origin(0);
        assert!(worker.observe_entry_anchor(anchor));
        assert!(worker.movement_submitted_state(
            crate::WorldPose {
                east: 2.0,
                ..anchor
            },
            true
        ));
        assert!(worker.begin_movement_proof().is_some());
        let before = client.snapshot().queue_counters.movement_revision;

        assert_eq!(
            client.publish_movement_intent(MovementIntent::planar(1.0, 0.0).unwrap()),
            Err(BoundaryError::InputGated)
        );
        assert_eq!(client.snapshot().queue_counters.movement_revision, before);
        assert!(!worker.latest_movement_intent().engaged());
    }

    #[test]
    fn persisted_movement_proof_keeps_input_gated_through_reconnect_ready_and_comparison() {
        let (client, mut worker) = new_boundary(identity()).unwrap();
        let anchor = crate::WorldPose::origin(0);
        assert!(worker.observe_entry_anchor(anchor));
        assert!(worker.movement_submitted_state(
            crate::WorldPose {
                east: 2.0,
                ..anchor
            },
            true
        ));
        assert!(worker.begin_movement_proof().is_some());
        assert!(worker.movement_ready(7.0));
        assert!(worker.proof_stage(crate::ProofStage::Comparing));

        assert_eq!(
            client.publish_movement_intent(MovementIntent::planar(1.0, 0.0).unwrap()),
            Err(BoundaryError::InputGated)
        );
        assert!(!worker.latest_movement_intent().engaged());
    }

    #[test]
    fn persisted_movement_proof_discards_stale_transition_before_reconnect() {
        let (client, mut worker) = new_boundary(identity()).unwrap();
        let anchor = crate::WorldPose::origin(0);
        assert!(worker.observe_entry_anchor(anchor));
        assert!(worker.movement_submitted_state(
            crate::WorldPose {
                east: 2.0,
                ..anchor
            },
            true
        ));
        // This models an input edge that was queued after the proof button but
        // before the worker acquired the control FIFO again.
        client
            .send_control(ControlCommand::MovementTransition { engaged: true })
            .unwrap();

        assert!(worker.begin_movement_proof().is_some());
        assert_eq!(
            worker.receive_control(Duration::ZERO),
            Err(mpsc::RecvTimeoutError::Timeout)
        );
        assert_eq!(client.snapshot().queue_counters.control_queued, 0);
        assert!(!worker.latest_movement_intent().engaged());
    }

    #[test]
    fn persisted_movement_proof_preserves_a_queued_disconnect_as_shutdown() {
        let (client, mut worker) = new_boundary(identity()).unwrap();
        let anchor = crate::WorldPose::origin(0);
        assert!(worker.observe_entry_anchor(anchor));
        assert!(worker.movement_submitted_state(
            crate::WorldPose {
                east: 2.0,
                ..anchor
            },
            true
        ));
        client.send_control(ControlCommand::Disconnect).unwrap();

        assert!(worker.begin_movement_proof().is_some());
        assert!(worker.is_shutdown());
        assert_eq!(
            worker.receive_control(Duration::ZERO),
            Err(mpsc::RecvTimeoutError::Timeout)
        );
        assert_eq!(client.snapshot().queue_counters.control_queued, 0);
    }

    #[test]
    fn control_backpressure_is_visible_and_fail_closed_in_the_snapshot() {
        let (client, _worker) = new_boundary(identity()).unwrap();
        for _ in 0..CONTROL_CAPACITY {
            client.send_control(ControlCommand::StartEntry).unwrap();
        }
        assert_eq!(
            client.send_control(ControlCommand::StartEntry),
            Err(BoundaryError::ControlBackpressure)
        );
        assert_eq!(
            client.snapshot().phase,
            ClientPhase::Failed(Recovery {
                category: FailureCategory::InternalBackpressure,
                action: RecoveryAction::RestartClient,
            })
        );
    }

    #[test]
    fn event_backpressure_stops_the_worker_and_retains_snapshot_evidence() {
        let (client, mut worker) = new_boundary(identity()).unwrap();
        for _ in 2..EVENT_CAPACITY {
            assert!(worker.publish(ClientEventKind::PhaseChanged {
                phase: ClientPhase::Offline,
            }));
        }
        assert!(!worker.publish(ClientEventKind::Disconnected));
        let current = client.snapshot();
        assert_eq!(
            current.phase,
            ClientPhase::Failed(Recovery {
                category: FailureCategory::InternalBackpressure,
                action: RecoveryAction::RestartClient,
            })
        );
        assert_eq!(
            current.latest_failure.as_ref().unwrap().category(),
            FailureCategory::InternalBackpressure
        );
        assert_eq!(
            current.diagnostics.last().unwrap().message(),
            "event FIFO reached capacity"
        );
    }

    fn identity() -> SanitizedIdentity {
        SanitizedIdentity::new(1, "Miazcore Reference Realm", "Miaztest", 12_340).unwrap()
    }
}
