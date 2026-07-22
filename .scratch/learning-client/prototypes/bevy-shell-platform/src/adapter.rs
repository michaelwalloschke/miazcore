//! Thin Bevy adapter around the engine-free model.

use std::collections::VecDeque;

use bevy::prelude::*;

use crate::model::{ClientEvent, ClientState};

#[derive(Debug, Resource, Default)]
pub struct ClientModel(ClientState);

impl ClientModel {
    #[must_use]
    pub const fn state(&self) -> &ClientState {
        &self.0
    }
}

#[derive(Debug, Resource, Default)]
pub struct ClientEventQueue(VecDeque<ClientEvent>);

impl ClientEventQueue {
    pub fn push(&mut self, event: ClientEvent) {
        self.0.push_back(event);
    }

    pub fn extend(&mut self, events: impl IntoIterator<Item = ClientEvent>) {
        self.0.extend(events);
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub enum ClientAdapterSet {
    ApplyEvents,
}

pub fn install_client_adapter(app: &mut App) {
    app.init_resource::<ClientModel>()
        .init_resource::<ClientEventQueue>()
        .add_systems(Update, apply_events.in_set(ClientAdapterSet::ApplyEvents));
}

fn apply_events(mut model: ResMut<ClientModel>, mut events: ResMut<ClientEventQueue>) {
    while let Some(event) = events.0.pop_front() {
        model.0.apply(event);
    }
}
