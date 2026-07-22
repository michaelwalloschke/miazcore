use std::fmt::Write as _;

use bevy::prelude::*;
use client_session::{ClientEventKind, ClientPhase};

use crate::{ClientScheduleSet, DiagnosticView, world::OfflinePresentation};

const INK: Color = Color::srgb(0.93, 0.96, 0.93);
const MUTED: Color = Color::srgb(0.58, 0.65, 0.62);
const CYAN: Color = Color::srgb(0.41, 0.85, 0.86);
const AMBER: Color = Color::srgb(0.94, 0.74, 0.41);
const LIME: Color = Color::srgb(0.71, 0.95, 0.42);
const PANEL: Color = Color::srgba(0.025, 0.042, 0.034, 0.93);

#[derive(Component)]
enum DiagnosticText {
    Header,
    Ladder,
    Inspector,
    Events,
    Acceptance,
}

pub(crate) struct DiagnosticsPlugin;

impl Plugin for DiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_diagnostics).add_systems(
            Update,
            update_diagnostics.in_set(ClientScheduleSet::Diagnostics),
        );
    }
}

fn setup_diagnostics(mut commands: Commands) {
    spawn_text_panel(
        &mut commands,
        DiagnosticText::Header,
        "",
        20.0,
        INK,
        Node {
            position_type: PositionType::Absolute,
            top: px(0),
            left: px(0),
            right: px(0),
            height: px(76),
            padding: UiRect::axes(px(22), px(13)),
            ..default()
        },
    );
    spawn_text_panel(
        &mut commands,
        DiagnosticText::Ladder,
        "",
        13.0,
        MUTED,
        Node {
            position_type: PositionType::Absolute,
            top: px(77),
            bottom: px(174),
            left: px(0),
            width: px(214),
            padding: UiRect::all(px(18)),
            ..default()
        },
    );
    spawn_text_panel(
        &mut commands,
        DiagnosticText::Inspector,
        "",
        13.0,
        INK,
        Node {
            position_type: PositionType::Absolute,
            top: px(77),
            right: px(0),
            bottom: px(174),
            width: px(320),
            padding: UiRect::all(px(18)),
            ..default()
        },
    );
    spawn_text_panel(
        &mut commands,
        DiagnosticText::Events,
        "",
        12.0,
        MUTED,
        Node {
            position_type: PositionType::Absolute,
            left: px(214),
            right: px(360),
            bottom: px(0),
            height: px(173),
            padding: UiRect::all(px(16)),
            ..default()
        },
    );
    spawn_text_panel(
        &mut commands,
        DiagnosticText::Acceptance,
        "",
        12.0,
        LIME,
        Node {
            position_type: PositionType::Absolute,
            right: px(0),
            bottom: px(0),
            width: px(360),
            height: px(173),
            padding: UiRect::all(px(16)),
            ..default()
        },
    );
}

fn spawn_text_panel(
    commands: &mut Commands,
    marker: DiagnosticText,
    value: &str,
    font_size: f32,
    color: Color,
    node: Node,
) {
    commands.spawn((
        marker,
        Text::new(value),
        TextFont {
            font_size: FontSize::Px(font_size),
            ..default()
        },
        TextColor(color),
        node,
        BackgroundColor(PANEL),
        ZIndex(10),
    ));
}

fn update_diagnostics(
    view: Res<DiagnosticView>,
    presentation: Res<OfflinePresentation>,
    mut texts: Query<(&DiagnosticText, &mut Text, &mut TextColor)>,
) {
    let snapshot = view.snapshot();
    for (marker, mut text, mut color) in &mut texts {
        match marker {
            DiagnosticText::Header => {
                text.0 = format!(
                    "OFFLINE  /  DIAGNOSTIC WORLD\n{}  /  REALM {}  /  BUILD {}  /  {}",
                    snapshot.identity.realm_name(),
                    snapshot.identity.realm_id(),
                    snapshot.identity.client_build(),
                    snapshot.identity.character_name(),
                );
                color.0 = INK;
            }
            DiagnosticText::Ladder => {
                "SESSION LADDER\n\n>  OFFLINE\n-  LOGIN\n-  REALM SELECTION\n-  WORLD AUTH\n-  CHARACTER\n-  BOOTSTRAP\n-  MOVEMENT READY\n\nSLICE 12\nPresentation sandbox only.\nNetwork capability is absent."
                    .clone_into(&mut text.0);
                color.0 = MUTED;
            }
            DiagnosticText::Inspector => {
                let counters = snapshot.queue_counters;
                let submitted = format_pose("SUBMITTED POSE", snapshot.submitted_pose);
                let observed = format_pose("REALM-OBSERVED POSE", snapshot.realm_observed_pose);
                text.0 = format!(
                    "IDENTITY & POSES\n\nRENDERED / OFFLINE DISPLAY\nspace       {:>7.2}  {:>7.2}\n\n{submitted}\n\n{observed}\n\nBOUNDARY\ncontrol  {:>2}/16\nevents   {:>2}/64\nintent revision  {:>3}\n\nNO SOCKETS / NO PACKETS",
                    presentation.rendered_planar.x,
                    presentation.rendered_planar.y,
                    counters.control_queued,
                    counters.event_queued,
                    counters.movement_revision,
                );
                color.0 = CYAN;
            }
            DiagnosticText::Events => {
                text.0 = format_event_tail(&view);
                color.0 = MUTED;
            }
            DiagnosticText::Acceptance => {
                let offline = snapshot.phase == ClientPhase::Offline;
                text.0 = format!(
                    "{}  OFFLINE GATE\n\nWASD  display-only movement\nRMB   orbit camera\nWHEEL / Q E   zoom\nARROWS   orbit fallback\n\nSubmitted and Realm-observed poses are unavailable. Their markers remain hidden. No network claim.",
                    if offline { "PASS" } else { "WAIT" }
                );
                color.0 = if offline { LIME } else { AMBER };
            }
        }
    }
}

fn format_pose(label: &str, pose: Option<client_session::WorldPose>) -> String {
    if let Some(pose) = pose {
        format!(
            "{label}\nmap {:>4}  {:>7.2}  {:>7.2}  {:>7.2}",
            pose.map_id, pose.east, pose.north, pose.elevation
        )
    } else {
        format!("{label}\nNOT AVAILABLE / NO REALM EVIDENCE")
    }
}

fn format_event_tail(view: &DiagnosticView) -> String {
    let mut output = String::from("RECENT SEMANTIC EVENTS\n");
    for event in view.recent_events().rev().take(4) {
        let description = match &event.kind {
            ClientEventKind::PhaseChanged { phase } => format!("phase -> {phase:?}"),
            ClientEventKind::IdentityConfigured { identity } => format!(
                "identity -> realm {} / {}",
                identity.realm_id(),
                identity.character_name()
            ),
            ClientEventKind::RealmDiscovered { realm } => format!(
                "realm discovered -> {} / {}",
                realm.realm_id(),
                realm.endpoint()
            ),
            ClientEventKind::CharacterSelected { character } => format!(
                "character selected -> {} / {}",
                character.guid(),
                character.name()
            ),
            ClientEventKind::PoseObserved { source, .. } => format!("pose observed / {source:?}"),
            ClientEventKind::MovementSubmitted { .. } => "movement submitted".to_owned(),
            ClientEventKind::CommandRejected { command, failure } => format!(
                "{command:?} rejected / {:?} / {}",
                failure.category(),
                failure.context()
            ),
            ClientEventKind::Disconnected => "disconnected".to_owned(),
        };
        let _ = write!(output, "\n+{:04}  {description}", event.sequence);
    }
    output
}

#[cfg(test)]
mod tests {
    use client_session::{ClientEvent, ClientEventKind, ClientSnapshot, SanitizedIdentity};

    use crate::DiagnosticView;

    use super::{format_event_tail, format_pose};

    #[test]
    fn semantic_event_tail_contains_no_credential_vocabulary() {
        let identity =
            SanitizedIdentity::new(1, "Miazcore Reference Realm", "Miaztest", 12_340).unwrap();
        let view = DiagnosticView {
            snapshot: ClientSnapshot::offline(identity.clone()),
            recent_events: [ClientEvent {
                sequence: 1,
                kind: ClientEventKind::IdentityConfigured { identity },
            }]
            .into(),
        };

        let output = format_event_tail(&view).to_ascii_lowercase();
        assert!(!output.contains("password"));
        assert!(!output.contains("session key"));
        assert!(!output.contains("packet body"));
    }

    #[test]
    fn absent_pose_is_never_formatted_as_an_origin_observation() {
        let output = format_pose("REALM-OBSERVED POSE", None);
        assert!(output.contains("NOT AVAILABLE"));
        assert!(!output.contains("map 0"));
    }
}
