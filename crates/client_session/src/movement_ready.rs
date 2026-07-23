use crate::{
    ClientEvent, ClientSnapshot, ControlCommand, LoadedClientConfig, MovementIntent,
    boundary::BoundaryError,
    headless::{HeadlessEvidence, HeadlessSession},
    runtime::WorkerTarget,
};

/// Redacted evidence returned after a headless world-entry worker has stopped.
#[derive(Clone, Debug, PartialEq)]
pub struct MovementReadyEvidence(HeadlessEvidence);

impl MovementReadyEvidence {
    #[must_use]
    pub const fn snapshot(&self) -> &ClientSnapshot {
        self.0.snapshot()
    }

    #[must_use]
    pub fn events(&self) -> &[ClientEvent] {
        self.0.events()
    }
}

/// Engine-independent authenticated world-entry session.
///
/// A failed attempt keeps the worker available for exactly explicit
/// [`ControlCommand::RetryEntry`] or disconnect. Every attempt recreates both
/// sockets, stream ciphers, deadlines, and client entropy.
pub struct MovementReadySession(HeadlessSession);

impl MovementReadySession {
    /// Start the dedicated blocking-I/O worker behind the semantic boundary.
    ///
    /// # Errors
    ///
    /// Returns an error if initial boundary publication or worker creation fails.
    pub fn start(loaded: LoadedClientConfig) -> Result<Self, BoundaryError> {
        HeadlessSession::start(loaded, WorkerTarget::MovementReady).map(Self)
    }

    /// Send a lossless semantic command to the worker.
    ///
    /// # Errors
    ///
    /// Returns an error when the command queue is full or the worker has stopped.
    pub fn send_control(&self, command: ControlCommand) -> Result<(), BoundaryError> {
        self.0.send_control(command)
    }

    #[must_use]
    pub fn snapshot(&self) -> ClientSnapshot {
        self.0.snapshot()
    }

    #[must_use]
    pub fn drain_events(&self) -> Vec<ClientEvent> {
        self.0.drain_events()
    }

    /// Replace the latest movement direction and losslessly enqueue an edge
    /// transition when it changes between moving and idle.
    ///
    /// # Errors
    ///
    /// Returns an error when the worker can no longer accept a transition.
    pub fn publish_movement_intent(&self, intent: MovementIntent) -> Result<(), BoundaryError> {
        self.0.publish_movement_intent(intent)
    }

    /// Join a successful or explicitly disconnected worker and return evidence.
    ///
    /// After a failed attempt, the caller must first send `RetryEntry` or
    /// `Disconnect`; failures never trigger an automatic retry.
    ///
    /// # Errors
    ///
    /// Returns an error if the worker panicked.
    pub fn wait(self) -> Result<MovementReadyEvidence, BoundaryError> {
        self.0.wait().map(MovementReadyEvidence)
    }

    /// Stop a worker that has not yet completed and join it.
    ///
    /// # Errors
    ///
    /// Returns an error if the worker panicked.
    pub fn shutdown(self) -> Result<(), BoundaryError> {
        self.0.shutdown()
    }
}

/// Engine-independent session retained at `MovementReady` for Bevy projection.
///
/// The worker still performs exactly the same complete `StartEntry` operation as
/// [`MovementReadySession`], but it retains the authenticated World transport
/// for the bounded on-ground movement capability.
pub struct LiveDiagnosticSession(HeadlessSession);

impl LiveDiagnosticSession {
    /// Start a dedicated worker that holds the completed entry invariant.
    ///
    /// # Errors
    ///
    /// Returns an error if initial boundary publication or worker creation fails.
    pub fn start(loaded: LoadedClientConfig) -> Result<Self, BoundaryError> {
        HeadlessSession::start(loaded, WorkerTarget::LiveDiagnostic).map(Self)
    }

    /// Send a lossless semantic control command to the worker.
    ///
    /// The live Diagnostic World accepts `StartEntry`, `Disconnect`, and
    /// bounded movement intent after it reaches `MovementReady`.
    ///
    /// # Errors
    ///
    /// Returns an error when the control queue is full or the worker has stopped.
    pub fn send_control(&self, command: ControlCommand) -> Result<(), BoundaryError> {
        self.0.send_control(command)
    }

    #[must_use]
    pub fn snapshot(&self) -> ClientSnapshot {
        self.0.snapshot()
    }

    #[must_use]
    pub fn drain_events(&self) -> Vec<ClientEvent> {
        self.0.drain_events()
    }

    /// Replace the latest movement direction and losslessly enqueue an edge
    /// transition when it changes between moving and idle.
    ///
    /// # Errors
    ///
    /// Returns an error when the worker can no longer accept a transition.
    pub fn publish_movement_intent(&self, intent: MovementIntent) -> Result<(), BoundaryError> {
        self.0.publish_movement_intent(intent)
    }

    /// Request a clean disconnect and join the worker.
    ///
    /// # Errors
    ///
    /// Returns an error if the worker panicked.
    pub fn shutdown(self) -> Result<(), BoundaryError> {
        self.0.shutdown()
    }
}
