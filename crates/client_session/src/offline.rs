use std::{
    sync::mpsc,
    thread::{self, JoinHandle},
    time::Duration,
};

use crate::{
    ClientEvent, ClientFailure, ClientSnapshot, ControlCommand, FailureCategory,
    LoadedClientConfig, MovementIntent, RecoveryAction,
    boundary::{BoundaryError, SessionClient, WorkerBoundary, new_boundary},
};

/// Disposable offline source that exercises the final application/session boundary.
pub struct OfflineSession {
    client: SessionClient,
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
        let (client, mut boundary) = new_boundary(identity)?;
        let worker = thread::Builder::new()
            .name("miazcore-offline-session".to_owned())
            .spawn(move || {
                let (_config, _credentials) = loaded.into_parts();
                run_offline_worker(&mut boundary);
                boundary.mark_stopped();
            })
            .map_err(|_| BoundaryError::WorkerStopped)?;

        Ok(Self {
            client,
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

    /// # Errors
    ///
    /// Returns an error when the semantic boundary cannot retain an intent edge.
    pub fn publish_movement_intent(&self, intent: MovementIntent) -> Result<(), BoundaryError> {
        self.client.publish_movement_intent(intent)
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
        self.client.request_shutdown();
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

fn run_offline_worker(boundary: &mut WorkerBoundary) {
    while !boundary.is_shutdown() {
        match boundary.receive_control(Duration::from_millis(20)) {
            Ok(command) => {
                boundary.control_consumed();
                if command == ControlCommand::Disconnect {
                    boundary.disconnect();
                    break;
                }
                let failure = ClientFailure::new(
                    FailureCategory::Configuration,
                    "offline",
                    "network capability is deferred in this slice",
                    RecoveryAction::RestartClient,
                );
                if !boundary.reject(command.kind(), failure) {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}
