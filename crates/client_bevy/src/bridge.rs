use std::collections::VecDeque;

use bevy::prelude::*;
use client_session::{
    BoundaryError, ClientEvent, ClientEventKind, ClientSnapshot, ControlCommand,
    LiveDiagnosticSession, MovementIntent, OfflineSession,
};

use crate::ClientScheduleSet;

/// The narrowly projected session surface the Bevy layer is allowed to observe.
///
/// It deliberately exposes complete semantic commands only; protocol stages,
/// credentials, packet bodies, and movement publication stay beneath the
/// engine-independent session boundary.
pub trait DiagnosticSession: Send + Sync + 'static {
    fn snapshot(&self) -> ClientSnapshot;
    fn drain_events(&self) -> Vec<ClientEvent>;

    /// Send a bounded semantic control command.
    ///
    /// # Errors
    ///
    /// Returns an error when the session cannot accept the command.
    fn send_control(&self, command: ControlCommand) -> Result<(), BoundaryError>;

    /// Publish a replaceable movement intent.
    ///
    /// # Errors
    ///
    /// Returns an error when the boundary cannot retain an intent edge.
    fn publish_movement_intent(&self, intent: MovementIntent) -> Result<(), BoundaryError>;
    fn diagnostic_mode(&self) -> DiagnosticMode;
}

/// The deliberately small capability profile exposed to the Diagnostic World.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticMode {
    Offline,
    LiveEntry,
}

impl DiagnosticMode {
    #[must_use]
    pub const fn is_live_entry(self) -> bool {
        matches!(self, Self::LiveEntry)
    }
}

impl DiagnosticSession for OfflineSession {
    fn snapshot(&self) -> ClientSnapshot {
        self.snapshot()
    }

    fn drain_events(&self) -> Vec<ClientEvent> {
        self.drain_events()
    }

    fn send_control(&self, command: ControlCommand) -> Result<(), BoundaryError> {
        self.send_control(command)
    }

    fn publish_movement_intent(&self, intent: MovementIntent) -> Result<(), BoundaryError> {
        OfflineSession::publish_movement_intent(self, intent)
    }

    fn diagnostic_mode(&self) -> DiagnosticMode {
        DiagnosticMode::Offline
    }
}

impl DiagnosticSession for LiveDiagnosticSession {
    fn snapshot(&self) -> ClientSnapshot {
        self.snapshot()
    }

    fn drain_events(&self) -> Vec<ClientEvent> {
        self.drain_events()
    }

    fn send_control(&self, command: ControlCommand) -> Result<(), BoundaryError> {
        self.send_control(command)
    }

    fn publish_movement_intent(&self, intent: MovementIntent) -> Result<(), BoundaryError> {
        LiveDiagnosticSession::publish_movement_intent(self, intent)
    }

    fn diagnostic_mode(&self) -> DiagnosticMode {
        DiagnosticMode::LiveEntry
    }
}

#[derive(Resource)]
pub struct SessionBridge {
    session: Box<dyn DiagnosticSession>,
    mode: DiagnosticMode,
}

impl SessionBridge {
    #[must_use]
    pub fn new(session: impl DiagnosticSession) -> Self {
        let mode = session.diagnostic_mode();
        Self {
            session: Box::new(session),
            mode,
        }
    }

    #[must_use]
    pub const fn is_live_entry(&self) -> bool {
        self.mode.is_live_entry()
    }

    /// Begin the one complete configured world-entry operation.
    ///
    /// # Errors
    ///
    /// Returns an error when the session worker is no longer able to accept a
    /// bounded semantic command.
    pub fn start_entry(&self) -> Result<(), BoundaryError> {
        self.session.send_control(ControlCommand::StartEntry)
    }

    /// Retry one previously failed configured world-entry operation.
    ///
    /// # Errors
    ///
    /// Returns an error when the session worker is no longer able to accept a
    /// bounded semantic command.
    pub fn retry_entry(&self) -> Result<(), BoundaryError> {
        self.session.send_control(ControlCommand::RetryEntry)
    }

    /// Freeze bounded movement and begin the one accepted saving-reconnect
    /// persistence proof.  Eligibility remains session-owned.
    ///
    /// # Errors
    ///
    /// Returns an error when the session worker can no longer accept a
    /// semantic control command.
    pub fn verify_persisted_movement(&self) -> Result<(), BoundaryError> {
        self.session
            .send_control(ControlCommand::BeginMovementProof)
    }

    /// Start the repository-owned deterministic movement segment used only by
    /// the persisted-movement proof.
    ///
    /// # Errors
    ///
    /// Returns an error when the worker cannot accept the semantic command.
    pub fn start_scripted_persisted_movement(&self) -> Result<(), BoundaryError> {
        self.session
            .send_control(ControlCommand::ScriptedMovementProofStart)
    }

    /// Publish replaceable camera-relative movement intent through the session
    /// boundary.  Protocol serialization remains worker-owned.
    ///
    /// # Errors
    ///
    /// Returns an error when the session worker cannot accept an intent edge.
    pub fn publish_movement_intent(&self, intent: MovementIntent) -> Result<(), BoundaryError> {
        self.session.publish_movement_intent(intent)
    }
}

#[derive(Clone, Debug, Resource)]
pub struct DiagnosticView {
    pub(crate) snapshot: ClientSnapshot,
    pub(crate) recent_events: VecDeque<ClientEvent>,
    pub(crate) mode: DiagnosticMode,
}

impl DiagnosticView {
    #[must_use]
    pub const fn snapshot(&self) -> &ClientSnapshot {
        &self.snapshot
    }

    #[must_use]
    pub fn recent_events(&self) -> impl DoubleEndedIterator<Item = &ClientEvent> {
        self.recent_events.iter()
    }

    #[must_use]
    pub const fn is_live_entry(&self) -> bool {
        self.mode.is_live_entry()
    }
}

impl FromWorld for DiagnosticView {
    fn from_world(world: &mut World) -> Self {
        let session = world.resource::<SessionBridge>();
        Self {
            snapshot: session.session.snapshot(),
            // Event consumption is owned by the Ingress schedule. Draining
            // here would drop startup Remote Avatar lifecycle events before
            // they reach the one-frame lossless projection batch.
            recent_events: VecDeque::new(),
            mode: session.mode,
        }
    }
}

/// One-frame, lossless Remote Avatar event ingress. The diagnostic event tail
/// is intentionally shorter and must never be the projection source.
#[derive(Debug, Default, Resource)]
pub(crate) struct RemoteAvatarIngress(pub(crate) Vec<ClientEvent>);

pub(crate) struct SessionBridgePlugin;

impl Plugin for SessionBridgePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DiagnosticView>()
            .init_resource::<RemoteAvatarIngress>()
            .add_systems(
                Update,
                project_session_boundary.in_set(ClientScheduleSet::Ingress),
            );
    }
}

fn project_session_boundary(
    session: Res<SessionBridge>,
    mut view: ResMut<DiagnosticView>,
    mut remote_ingress: ResMut<RemoteAvatarIngress>,
) {
    view.snapshot = session.session.snapshot();
    let events = session.session.drain_events();
    remote_ingress.0 = events
        .iter()
        .filter(|event| matches!(event.kind, ClientEventKind::RemoteAvatar { .. }))
        .cloned()
        .collect();
    view.recent_events.extend(events);
    while view.recent_events.len() > 8 {
        view.recent_events.pop_front();
    }
}
