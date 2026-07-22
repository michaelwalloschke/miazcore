use std::collections::VecDeque;

use bevy::prelude::*;
use client_session::{ClientEvent, ClientSnapshot, OfflineSession};

use crate::ClientScheduleSet;

#[derive(Resource)]
pub struct SessionBridge(OfflineSession);

impl SessionBridge {
    #[must_use]
    pub const fn new(session: OfflineSession) -> Self {
        Self(session)
    }
}

#[derive(Clone, Debug, Resource)]
pub struct DiagnosticView {
    pub(crate) snapshot: ClientSnapshot,
    pub(crate) recent_events: VecDeque<ClientEvent>,
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
}

impl FromWorld for DiagnosticView {
    fn from_world(world: &mut World) -> Self {
        let session = world.resource::<SessionBridge>();
        Self {
            snapshot: session.0.snapshot(),
            recent_events: session.0.drain_events().into(),
        }
    }
}

pub(crate) struct SessionBridgePlugin;

impl Plugin for SessionBridgePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DiagnosticView>().add_systems(
            Update,
            project_session_boundary.in_set(ClientScheduleSet::Ingress),
        );
    }
}

fn project_session_boundary(session: Res<SessionBridge>, mut view: ResMut<DiagnosticView>) {
    view.snapshot = session.0.snapshot();
    view.recent_events.extend(session.0.drain_events());
    while view.recent_events.len() > 8 {
        view.recent_events.pop_front();
    }
}
