use std::{
    error::Error,
    fmt,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    time::Duration,
};

use crate::{
    CONTROL_CAPACITY, ClientEvent, ClientEventKind, ClientFailure, ClientPhase, ClientSnapshot,
    CommandKind, ControlCommand, CorrectionTarget, DiscoveredRealm, EVENT_CAPACITY,
    FailureCategory, MovementIntent, MovementProofEvidence, PoseSource, ProofStage, QueueCounters,
    Recovery, RecoveryAction, SanitizedIdentity, SelectedCharacter, SemanticDiagnostic, WorldPose,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoundaryError {
    ControlBackpressure,
    EventBackpressure,
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
        let was_engaged = {
            let mut movement = self.movement.lock().expect("movement mailbox poisoned");
            let was_engaged = movement.engaged();
            *movement = intent;
            was_engaged
        };
        self.counters
            .movement_revision
            .fetch_add(1, Ordering::AcqRel);
        if was_engaged != intent.engaged() {
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
    last_submitted_was_stop: bool,
    event_sequence: u64,
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

    pub(crate) fn discard_pending_controls(&self) {
        while self.control.try_recv().is_ok() {
            self.counters.control_queued.fetch_sub(1, Ordering::AcqRel);
        }
    }

    pub(crate) fn latest_movement_intent(&self) -> MovementIntent {
        *self.movement.lock().expect("movement mailbox poisoned")
    }

    pub(crate) fn begin_movement_proof(&mut self) -> Option<WorldPose> {
        let mut current = self.snapshot.write().expect("client snapshot poisoned");
        let anchor = current.entry_anchor?;
        let expected = current.submitted_pose?;
        let displacement = (expected.east - anchor.east).hypot(expected.north - anchor.north);
        if displacement < 2.0
            || !self.last_submitted_was_stop
            || self.latest_movement_intent().engaged()
        {
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
        self.publish(ClientEventKind::PhaseChanged {
            phase: ClientPhase::ProvingMovement(ProofStage::SavingLogout),
        })
        .then_some(expected)
    }

    pub(crate) fn proof_stage(&mut self, stage: ProofStage) -> bool {
        self.transition(ClientPhase::ProvingMovement(stage))
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
            self.counters
                .snapshot_revision
                .fetch_add(1, Ordering::AcqRel);
        }
        self.publish(ClientEventKind::MovementSubmitted { pose })
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
        let mut current = self.snapshot.write().expect("client snapshot poisoned");
        current.phase = ClientPhase::Offline;
        current.discovered_realm = None;
        current.selected_character = None;
        current.entry_anchor = None;
        current.predicted_pose = None;
        current.submitted_pose = None;
        current.realm_observed_pose = None;
        current.correction_target = None;
        current.movement_proof = None;
        current.run_speed = None;
        current.latest_failure = None;
        self.counters
            .snapshot_revision
            .fetch_add(1, Ordering::AcqRel);
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
            current.latest_failure = Some(failure.clone());
            push_diagnostic(&mut current, |sequence| {
                SemanticDiagnostic::from_failure(sequence, &failure)
            });
            self.counters
                .snapshot_revision
                .fetch_add(1, Ordering::AcqRel);
        }
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

    fn publish(&mut self, kind: ClientEventKind) -> bool {
        self.event_sequence = self.event_sequence.saturating_add(1);
        let event = ClientEvent {
            sequence: self.event_sequence,
            kind,
        };
        match emit_event(&self.events, &self.counters, event) {
            Ok(()) => true,
            Err(BoundaryError::EventBackpressure) => {
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
                | BoundaryError::ControlBackpressure,
            ) => false,
        }
    }
}

pub(crate) fn new_boundary(
    identity: SanitizedIdentity,
) -> Result<(SessionClient, WorkerBoundary), BoundaryError> {
    let (control_sender, control_receiver) = mpsc::sync_channel(CONTROL_CAPACITY);
    let (event_sender, event_receiver) = mpsc::sync_channel(EVENT_CAPACITY);
    let counters = Arc::new(BoundaryCounters::default());
    let movement = Arc::new(Mutex::new(MovementIntent::idle()));
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
            last_submitted_was_stop: false,
            event_sequence: 2,
        },
    ))
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
    use std::sync::mpsc::{self, TrySendError};

    use crate::{
        ClientEventKind, ClientPhase, ControlCommand, EVENT_CAPACITY, FailureCategory,
        MovementIntent, Recovery, RecoveryAction, SanitizedIdentity,
    };

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
