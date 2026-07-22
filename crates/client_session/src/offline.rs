use std::{
    error::Error,
    fmt,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use crate::{
    CONTROL_CAPACITY, ClientEvent, ClientEventKind, ClientFailure, ClientPhase, ClientSnapshot,
    ControlCommand, EVENT_CAPACITY, FailureCategory, LoadedClientConfig, MovementIntent,
    QueueCounters, Recovery, RecoveryAction, SemanticDiagnostic,
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
            Self::WorkerStopped => formatter.write_str("offline session worker has stopped"),
            Self::WorkerPanicked => formatter.write_str("offline session worker panicked"),
        }
    }
}

impl Error for BoundaryError {}

#[derive(Default)]
struct BoundaryCounters {
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

struct SessionClient {
    control: SyncSender<ControlCommand>,
    events: Mutex<Receiver<ClientEvent>>,
    movement: Mutex<MovementIntent>,
    snapshot: Arc<RwLock<ClientSnapshot>>,
    counters: Arc<BoundaryCounters>,
    shutdown: Arc<AtomicBool>,
    worker_stopped: Arc<AtomicBool>,
}

impl SessionClient {
    fn send_control(&self, command: ControlCommand) -> Result<(), BoundaryError> {
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

    fn publish_movement_intent(&self, intent: MovementIntent) {
        *self.movement.lock().expect("movement mailbox poisoned") = intent;
        self.counters
            .movement_revision
            .fetch_add(1, Ordering::AcqRel);
    }

    fn drain_events(&self) -> Vec<ClientEvent> {
        let receiver = self.events.lock().expect("event receiver poisoned");
        let mut drained = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            self.counters.event_queued.fetch_sub(1, Ordering::AcqRel);
            drained.push(event);
        }
        drained
    }

    fn snapshot(&self) -> ClientSnapshot {
        let mut snapshot = self
            .snapshot
            .read()
            .expect("client snapshot poisoned")
            .clone();
        snapshot.queue_counters = self.counters.snapshot();
        snapshot
    }
}

/// Disposable offline source that exercises the final application/session boundary.
pub struct OfflineSession {
    client: SessionClient,
    shutdown: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl OfflineSession {
    /// Start a network-free worker that owns loaded credentials and exercises the final boundary.
    ///
    /// # Errors
    ///
    /// Returns an error if the worker thread or initial bounded event publication fails.
    pub fn start(loaded: LoadedClientConfig) -> Result<Self, BoundaryError> {
        let identity = loaded.config().identity().clone();
        let (control_sender, control_receiver) = mpsc::sync_channel(CONTROL_CAPACITY);
        let (event_sender, event_receiver) = mpsc::sync_channel(EVENT_CAPACITY);
        let counters = Arc::new(BoundaryCounters::default());
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

        let worker_counters = Arc::clone(&counters);
        let worker_shutdown = Arc::clone(&shutdown);
        let stopped_flag = Arc::clone(&worker_stopped);
        let worker_snapshot = Arc::clone(&snapshot);
        let worker = thread::Builder::new()
            .name("miazcore-offline-session".to_owned())
            .spawn(move || {
                let (_config, _credentials) = loaded.into_parts();
                run_offline_worker(
                    &control_receiver,
                    &event_sender,
                    &worker_snapshot,
                    &worker_counters,
                    &worker_shutdown,
                );
                stopped_flag.store(true, Ordering::Release);
            })
            .map_err(|_| BoundaryError::WorkerStopped)?;

        Ok(Self {
            client: SessionClient {
                control: control_sender,
                events: Mutex::new(event_receiver),
                movement: Mutex::new(MovementIntent::idle()),
                snapshot,
                counters,
                shutdown: Arc::clone(&shutdown),
                worker_stopped,
            },
            shutdown,
            worker: Some(worker),
        })
    }

    /// Publish a lossless semantic operation to the offline worker.
    ///
    /// # Errors
    ///
    /// Returns an error when the control FIFO is full or the worker has stopped.
    pub fn send_control(&self, command: ControlCommand) -> Result<(), BoundaryError> {
        self.client.send_control(command)
    }

    pub fn publish_movement_intent(&self, intent: MovementIntent) {
        self.client.publish_movement_intent(intent);
    }

    #[must_use]
    pub fn drain_events(&self) -> Vec<ClientEvent> {
        self.client.drain_events()
    }

    #[must_use]
    pub fn snapshot(&self) -> ClientSnapshot {
        self.client.snapshot()
    }

    /// Stop and join the offline worker.
    ///
    /// # Errors
    ///
    /// Returns an error if the worker panicked while shutting down.
    pub fn shutdown(mut self) -> Result<(), BoundaryError> {
        self.stop_worker()
    }

    fn stop_worker(&mut self) -> Result<(), BoundaryError> {
        self.shutdown.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.join().map_err(|_| BoundaryError::WorkerPanicked)?;
        }
        Ok(())
    }
}

impl Drop for OfflineSession {
    fn drop(&mut self) {
        let _ = self.stop_worker();
    }
}

fn run_offline_worker(
    control: &Receiver<ControlCommand>,
    events: &SyncSender<ClientEvent>,
    snapshot: &Arc<RwLock<ClientSnapshot>>,
    counters: &Arc<BoundaryCounters>,
    shutdown: &Arc<AtomicBool>,
) {
    let mut event_sequence = 2;
    while !shutdown.load(Ordering::Acquire) {
        match control.recv_timeout(Duration::from_millis(20)) {
            Ok(command) => {
                counters.control_queued.fetch_sub(1, Ordering::AcqRel);
                if command == ControlCommand::Disconnect {
                    event_sequence += 1;
                    if !publish_worker_event(
                        events,
                        counters,
                        snapshot,
                        shutdown,
                        ClientEvent {
                            sequence: event_sequence,
                            kind: ClientEventKind::Disconnected,
                        },
                    ) {
                        break;
                    }
                    break;
                }
                let failure = ClientFailure::new(
                    FailureCategory::Configuration,
                    "offline",
                    "network capability is deferred in this slice",
                    RecoveryAction::RestartClient,
                );
                {
                    let mut current = snapshot.write().expect("client snapshot poisoned");
                    current.latest_failure = Some(failure.clone());
                    counters.snapshot_revision.fetch_add(1, Ordering::AcqRel);
                }
                event_sequence += 1;
                if !publish_worker_event(
                    events,
                    counters,
                    snapshot,
                    shutdown,
                    ClientEvent {
                        sequence: event_sequence,
                        kind: ClientEventKind::CommandRejected {
                            command: command.kind(),
                            failure,
                        },
                    },
                ) {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
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

fn publish_worker_event(
    sender: &SyncSender<ClientEvent>,
    counters: &BoundaryCounters,
    snapshot: &Arc<RwLock<ClientSnapshot>>,
    shutdown: &AtomicBool,
    event: ClientEvent,
) -> bool {
    match emit_event(sender, counters, event) {
        Ok(()) => true,
        Err(BoundaryError::EventBackpressure) => {
            record_backpressure_failure(snapshot, counters, "event FIFO reached capacity");
            shutdown.store(true, Ordering::Release);
            false
        }
        Err(
            BoundaryError::WorkerStopped
            | BoundaryError::WorkerPanicked
            | BoundaryError::ControlBackpressure,
        ) => false,
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
    let diagnostic_sequence = current
        .diagnostics
        .last()
        .map_or(1, |diagnostic| diagnostic.sequence().saturating_add(1));
    current
        .diagnostics
        .push(SemanticDiagnostic::new(diagnostic_sequence, context));
    if current.diagnostics.len() > 8 {
        current.diagnostics.remove(0);
    }
    counters.snapshot_revision.fetch_add(1, Ordering::AcqRel);
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, Ordering},
        mpsc,
    };

    use crate::{
        ClientEvent, ClientEventKind, ClientPhase, ClientSnapshot, ControlCommand, EVENT_CAPACITY,
        FailureCategory, MovementIntent, Recovery, RecoveryAction, SanitizedIdentity,
    };

    use super::{
        BoundaryCounters, BoundaryError, CONTROL_CAPACITY, SessionClient, TrySendError,
        run_offline_worker,
    };

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
        let mailbox = std::sync::Mutex::new(MovementIntent::idle());
        *mailbox.lock().unwrap() = MovementIntent::planar(1.0, 0.0).unwrap();
        *mailbox.lock().unwrap() = MovementIntent::planar(0.0, -1.0).unwrap();
        assert_eq!(
            *mailbox.lock().unwrap(),
            MovementIntent::planar(0.0, -1.0).unwrap()
        );
    }

    #[test]
    fn control_backpressure_is_visible_and_fail_closed_in_the_snapshot() {
        let (control, _control_receiver) = mpsc::sync_channel(CONTROL_CAPACITY);
        let (_event_sender, events) = mpsc::sync_channel(EVENT_CAPACITY);
        let counters = Arc::new(BoundaryCounters::default());
        let snapshot = Arc::new(RwLock::new(offline_snapshot()));
        let shutdown = Arc::new(AtomicBool::new(false));
        let client = SessionClient {
            control,
            events: Mutex::new(events),
            movement: Mutex::new(MovementIntent::idle()),
            snapshot: Arc::clone(&snapshot),
            counters,
            shutdown: Arc::clone(&shutdown),
            worker_stopped: Arc::new(AtomicBool::new(false)),
        };

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
        assert!(shutdown.load(Ordering::Acquire));
    }

    #[test]
    fn event_backpressure_stops_the_worker_and_retains_snapshot_evidence() {
        let (control_sender, control) = mpsc::sync_channel(CONTROL_CAPACITY);
        let (events, _event_receiver) = mpsc::sync_channel(EVENT_CAPACITY);
        let counters = Arc::new(BoundaryCounters::default());
        let snapshot = Arc::new(RwLock::new(offline_snapshot()));
        let shutdown = Arc::new(AtomicBool::new(false));

        for sequence in 0..EVENT_CAPACITY {
            events
                .try_send(ClientEvent {
                    sequence: u64::try_from(sequence).unwrap(),
                    kind: ClientEventKind::PhaseChanged {
                        phase: ClientPhase::Offline,
                    },
                })
                .unwrap();
        }
        counters
            .event_queued
            .store(EVENT_CAPACITY, Ordering::Release);
        control_sender.try_send(ControlCommand::StartEntry).unwrap();
        counters.control_queued.store(1, Ordering::Release);

        run_offline_worker(&control, &events, &snapshot, &counters, &shutdown);

        let current = snapshot.read().unwrap();
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
        assert!(shutdown.load(Ordering::Acquire));
    }

    fn offline_snapshot() -> ClientSnapshot {
        ClientSnapshot::offline(
            SanitizedIdentity::new(1, "Miazcore Reference Realm", "Miaztest", 12_340).unwrap(),
        )
    }
}
