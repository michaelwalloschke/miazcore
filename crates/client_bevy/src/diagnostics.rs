use std::fmt::Write as _;

use bevy::prelude::*;
use client_session::{ClientEventKind, ClientPhase, EntryStage};

use crate::{ClientScheduleSet, DiagnosticView, SessionBridge, world::DiagnosticPresentation};

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
    Connect,
}

#[derive(Component)]
struct ConnectEntryAction;

#[derive(Debug, Default, Resource)]
struct ConnectActionFeedback(Option<&'static str>);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DisplayPhase {
    Offline,
    Entering(usize),
    MovementReady,
    Failed,
    Proving,
}

fn display_phase(phase: &ClientPhase) -> DisplayPhase {
    match phase {
        ClientPhase::Offline => DisplayPhase::Offline,
        ClientPhase::Entering(EntryStage::LoginConnection) => DisplayPhase::Entering(1),
        ClientPhase::Entering(EntryStage::LoginAuthentication) => DisplayPhase::Entering(2),
        ClientPhase::Entering(EntryStage::RealmSelection) => DisplayPhase::Entering(3),
        ClientPhase::Entering(EntryStage::WorldAuthentication) => DisplayPhase::Entering(4),
        ClientPhase::Entering(EntryStage::CharacterSelection) => DisplayPhase::Entering(5),
        ClientPhase::Entering(EntryStage::Bootstrap) => DisplayPhase::Entering(6),
        ClientPhase::Entering(EntryStage::ControlSynchronization) => DisplayPhase::Entering(7),
        ClientPhase::MovementReady => DisplayPhase::MovementReady,
        ClientPhase::Failed(_) => DisplayPhase::Failed,
        ClientPhase::ProvingMovement(_) => DisplayPhase::Proving,
    }
}

pub(crate) struct DiagnosticsPlugin;

impl Plugin for DiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ConnectActionFeedback>()
            .add_systems(Startup, setup_diagnostics)
            .add_systems(
                Update,
                update_diagnostics.in_set(ClientScheduleSet::Diagnostics),
            )
            .add_systems(
                Update,
                dispatch_connect_entry.in_set(ClientScheduleSet::Input),
            );
    }
}

#[allow(clippy::too_many_lines)]
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
    commands.spawn((
        Name::new("Connect & Enter Reference Realm action"),
        DiagnosticText::Connect,
        ConnectEntryAction,
        Button,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(13.0),
            ..default()
        },
        TextColor(INK),
        Node {
            position_type: PositionType::Absolute,
            left: px(18),
            bottom: px(18),
            width: px(178),
            height: px(62),
            padding: UiRect::all(px(10)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.16, 0.38, 0.34, 0.88)),
        ZIndex(11),
    ));
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
    presentation: Res<DiagnosticPresentation>,
    feedback: Res<ConnectActionFeedback>,
    mut texts: Query<(&DiagnosticText, &mut Text, &mut TextColor)>,
) {
    let snapshot = view.snapshot();
    for (marker, mut text, mut color) in &mut texts {
        match marker {
            DiagnosticText::Header => {
                text.0 = format!(
                    "{}  /  DIAGNOSTIC WORLD\n{}  /  REALM {}  /  BUILD {}  /  {}",
                    if view.is_live_entry() {
                        "REFERENCE REALM"
                    } else {
                        "OFFLINE"
                    },
                    snapshot.identity.realm_name(),
                    snapshot.identity.realm_id(),
                    snapshot.identity.client_build(),
                    snapshot.identity.character_name(),
                );
                color.0 = INK;
            }
            DiagnosticText::Ladder => {
                text.0 =
                    format_session_ladder(display_phase(&snapshot.phase), view.is_live_entry());
                color.0 = MUTED;
            }
            DiagnosticText::Inspector => {
                let counters = snapshot.queue_counters;
                let rendered = if view.is_live_entry() {
                    format_pose("RENDERED POSE", presentation.rendered_pose())
                } else {
                    format!(
                        "RENDERED / OFFLINE DISPLAY\nspace       {:>7.2}  {:>7.2}",
                        presentation.rendered_planar.x, presentation.rendered_planar.y,
                    )
                };
                let anchor = format_pose("ENTRY ANCHOR", snapshot.entry_anchor);
                let predicted = format_pose("PREDICTED POSE", snapshot.predicted_pose);
                let submitted = format_pose("SUBMITTED POSE", snapshot.submitted_pose);
                let observed = format_pose("REALM-OBSERVED POSE", snapshot.realm_observed_pose);
                let correction = snapshot.correction_target.map_or_else(
                    || "SCRIPTED CORRECTION\nNOT ACTIVE".to_owned(),
                    |target| format_pose("SCRIPTED CORRECTION", Some(target.pose())),
                );
                text.0 = format!(
                    "IDENTITY & POSES\n\n{rendered}\n\n{anchor}\n\n{predicted}\n\n{submitted}\n\n{observed}\n\n{correction}\n\nRUN SPEED\n{}\n\nBOUNDARY\ncontrol  {:>2}/16\nevents   {:>2}/64\nintent revision  {:>3}\n\n{}",
                    format_run_speed(snapshot.run_speed),
                    counters.control_queued,
                    counters.event_queued,
                    counters.movement_revision,
                    if view.is_live_entry() {
                        "BOUNDED GROUND MOVEMENT\nNo realm acceptance or persistence claim"
                    } else {
                        "NO SOCKETS / NO PACKETS"
                    },
                );
                color.0 = CYAN;
            }
            DiagnosticText::Events => {
                text.0 = format_event_tail(&view);
                color.0 = MUTED;
            }
            DiagnosticText::Acceptance => {
                let (value, accepted) =
                    format_acceptance(snapshot, view.is_live_entry(), feedback.0);
                text.0 = value;
                color.0 = if accepted { LIME } else { AMBER };
            }
            DiagnosticText::Connect => {
                let display_phase = display_phase(&snapshot.phase);
                text.0 = format_connect_action(display_phase, view.is_live_entry());
                color.0 = if view.is_live_entry()
                    && matches!(display_phase, DisplayPhase::Offline | DisplayPhase::Failed)
                {
                    LIME
                } else {
                    MUTED
                };
            }
        }
    }
}

fn dispatch_connect_entry(
    interactions: Query<&Interaction, (Changed<Interaction>, With<ConnectEntryAction>)>,
    session: Res<SessionBridge>,
    view: Res<DiagnosticView>,
    mut feedback: ResMut<ConnectActionFeedback>,
) {
    for interaction in &interactions {
        if *interaction != Interaction::Pressed || !view.is_live_entry() {
            continue;
        }
        let result = match display_phase(&view.snapshot().phase) {
            DisplayPhase::Offline => session.start_entry(),
            DisplayPhase::Failed => session.retry_entry(),
            DisplayPhase::Entering(_) | DisplayPhase::MovementReady | DisplayPhase::Proving => {
                continue;
            }
        };
        feedback.0 = result
            .err()
            .map(|_| "ENTRY CONTROL UNAVAILABLE / restart the Diagnostic World and retry");
        if result.is_ok() {
            feedback.0 = None;
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

fn format_run_speed(run_speed: Option<f32>) -> String {
    run_speed.map_or_else(
        || "NOT AVAILABLE / INPUT GATED".to_owned(),
        |speed| format!("{speed:.3} m/s / realm-provided"),
    )
}

fn format_session_ladder(phase: DisplayPhase, live_entry: bool) -> String {
    if !live_entry {
        return "SESSION LADDER\n\n>  OFFLINE\n-  LOGIN\n-  REALM SELECTION\n-  WORLD AUTH\n-  CHARACTER\n-  BOOTSTRAP\n-  MOVEMENT READY\n\nOFFLINE PRESENTATION\nNetwork capability is absent."
            .to_owned();
    }
    let active = match phase {
        DisplayPhase::Offline | DisplayPhase::Proving | DisplayPhase::Failed => 0,
        DisplayPhase::Entering(active) => active,
        DisplayPhase::MovementReady => 8,
    };
    let stages = [
        "OFFLINE",
        "LOGIN CONNECT",
        "LOGIN AUTH",
        "REALM SELECTION",
        "WORLD AUTH",
        "CHARACTER",
        "BOOTSTRAP",
        "CONTROL SYNC",
        "MOVEMENT READY",
    ];
    let mut output = String::from("SESSION LADDER\n\n");
    for (index, stage) in stages.iter().enumerate() {
        let marker = if phase == DisplayPhase::Failed {
            if index == 0 { "!" } else { "-" }
        } else if index < active {
            "+"
        } else if index == active {
            ">"
        } else {
            "-"
        };
        let _ = writeln!(output, "{marker}  {stage}");
    }
    output.push_str(
        "\nOne configured entry operation.\nBounded ground movement only; persistence is deferred.",
    );
    output
}

fn format_connect_action(phase: DisplayPhase, live_entry: bool) -> String {
    if !live_entry {
        return "OFFLINE MODE\nNo realm connection".to_owned();
    }
    match phase {
        DisplayPhase::Offline => "CONNECT & ENTER\nREFERENCE REALM".to_owned(),
        DisplayPhase::MovementReady => "MOVEMENT READY\nWASD BOUNDED MOVEMENT".to_owned(),
        DisplayPhase::Failed => "RETRY ENTRY\nREFERENCE REALM".to_owned(),
        DisplayPhase::Entering(_) | DisplayPhase::Proving => {
            "ENTERING REFERENCE REALM\nPlease wait".to_owned()
        }
    }
}

fn format_acceptance(
    snapshot: &client_session::ClientSnapshot,
    live_entry: bool,
    control_feedback: Option<&str>,
) -> (String, bool) {
    if !live_entry {
        let offline = snapshot.phase == ClientPhase::Offline;
        return (
            format!(
                "{}  OFFLINE GATE\n\nWASD  display-only movement\nRMB   orbit camera\nWHEEL / Q E   zoom\nARROWS   orbit fallback\n\nSubmitted and Realm-observed poses are unavailable. Their markers remain hidden. No network claim.",
                if offline { "PASS" } else { "WAIT" }
            ),
            offline,
        );
    }
    match display_phase(&snapshot.phase) {
        DisplayPhase::MovementReady => (
            "PASS  MOVEMENT-READY ENTRY\n\nWASD  bounded ground movement\nRMB   orbit camera\nWHEEL / Q E   zoom\nARROWS   orbit fallback\n\nSubmitted pose is client-written only. Realm acceptance and persistence remain unproven.".to_owned(),
            true,
        ),
        DisplayPhase::Failed => (
            {
                let ClientPhase::Failed(recovery) = &snapshot.phase else {
                    unreachable!("display phase only reports failure for failed snapshots");
                };
                format!(
                "ENTRY FAILED\n\n{:?} / {:?}\n\n{}\n\nRETRY ENTRY restarts one complete configured operation.\nInput stays gated. Submitted pose evidence, if present, remains available.{}",
                recovery.category,
                recovery.action,
                snapshot
                    .latest_failure
                    .as_ref()
                    .map_or("No additional diagnostic", client_session::ClientFailure::context),
                control_feedback.map_or_else(String::new, |message| format!("\n\n{message}")),
                )
            },
            false,
        ),
        DisplayPhase::Offline | DisplayPhase::Entering(_) | DisplayPhase::Proving => (
            "WAITING FOR ENTRY\n\nConnect & Enter starts one complete configured operation.\n\nInput stays gated until the realm reaches MovementReady.".to_owned(),
            false,
        ),
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
            ClientEventKind::ScriptedCorrection { .. } => "scripted correction target".to_owned(),
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

    use crate::{DiagnosticMode, DiagnosticView};

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
            mode: DiagnosticMode::Offline,
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
