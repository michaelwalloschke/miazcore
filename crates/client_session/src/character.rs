use crate::{
    ClientEvent, ClientSnapshot, ControlCommand, LoadedClientConfig,
    boundary::BoundaryError,
    headless::{HeadlessEvidence, HeadlessSession},
    runtime::WorkerTarget,
};

/// Redacted evidence returned after the headless character-selection worker has stopped.
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterSelectionEvidence(HeadlessEvidence);

impl CharacterSelectionEvidence {
    #[must_use]
    pub const fn snapshot(&self) -> &ClientSnapshot {
        self.0.snapshot()
    }

    #[must_use]
    pub fn events(&self) -> &[ClientEvent] {
        self.0.events()
    }
}

/// Engine-independent, one-attempt authenticated character-selection session.
///
/// This session intentionally disconnects before player login. The production Bevy
/// application continues to use `OfflineSession` until the live-entry slice.
pub struct CharacterSelectionSession(HeadlessSession);

impl CharacterSelectionSession {
    /// Start the dedicated blocking-I/O worker behind the semantic boundary.
    ///
    /// # Errors
    ///
    /// Returns an error if initial boundary publication or worker creation fails.
    pub fn start(loaded: LoadedClientConfig) -> Result<Self, BoundaryError> {
        HeadlessSession::start(loaded, WorkerTarget::CharacterSelection).map(Self)
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
    pub fn wait(self) -> Result<CharacterSelectionEvidence, BoundaryError> {
        self.0.wait().map(CharacterSelectionEvidence)
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
