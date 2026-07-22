use std::thread::JoinHandle;

use crate::{
    ClientEvent, ClientSnapshot, ControlCommand, LoadedClientConfig,
    boundary::{BoundaryError, SessionClient, new_boundary},
    runtime::{self, WorkerTarget},
};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HeadlessEvidence {
    snapshot: ClientSnapshot,
    events: Vec<ClientEvent>,
}

impl HeadlessEvidence {
    pub(crate) const fn snapshot(&self) -> &ClientSnapshot {
        &self.snapshot
    }

    pub(crate) fn events(&self) -> &[ClientEvent] {
        &self.events
    }
}

pub(crate) struct HeadlessSession {
    client: SessionClient,
    worker: Option<JoinHandle<()>>,
}

impl HeadlessSession {
    pub(crate) fn start(
        loaded: LoadedClientConfig,
        target: WorkerTarget,
    ) -> Result<Self, BoundaryError> {
        let identity = loaded.config().identity().clone();
        let (client, boundary) = new_boundary(identity)?;
        let worker = runtime::spawn_production_worker(loaded, boundary, target)?;
        Ok(Self {
            client,
            worker: Some(worker),
        })
    }

    pub(crate) fn send_control(&self, command: ControlCommand) -> Result<(), BoundaryError> {
        self.client.send_control(command)
    }

    pub(crate) fn snapshot(&self) -> ClientSnapshot {
        self.client.snapshot()
    }

    pub(crate) fn drain_events(&self) -> Vec<ClientEvent> {
        self.client.drain_events()
    }

    pub(crate) fn wait(mut self) -> Result<HeadlessEvidence, BoundaryError> {
        self.join_worker()?;
        Ok(HeadlessEvidence {
            snapshot: self.client.snapshot(),
            events: self.client.drain_events(),
        })
    }

    pub(crate) fn shutdown(mut self) -> Result<(), BoundaryError> {
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

impl Drop for HeadlessSession {
    fn drop(&mut self) {
        let _ = self.stop_worker();
    }
}
