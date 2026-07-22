use crate::{
    ClientEvent, ClientSnapshot, ControlCommand, LoadedClientConfig,
    boundary::BoundaryError,
    headless::{HeadlessEvidence, HeadlessSession},
    runtime::WorkerTarget,
};

/// Redacted evidence returned after the headless discovery worker has stopped.
#[derive(Clone, Debug, PartialEq)]
pub struct RealmDiscoveryEvidence(HeadlessEvidence);

impl RealmDiscoveryEvidence {
    #[must_use]
    pub const fn snapshot(&self) -> &ClientSnapshot {
        self.0.snapshot()
    }

    #[must_use]
    pub fn events(&self) -> &[ClientEvent] {
        self.0.events()
    }
}

/// Engine-independent, one-attempt authenticated realm-discovery session.
///
/// The production Bevy application deliberately continues to use `OfflineSession`.
pub struct RealmDiscoverySession(HeadlessSession);

impl RealmDiscoverySession {
    /// Start the dedicated blocking-I/O worker behind the final semantic boundary.
    ///
    /// # Errors
    ///
    /// Returns an error if initial boundary publication or worker creation fails.
    pub fn start(loaded: LoadedClientConfig) -> Result<Self, BoundaryError> {
        HeadlessSession::start(loaded, WorkerTarget::RealmDiscovery).map(Self)
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

    /// Join the one-attempt worker and return redacted final-boundary evidence.
    ///
    /// The caller must first send `ControlCommand::StartEntry` or `Disconnect`.
    ///
    /// # Errors
    ///
    /// Returns an error if the worker panicked.
    pub fn wait(self) -> Result<RealmDiscoveryEvidence, BoundaryError> {
        self.0.wait().map(RealmDiscoveryEvidence)
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
