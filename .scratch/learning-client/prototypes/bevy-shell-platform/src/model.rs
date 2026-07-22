//! Engine-free events and state for the disposable shell proof.

/// A position in the learning client's world coordinate vocabulary.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Position {
    pub east: f32,
    pub north: f32,
    pub elevation: f32,
}

impl Position {
    #[must_use]
    pub const fn new(east: f32, north: f32, elevation: f32) -> Self {
        Self {
            east,
            north,
            elevation,
        }
    }
}

/// Scriptable client-domain events. This type intentionally has no engine types.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClientEvent {
    EnterWorld { anchor: Position },
    PredictPlanar { east: f32, north: f32 },
    MarkSubmitted,
    ObserveRealm { position: Position },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WorldPhase {
    #[default]
    Disconnected,
    Ready,
}

/// The three pose truths selected by the movement-contract ticket.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClientState {
    pub phase: WorldPhase,
    pub rendered: Position,
    pub submitted: Position,
    pub realm_observed: Position,
}

impl ClientState {
    /// Apply one domain event. Movement before world entry is deliberately ignored.
    pub fn apply(&mut self, event: ClientEvent) {
        match event {
            ClientEvent::EnterWorld { anchor } => {
                self.phase = WorldPhase::Ready;
                self.rendered = anchor;
                self.submitted = anchor;
                self.realm_observed = anchor;
            }
            ClientEvent::PredictPlanar { east, north } if self.phase == WorldPhase::Ready => {
                self.rendered.east += east;
                self.rendered.north += north;
            }
            ClientEvent::MarkSubmitted if self.phase == WorldPhase::Ready => {
                self.submitted = self.rendered;
            }
            ClientEvent::ObserveRealm { position } if self.phase == WorldPhase::Ready => {
                self.realm_observed = position;
            }
            ClientEvent::PredictPlanar { .. }
            | ClientEvent::MarkSubmitted
            | ClientEvent::ObserveRealm { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ClientEvent, ClientState, Position, WorldPhase};

    #[test]
    fn movement_is_inert_until_world_entry() {
        let mut state = ClientState::default();

        state.apply(ClientEvent::PredictPlanar {
            east: 3.0,
            north: -2.0,
        });

        assert_eq!(state.phase, WorldPhase::Disconnected);
        assert_eq!(state.rendered, Position::default());
    }

    #[test]
    fn pose_truths_remain_separate() {
        let anchor = Position::new(1.0, 2.0, 0.25);
        let correction = Position::new(1.5, 1.75, 0.25);
        let mut state = ClientState::default();

        for event in [
            ClientEvent::EnterWorld { anchor },
            ClientEvent::PredictPlanar {
                east: 2.0,
                north: -1.0,
            },
            ClientEvent::MarkSubmitted,
            ClientEvent::ObserveRealm {
                position: correction,
            },
            ClientEvent::PredictPlanar {
                east: 0.5,
                north: 0.25,
            },
        ] {
            state.apply(event);
        }

        assert_eq!(state.rendered, Position::new(3.5, 1.25, 0.25));
        assert_eq!(state.submitted, Position::new(3.0, 1.0, 0.25));
        assert_eq!(state.realm_observed, correction);
    }
}
