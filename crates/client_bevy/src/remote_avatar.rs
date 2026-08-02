use std::f32::consts::PI;

use bevy::prelude::*;
use client_session::{
    ClientEventKind, ClientPhase, RemoteAvatarChange, RemoteAvatarFaultCategory, RemoteAvatarId,
    WorldPose,
};

use crate::{ClientScheduleSet, DiagnosticView, bridge::RemoteAvatarIngress};

const REMOTE_SNAP_DISTANCE_METRES: f32 = 1.628;
const REMOTE_SMOOTH_METRES_PER_SECOND: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum RemoteAvatarPresentationState {
    Absent,
    Present {
        id: RemoteAvatarId,
        realm_observed_pose: WorldPose,
        rendered_pose: WorldPose,
        smooth: bool,
    },
    Fault {
        id: RemoteAvatarId,
        category: RemoteAvatarFaultCategory,
    },
    MapContextUnavailable {
        id: RemoteAvatarId,
    },
}

#[derive(Debug, Resource)]
pub(crate) struct RemoteAvatarPresentation {
    pub(crate) state: RemoteAvatarPresentationState,
    last_event_sequence: u64,
    invalidated_through: u64,
    pub(crate) last_ingress_diagnostic: Option<&'static str>,
}

impl Default for RemoteAvatarPresentation {
    fn default() -> Self {
        Self {
            state: RemoteAvatarPresentationState::Absent,
            last_event_sequence: 0,
            invalidated_through: 0,
            last_ingress_diagnostic: None,
        }
    }
}

impl RemoteAvatarPresentation {
    fn clear(&mut self) {
        self.state = RemoteAvatarPresentationState::Absent;
    }

    fn project(
        &mut self,
        snapshot: &client_session::ClientSnapshot,
        events: &[client_session::ClientEvent],
        delta_seconds: f32,
    ) {
        if matches!(
            snapshot.phase,
            ClientPhase::Offline | ClientPhase::Failed(_)
        ) {
            self.clear();
            return;
        }
        if snapshot.remote_avatar_invalidated_through > self.invalidated_through {
            self.invalidated_through = snapshot.remote_avatar_invalidated_through;
            self.last_event_sequence = self.last_event_sequence.max(self.invalidated_through);
            self.clear();
        }
        for event in events {
            if event.sequence <= self.invalidated_through {
                continue;
            }
            if event.sequence <= self.last_event_sequence {
                self.last_ingress_diagnostic = Some("OUT-OF-ORDER EVENT IGNORED");
                continue;
            }
            self.last_event_sequence = event.sequence;
            if let ClientEventKind::RemoteAvatar { change } = event.kind {
                self.apply_change(snapshot.entry_anchor, change);
            }
        }
        if let Some(remote) = snapshot.remote_avatar
            && (remote.source_sequence > self.last_event_sequence
                || matches!(self.state, RemoteAvatarPresentationState::MapContextUnavailable { id } if id == remote.id))
        {
            self.last_event_sequence = remote.source_sequence;
            self.hydrate(snapshot.entry_anchor, remote.id, remote.realm_observed_pose);
        }
        self.smooth(delta_seconds);
    }

    fn apply_change(&mut self, anchor: Option<WorldPose>, change: RemoteAvatarChange) {
        match change {
            RemoteAvatarChange::Created {
                id,
                realm_observed_pose,
            } => {
                self.hydrate(anchor, id, realm_observed_pose);
            }
            RemoteAvatarChange::Updated {
                id,
                realm_observed_pose,
            } => {
                let Some(anchor) = anchor else {
                    self.state = RemoteAvatarPresentationState::MapContextUnavailable { id };
                    return;
                };
                if anchor.map_id != realm_observed_pose.map_id {
                    self.state = RemoteAvatarPresentationState::MapContextUnavailable { id };
                    return;
                }
                match self.state {
                    RemoteAvatarPresentationState::Present {
                        id: known,
                        rendered_pose,
                        ..
                    } if known == id => {
                        let distance = planar_distance(rendered_pose, realm_observed_pose);
                        let smooth = distance < REMOTE_SNAP_DISTANCE_METRES;
                        self.state = RemoteAvatarPresentationState::Present {
                            id,
                            realm_observed_pose,
                            rendered_pose: if smooth {
                                rendered_pose
                            } else {
                                realm_observed_pose
                            },
                            smooth,
                        };
                    }
                    RemoteAvatarPresentationState::MapContextUnavailable { id: known }
                        if known == id =>
                    {
                        self.hydrate(Some(anchor), id, realm_observed_pose);
                    }
                    _ => {}
                }
            }
            RemoteAvatarChange::Removed { .. }
                if !matches!(self.state, RemoteAvatarPresentationState::Fault { .. }) =>
            {
                self.clear();
            }
            RemoteAvatarChange::Removed { .. } => {}
            RemoteAvatarChange::Faulted { id, category } => {
                self.state = RemoteAvatarPresentationState::Fault { id, category };
            }
        }
    }

    fn hydrate(&mut self, anchor: Option<WorldPose>, id: RemoteAvatarId, pose: WorldPose) {
        if anchor.is_none_or(|anchor| anchor.map_id != pose.map_id) {
            self.state = RemoteAvatarPresentationState::MapContextUnavailable { id };
            return;
        }
        self.state = RemoteAvatarPresentationState::Present {
            id,
            realm_observed_pose: pose,
            rendered_pose: pose,
            smooth: false,
        };
    }

    fn smooth(&mut self, delta_seconds: f32) {
        let RemoteAvatarPresentationState::Present {
            id,
            realm_observed_pose,
            rendered_pose,
            smooth: true,
            ..
        } = self.state
        else {
            return;
        };
        let distance = planar_distance(rendered_pose, realm_observed_pose);
        let blend = if distance <= f32::EPSILON {
            1.0
        } else {
            (REMOTE_SMOOTH_METRES_PER_SECOND * delta_seconds.max(0.0) / distance).min(1.0)
        };
        let rendered_pose = WorldPose {
            east: rendered_pose.east + (realm_observed_pose.east - rendered_pose.east) * blend,
            north: rendered_pose.north + (realm_observed_pose.north - rendered_pose.north) * blend,
            elevation: rendered_pose.elevation
                + (realm_observed_pose.elevation - rendered_pose.elevation) * blend,
            orientation: normalize_angle(
                rendered_pose.orientation
                    + shortest_angle(rendered_pose.orientation, realm_observed_pose.orientation)
                        * blend,
            ),
            ..realm_observed_pose
        };
        self.state = RemoteAvatarPresentationState::Present {
            id,
            realm_observed_pose,
            rendered_pose,
            smooth: blend < 1.0,
        };
    }
}

fn planar_distance(left: WorldPose, right: WorldPose) -> f32 {
    (left.east - right.east).hypot(left.north - right.north)
}

fn shortest_angle(from: f32, to: f32) -> f32 {
    normalize_angle(to - from)
}

fn normalize_angle(angle: f32) -> f32 {
    (angle + PI).rem_euclid(2.0 * PI) - PI
}

pub(crate) struct RemoteAvatarPlugin;

impl Plugin for RemoteAvatarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RemoteAvatarPresentation>().add_systems(
            Update,
            project_remote_avatar.in_set(ClientScheduleSet::Presentation),
        );
    }
}

pub(crate) fn project_remote_avatar(
    view: Res<DiagnosticView>,
    mut ingress: ResMut<RemoteAvatarIngress>,
    mut presentation: ResMut<RemoteAvatarPresentation>,
    time: Res<Time>,
) {
    if ingress.0.len() > client_session::EVENT_CAPACITY {
        presentation.clear();
        ingress.0.clear();
        return;
    }
    presentation.project(view.snapshot(), &ingress.0, time.delta_secs());
    ingress.0.clear();
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use bevy::prelude::{App, MinimalPlugins};
    use client_session::{
        BoundaryError, ClientEvent, ClientEventKind, ClientSnapshot, ControlCommand,
        MovementIntent, RemoteAvatarChange, RemoteAvatarId, RemoteAvatarSnapshot,
        SanitizedIdentity,
    };

    use crate::{
        DiagnosticMode, DiagnosticSession, DiagnosticView, LearningClientPlugin, SessionBridge,
    };

    use super::{RemoteAvatarPresentation, RemoteAvatarPresentationState};

    fn snapshot() -> ClientSnapshot {
        let mut snapshot =
            ClientSnapshot::offline(SanitizedIdentity::new(1, "Realm", "Local", 12_340).unwrap());
        snapshot.phase = client_session::ClientPhase::MovementReady;
        snapshot.entry_anchor = Some(client_session::WorldPose::origin(0));
        snapshot
    }

    fn pose(east: f32, orientation: f32) -> client_session::WorldPose {
        client_session::WorldPose {
            map_id: 0,
            east,
            north: 0.0,
            elevation: 1.0,
            orientation,
        }
    }

    #[test]
    fn remote_projection_snaps_created_smooths_small_updates_and_snaps_at_boundary() {
        let id = RemoteAvatarId::from_realm_guid(7).unwrap();
        let mut presentation = RemoteAvatarPresentation::default();
        let snapshot = snapshot();
        presentation.project(
            &snapshot,
            &[ClientEvent {
                sequence: 3,
                kind: ClientEventKind::RemoteAvatar {
                    change: RemoteAvatarChange::Created {
                        id,
                        realm_observed_pose: pose(0.0, 3.1),
                    },
                },
            }],
            0.0,
        );
        presentation.project(
            &snapshot,
            &[ClientEvent {
                sequence: 4,
                kind: ClientEventKind::RemoteAvatar {
                    change: RemoteAvatarChange::Updated {
                        id,
                        realm_observed_pose: pose(1.0, -3.1),
                    },
                },
            }],
            0.05,
        );
        let RemoteAvatarPresentationState::Present {
            rendered_pose,
            smooth,
            ..
        } = presentation.state
        else {
            panic!("remote is present")
        };
        assert!(smooth);
        assert!(rendered_pose.east > 0.0 && rendered_pose.east < 1.0);
        assert!(rendered_pose.orientation.abs() > 3.0);
        presentation.project(
            &snapshot,
            &[ClientEvent {
                sequence: 5,
                kind: ClientEventKind::RemoteAvatar {
                    change: RemoteAvatarChange::Updated {
                        id,
                        realm_observed_pose: pose(2.028, 0.0),
                    },
                },
            }],
            0.0,
        );
        assert!(
            matches!(presentation.state, RemoteAvatarPresentationState::Present { rendered_pose, smooth: false, .. } if (rendered_pose.east - 2.028).abs() < f32::EPSILON)
        );
    }

    #[test]
    fn remote_projection_fence_and_map_mismatch_clear_without_replaying_old_events() {
        let id = RemoteAvatarId::from_realm_guid(7).unwrap();
        let mut presentation = RemoteAvatarPresentation::default();
        let mut snapshot = snapshot();
        presentation.project(
            &snapshot,
            &[ClientEvent {
                sequence: 3,
                kind: ClientEventKind::RemoteAvatar {
                    change: RemoteAvatarChange::Created {
                        id,
                        realm_observed_pose: pose(0.0, 0.0),
                    },
                },
            }],
            0.0,
        );
        snapshot.remote_avatar_invalidated_through = 3;
        presentation.project(
            &snapshot,
            &[ClientEvent {
                sequence: 3,
                kind: ClientEventKind::RemoteAvatar {
                    change: RemoteAvatarChange::Created {
                        id,
                        realm_observed_pose: pose(0.0, 0.0),
                    },
                },
            }],
            0.0,
        );
        assert!(matches!(
            presentation.state,
            RemoteAvatarPresentationState::Absent
        ));
        presentation.project(
            &snapshot,
            &[ClientEvent {
                sequence: 4,
                kind: ClientEventKind::RemoteAvatar {
                    change: RemoteAvatarChange::Updated {
                        id,
                        realm_observed_pose: client_session::WorldPose {
                            map_id: 1,
                            ..pose(0.0, 0.0)
                        },
                    },
                },
            }],
            0.0,
        );
        assert!(matches!(
            presentation.state,
            RemoteAvatarPresentationState::MapContextUnavailable { .. }
        ));

        snapshot.remote_avatar = Some(RemoteAvatarSnapshot {
            id,
            realm_observed_pose: pose(0.25, 0.5),
            source_sequence: 4,
        });
        presentation.project(&snapshot, &[], 0.0);
        assert!(matches!(
            presentation.state,
            RemoteAvatarPresentationState::Present { id: observed_id, rendered_pose, smooth: false, .. }
                if observed_id == id && (rendered_pose.east - 0.25).abs() < f32::EPSILON
        ));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn remote_projection_ignores_old_events_and_clears_on_fault_or_offline() {
        let id = RemoteAvatarId::from_realm_guid(7).unwrap();
        let mut presentation = RemoteAvatarPresentation::default();
        let mut snapshot = snapshot();
        presentation.project(
            &snapshot,
            &[ClientEvent {
                sequence: 8,
                kind: ClientEventKind::RemoteAvatar {
                    change: RemoteAvatarChange::Created {
                        id,
                        realm_observed_pose: pose(1.0, 0.0),
                    },
                },
            }],
            0.0,
        );
        presentation.project(
            &snapshot,
            &[ClientEvent {
                sequence: 7,
                kind: ClientEventKind::RemoteAvatar {
                    change: RemoteAvatarChange::Removed {
                        id,
                        source: client_session::RemoteAvatarRemovalSource::DestroyObject,
                    },
                },
            }],
            0.0,
        );
        assert!(matches!(
            presentation.state,
            RemoteAvatarPresentationState::Present { .. }
        ));
        presentation.project(
            &snapshot,
            &[ClientEvent {
                sequence: 9,
                kind: ClientEventKind::RemoteAvatar {
                    change: RemoteAvatarChange::Removed {
                        id,
                        source: client_session::RemoteAvatarRemovalSource::DestroyObject,
                    },
                },
            }],
            0.0,
        );
        assert!(matches!(
            presentation.state,
            RemoteAvatarPresentationState::Absent
        ));
        presentation.project(
            &snapshot,
            &[ClientEvent {
                sequence: 10,
                kind: ClientEventKind::RemoteAvatar {
                    change: RemoteAvatarChange::Created {
                        id,
                        realm_observed_pose: pose(1.0, 0.0),
                    },
                },
            }],
            0.0,
        );
        presentation.project(
            &snapshot,
            &[ClientEvent {
                sequence: 11,
                kind: ClientEventKind::RemoteAvatar {
                    change: RemoteAvatarChange::Faulted {
                        id,
                        category: client_session::RemoteAvatarFaultCategory::InvalidPose,
                    },
                },
            }],
            0.0,
        );
        assert!(matches!(
            presentation.state,
            RemoteAvatarPresentationState::Fault { id: observed_id, .. } if observed_id == id
        ));
        presentation.project(
            &snapshot,
            &[ClientEvent {
                sequence: 12,
                kind: ClientEventKind::RemoteAvatar {
                    change: RemoteAvatarChange::Updated {
                        id,
                        realm_observed_pose: pose(2.0, 0.0),
                    },
                },
            }],
            0.0,
        );
        assert!(matches!(
            presentation.state,
            RemoteAvatarPresentationState::Fault { id: observed_id, .. } if observed_id == id
        ));
        presentation.project(
            &snapshot,
            &[ClientEvent {
                sequence: 13,
                kind: ClientEventKind::RemoteAvatar {
                    change: RemoteAvatarChange::Removed {
                        id,
                        source: client_session::RemoteAvatarRemovalSource::DestroyObject,
                    },
                },
            }],
            0.0,
        );
        assert!(matches!(
            presentation.state,
            RemoteAvatarPresentationState::Fault { id: observed_id, .. } if observed_id == id
        ));
        snapshot.phase = client_session::ClientPhase::Offline;
        presentation.project(&snapshot, &[], 0.0);
        assert!(matches!(
            presentation.state,
            RemoteAvatarPresentationState::Absent
        ));
    }

    #[test]
    fn fence_keeps_newer_same_batch_recovery_and_more_than_tail_events() {
        let id = RemoteAvatarId::from_realm_guid(7).unwrap();
        let mut presentation = RemoteAvatarPresentation::default();
        let mut snapshot = snapshot();
        snapshot.remote_avatar_invalidated_through = 10;
        let events = (10..20)
            .map(|sequence| ClientEvent {
                sequence,
                kind: ClientEventKind::RemoteAvatar {
                    change: if sequence <= 11 {
                        RemoteAvatarChange::Created {
                            id,
                            realm_observed_pose: pose(0.0, 0.0),
                        }
                    } else {
                        RemoteAvatarChange::Updated {
                            id,
                            realm_observed_pose: pose(
                                f32::from(u8::try_from(sequence - 11).unwrap()) * 0.1,
                                0.0,
                            ),
                        }
                    },
                },
            })
            .collect::<Vec<_>>();

        presentation.project(&snapshot, &events, 0.0);

        assert!(matches!(
            presentation.state,
            RemoteAvatarPresentationState::Present { realm_observed_pose, .. }
                if (realm_observed_pose.east - 0.8).abs() < f32::EPSILON
        ));
        assert_eq!(presentation.last_event_sequence, 19);
    }

    #[test]
    fn out_of_order_event_cannot_mutate_remote_truth_and_is_diagnosed() {
        let id = RemoteAvatarId::from_realm_guid(7).unwrap();
        let mut presentation = RemoteAvatarPresentation::default();
        let snapshot = snapshot();
        presentation.project(
            &snapshot,
            &[ClientEvent {
                sequence: 2,
                kind: ClientEventKind::RemoteAvatar {
                    change: RemoteAvatarChange::Created {
                        id,
                        realm_observed_pose: pose(0.0, 0.0),
                    },
                },
            }],
            0.0,
        );
        presentation.project(
            &snapshot,
            &[ClientEvent {
                sequence: 1,
                kind: ClientEventKind::RemoteAvatar {
                    change: RemoteAvatarChange::Updated {
                        id,
                        realm_observed_pose: pose(99.0, 0.0),
                    },
                },
            }],
            0.0,
        );
        assert!(matches!(
            presentation.state,
            RemoteAvatarPresentationState::Present { rendered_pose, .. }
                if rendered_pose.east == 0.0
        ));
        assert_eq!(
            presentation.last_ingress_diagnostic,
            Some("OUT-OF-ORDER EVENT IGNORED")
        );
    }

    #[test]
    fn headless_ingress_projects_all_remote_events_beyond_the_visible_tail() {
        let first = RemoteAvatarId::from_realm_guid(7).unwrap();
        let second = RemoteAvatarId::from_realm_guid(8).unwrap();
        let mut expected = snapshot();
        expected.predicted_pose = Some(pose(3.0, 0.3));
        expected.submitted_pose = Some(pose(2.0, 0.2));
        expected.realm_observed_pose = Some(pose(1.0, 0.1));
        let events = (1..=10)
            .map(|sequence| ClientEvent {
                sequence,
                kind: ClientEventKind::RemoteAvatar {
                    change: match sequence {
                        1 => RemoteAvatarChange::Created {
                            id: first,
                            realm_observed_pose: pose(0.0, 0.0),
                        },
                        2 => RemoteAvatarChange::Removed {
                            id: first,
                            source: client_session::RemoteAvatarRemovalSource::DestroyObject,
                        },
                        3 => RemoteAvatarChange::Created {
                            id: second,
                            realm_observed_pose: pose(0.0, 0.0),
                        },
                        _ => RemoteAvatarChange::Updated {
                            id: second,
                            realm_observed_pose: pose(
                                f32::from(u8::try_from(sequence - 3).unwrap()) * 0.1,
                                0.0,
                            ),
                        },
                    },
                },
            })
            .collect();
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(SessionBridge::new(FakeSession::new(
                expected.clone(),
                events,
            )))
            .add_plugins(LearningClientPlugin::headless());

        app.update();

        let projection = app.world().resource::<RemoteAvatarPresentation>();
        let RemoteAvatarPresentationState::Present {
            id,
            realm_observed_pose,
            ..
        } = projection.state
        else {
            panic!(
                "expected a second remote avatar, got {:?}",
                projection.state
            );
        };
        assert_eq!(id, second);
        assert!((realm_observed_pose.east - 0.7).abs() < f32::EPSILON);
        let view = app.world().resource::<DiagnosticView>();
        assert_eq!(view.recent_events().count(), 8);
        assert_eq!(view.snapshot().predicted_pose, expected.predicted_pose);
        assert_eq!(view.snapshot().submitted_pose, expected.submitted_pose);
        assert_eq!(
            view.snapshot().realm_observed_pose,
            expected.realm_observed_pose
        );
    }

    struct FakeSession {
        snapshot: ClientSnapshot,
        events: Mutex<Vec<ClientEvent>>,
    }

    impl FakeSession {
        fn new(snapshot: ClientSnapshot, events: Vec<ClientEvent>) -> Self {
            Self {
                snapshot,
                events: Mutex::new(events),
            }
        }
    }

    impl DiagnosticSession for FakeSession {
        fn snapshot(&self) -> ClientSnapshot {
            self.snapshot.clone()
        }

        fn drain_events(&self) -> Vec<ClientEvent> {
            std::mem::take(&mut *self.events.lock().unwrap())
        }

        fn send_control(&self, _command: ControlCommand) -> Result<(), BoundaryError> {
            Ok(())
        }

        fn publish_movement_intent(&self, _intent: MovementIntent) -> Result<(), BoundaryError> {
            Ok(())
        }

        fn diagnostic_mode(&self) -> DiagnosticMode {
            DiagnosticMode::LiveEntry
        }
    }
}
