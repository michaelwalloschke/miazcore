use std::thread::JoinHandle;

use crate::{
    ClientEvent, ClientSnapshot, ControlCommand, LoadedClientConfig,
    boundary::{BoundaryError, SessionClient, new_boundary},
    runtime,
};

/// Redacted evidence returned after the headless character-selection worker has stopped.
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterSelectionEvidence {
    snapshot: ClientSnapshot,
    events: Vec<ClientEvent>,
}

impl CharacterSelectionEvidence {
    #[must_use]
    pub const fn snapshot(&self) -> &ClientSnapshot {
        &self.snapshot
    }

    #[must_use]
    pub fn events(&self) -> &[ClientEvent] {
        &self.events
    }
}

/// Engine-independent, one-attempt authenticated character-selection session.
///
/// This session intentionally disconnects before player login. The production Bevy
/// application continues to use `OfflineSession` until the live-entry slice.
pub struct CharacterSelectionSession {
    client: SessionClient,
    worker: Option<JoinHandle<()>>,
}

impl CharacterSelectionSession {
    /// Start the dedicated blocking-I/O worker behind the semantic boundary.
    ///
    /// # Errors
    ///
    /// Returns an error if initial boundary publication or worker creation fails.
    pub fn start(loaded: LoadedClientConfig) -> Result<Self, BoundaryError> {
        let identity = loaded.config().identity().clone();
        let (client, boundary) = new_boundary(identity)?;
        let worker = runtime::spawn_production_worker(
            loaded,
            boundary,
            runtime::WorkerTarget::CharacterSelection,
        )?;
        Ok(Self {
            client,
            worker: Some(worker),
        })
    }

    /// Send a lossless semantic command to the worker.
    ///
    /// # Errors
    ///
    /// Returns an error when the command queue is full or the worker has stopped.
    pub fn send_control(&self, command: ControlCommand) -> Result<(), BoundaryError> {
        self.client.send_control(command)
    }

    #[must_use]
    pub fn snapshot(&self) -> ClientSnapshot {
        self.client.snapshot()
    }

    #[must_use]
    pub fn drain_events(&self) -> Vec<ClientEvent> {
        self.client.drain_events()
    }

    /// Join the one-attempt worker and return redacted final-boundary evidence.
    ///
    /// The caller must first send `ControlCommand::StartEntry` or `Disconnect`.
    ///
    /// # Errors
    ///
    /// Returns an error if the worker panicked.
    pub fn wait(mut self) -> Result<CharacterSelectionEvidence, BoundaryError> {
        self.join_worker()?;
        Ok(CharacterSelectionEvidence {
            snapshot: self.client.snapshot(),
            events: self.client.drain_events(),
        })
    }

    /// Stop a worker that has not yet completed and join it.
    ///
    /// # Errors
    ///
    /// Returns an error if the worker panicked.
    pub fn shutdown(mut self) -> Result<(), BoundaryError> {
        self.stop_worker()
    }

    fn join_worker(&mut self) -> Result<(), BoundaryError> {
        if let Some(worker) = self.worker.take() {
            worker.join().map_err(|_| BoundaryError::WorkerPanicked)?;
        }
        Ok(())
    }

    fn stop_worker(&mut self) -> Result<(), BoundaryError> {
        self.client.request_shutdown();
        self.join_worker()
    }
}

impl Drop for CharacterSelectionSession {
    fn drop(&mut self) {
        let _ = self.stop_worker();
    }
}
