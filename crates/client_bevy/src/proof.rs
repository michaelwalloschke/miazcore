use std::{
    fmt::Write as _,
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
};

use crate::{
    ClientScheduleSet, DiagnosticView, SessionBridge, camera::CameraRig,
    world::DiagnosticPresentation,
};

// Leave enough presented frames for Metal pipeline creation on a cold shader cache.
const CAPTURE_FRAME: u32 = 180;
const TIMEOUT_FRAME: u32 = 900;
const PRESENTATION_SETTLE_DELAY: Duration = Duration::from_secs(3);

pub struct RenderProofPlugin {
    output: PathBuf,
    mode: RenderProofMode,
    backend: CaptureBackend,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RenderProofMode {
    Offline,
    LiveEntry,
    LiveMovement,
    PersistedMovement,
    PersistedMovementRejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CaptureBackend {
    Bevy,
    External,
}

impl RenderProofPlugin {
    #[must_use]
    pub fn new(output: impl Into<PathBuf>) -> Self {
        Self {
            output: output.into(),
            mode: RenderProofMode::Offline,
            backend: CaptureBackend::Bevy,
        }
    }

    /// Produce a bounded Metal proof after the real session reaches
    /// `MovementReady` through the same complete command as the visible action.
    #[must_use]
    pub fn live_entry(output: impl Into<PathBuf>) -> Self {
        Self {
            output: output.into(),
            mode: RenderProofMode::LiveEntry,
            backend: CaptureBackend::Bevy,
        }
    }

    #[must_use]
    pub fn external(output: impl Into<PathBuf>, live_entry: bool) -> Self {
        Self {
            output: output.into(),
            mode: if live_entry {
                RenderProofMode::LiveEntry
            } else {
                RenderProofMode::Offline
            },
            backend: CaptureBackend::External,
        }
    }

    #[must_use]
    pub fn live_movement_external(output: impl Into<PathBuf>) -> Self {
        Self {
            output: output.into(),
            mode: RenderProofMode::LiveMovement,
            backend: CaptureBackend::External,
        }
    }

    #[must_use]
    pub fn persisted_movement_external(output: impl Into<PathBuf>) -> Self {
        Self {
            output: output.into(),
            mode: RenderProofMode::PersistedMovement,
            backend: CaptureBackend::External,
        }
    }

    /// Live negative probe: prove that an ineligible short move is rejected
    /// before logout or reconnect can be attempted.
    #[must_use]
    pub fn persisted_movement_short_negative_external(output: impl Into<PathBuf>) -> Self {
        Self {
            output: output.into(),
            mode: RenderProofMode::PersistedMovementRejected,
            backend: CaptureBackend::External,
        }
    }
}

impl Plugin for RenderProofPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(RenderProofState {
            output: self.output.clone(),
            ready: self.output.with_extension("ready"),
            stage: self.output.with_extension("stage"),
            frame: 0,
            requested: false,
            ready_written: false,
            scripted: false,
            ready_frame: None,
            capture_not_before: None,
            movement_started_at: None,
            movement_turned: false,
            movement_stopped: false,
            verification_requested: false,
            mode: self.mode,
            backend: self.backend,
        })
        .add_systems(Update, script_render_proof.in_set(ClientScheduleSet::Input))
        .add_systems(
            Update,
            capture_render_proof.in_set(ClientScheduleSet::Diagnostics),
        );
    }
}

#[allow(clippy::struct_excessive_bools)] // independent proof lifecycle latches
#[derive(Resource)]
struct RenderProofState {
    output: PathBuf,
    ready: PathBuf,
    stage: PathBuf,
    frame: u32,
    requested: bool,
    ready_written: bool,
    scripted: bool,
    ready_frame: Option<u32>,
    capture_not_before: Option<Instant>,
    movement_started_at: Option<Instant>,
    movement_turned: bool,
    movement_stopped: bool,
    verification_requested: bool,
    mode: RenderProofMode,
    backend: CaptureBackend,
}

fn script_render_proof(
    mut proof: ResMut<RenderProofState>,
    mut presentation: ResMut<DiagnosticPresentation>,
    mut camera: ResMut<CameraRig>,
    session: Res<SessionBridge>,
) {
    if proof.scripted {
        return;
    }
    if let Some(parent) = proof.output.parent() {
        fs::create_dir_all(parent).expect("proof artifact directory should be creatable");
    }
    if proof.output.exists() {
        fs::remove_file(&proof.output).expect("old proof artifact should be replaceable");
    }
    let sidecar = proof.output.with_extension("json");
    if sidecar.exists() {
        fs::remove_file(sidecar).expect("old proof sidecar should be replaceable");
    }
    if proof.ready.exists() {
        fs::remove_file(&proof.ready).expect("old proof ready marker should be replaceable");
    }
    if proof.stage.exists() {
        fs::remove_file(&proof.stage).expect("old proof stage marker should be replaceable");
    }
    match proof.mode {
        RenderProofMode::Offline => {
            presentation.set_proof_pose();
            camera.set_proof_view();
        }
        RenderProofMode::LiveEntry
        | RenderProofMode::LiveMovement
        | RenderProofMode::PersistedMovement
        | RenderProofMode::PersistedMovementRejected => {
            assert!(
                session.is_live_entry(),
                "live proof requires a live entry session"
            );
            session
                .start_entry()
                .expect("live proof session should accept the complete entry operation");
        }
    }
    if proof.mode == RenderProofMode::Offline {
        proof.capture_not_before = Some(Instant::now() + PRESENTATION_SETTLE_DELAY);
    }
    proof.scripted = true;
}

#[allow(clippy::too_many_lines)] // one explicit proof lifecycle keeps capture sequencing auditable
fn capture_render_proof(
    mut commands: Commands,
    mut proof: ResMut<RenderProofState>,
    presentation: Res<DiagnosticPresentation>,
    view: Res<DiagnosticView>,
    session: Res<SessionBridge>,
    mut exit: MessageWriter<AppExit>,
) {
    proof.frame += 1;
    if matches!(
        proof.mode,
        RenderProofMode::LiveEntry
            | RenderProofMode::LiveMovement
            | RenderProofMode::PersistedMovement
            | RenderProofMode::PersistedMovementRejected
    ) && proof.ready_frame.is_none()
    {
        match view.snapshot().phase {
            client_session::ClientPhase::MovementReady => {
                proof.ready_frame = Some(proof.frame);
                if matches!(
                    proof.mode,
                    RenderProofMode::LiveMovement | RenderProofMode::PersistedMovement
                ) {
                    session
                        .publish_movement_intent(
                            client_session::MovementIntent::planar(1.0, 0.0)
                                .expect("finite proof intent"),
                        )
                        .expect("live movement proof should accept start intent");
                    proof.movement_started_at = Some(Instant::now());
                } else if proof.mode == RenderProofMode::PersistedMovementRejected {
                    // A zero-distance stopped pose is the deterministic lower
                    // boundary of the less-than-two-metre eligibility rule.
                    proof.movement_stopped = true;
                    proof.capture_not_before = Some(Instant::now() + Duration::from_millis(500));
                } else {
                    proof.capture_not_before = Some(Instant::now() + PRESENTATION_SETTLE_DELAY);
                }
            }
            client_session::ClientPhase::Failed(_) => {
                panic!(
                    "live proof session failed before MovementReady: {:?}",
                    view.snapshot().latest_failure
                )
            }
            _ if proof.frame > TIMEOUT_FRAME => {
                panic!("timed out while waiting for MovementReady")
            }
            _ => return,
        }
    }
    if matches!(
        proof.mode,
        RenderProofMode::LiveMovement
            | RenderProofMode::PersistedMovement
            | RenderProofMode::PersistedMovementRejected
    ) && !proof.movement_stopped
        && !proof.verification_requested
    {
        if proof.mode == RenderProofMode::LiveMovement
            && !proof.movement_turned
            && proof
                .movement_started_at
                .is_some_and(|started| started.elapsed() >= Duration::from_secs(1))
        {
            session
                .publish_movement_intent(
                    client_session::MovementIntent::planar(0.0, 1.0)
                        .expect("finite proof turn intent"),
                )
                .expect("live movement proof should accept turn intent");
            proof.movement_turned = true;
        }
        if proof.movement_started_at.is_some_and(|started| {
            started.elapsed()
                >= if matches!(
                    proof.mode,
                    RenderProofMode::PersistedMovement | RenderProofMode::PersistedMovementRejected
                ) {
                    if proof.mode == RenderProofMode::PersistedMovementRejected {
                        Duration::from_millis(100)
                    } else {
                        Duration::from_millis(450)
                    }
                } else {
                    PRESENTATION_SETTLE_DELAY
                }
        }) {
            session
                .publish_movement_intent(client_session::MovementIntent::idle())
                .expect("live movement proof should accept stop intent");
            proof.movement_stopped = true;
            proof.capture_not_before = Some(Instant::now() + Duration::from_millis(500));
        } else {
            return;
        }
    }
    if matches!(
        proof.mode,
        RenderProofMode::PersistedMovement | RenderProofMode::PersistedMovementRejected
    ) {
        // The negative renderer must continue to the compositor capture after
        // submitting its one deliberately ineligible operation. The sidecar
        // validator, not rendering timing, asserts the rejection evidence.
        let short_move_rejected = proof.mode == RenderProofMode::PersistedMovementRejected
            && proof.verification_requested;
        if short_move_rejected && !proof.ready_written {
            fs::write(&proof.ready, "miazcore external compositor capture ready\n")
                .expect("negative proof ready marker should be writable");
            info!(
                "external compositor capture ready at {}",
                proof.ready.display()
            );
            proof.ready_written = true;
        }
        if !short_move_rejected {
            match view.snapshot().phase {
                client_session::ClientPhase::MovementReady
                    if proof.movement_stopped && !proof.verification_requested =>
                {
                    fs::write(&proof.stage, "begin-movement-proof\n")
                        .expect("proof stage marker should be writable");
                    session.verify_persisted_movement().expect(
                        "persisted proof should accept the semantic verification operation",
                    );
                    proof.verification_requested = true;
                    return;
                }
                client_session::ClientPhase::MovementReady
                    if proof.mode == RenderProofMode::PersistedMovementRejected
                        && proof.verification_requested => {}
                client_session::ClientPhase::ProvingMovement(
                    client_session::ProofStage::Comparing,
                ) if proof.mode == RenderProofMode::PersistedMovement
                    && view
                        .snapshot()
                        .movement_proof
                        .is_some_and(client_session::MovementProofEvidence::passed) => {}
                client_session::ClientPhase::ProvingMovement(
                    client_session::ProofStage::Reconnecting,
                ) if proof.mode == RenderProofMode::PersistedMovement => {
                    fs::write(&proof.stage, "reconnecting\n")
                        .expect("proof reconnect stage marker should be writable");
                    return;
                }
                client_session::ClientPhase::Failed(_)
                    if proof.mode == RenderProofMode::PersistedMovementRejected => {}
                client_session::ClientPhase::Failed(_) => panic!(
                    "persisted movement proof failed before fresh reconnect comparison: {:?}",
                    view.snapshot().latest_failure
                ),
                _ if proof.frame > TIMEOUT_FRAME => {
                    panic!("timed out while waiting for persisted movement proof")
                }
                _ => return,
            }
        }
    }
    // A persisted-movement image is evidence of the *completed* fresh-session
    // comparison.  Do not arm an external compositor capture while logout or
    // reconnect is still in flight: an injected reconnect failure must reach
    // the explicit proof failure boundary, rather than timing out waiting for
    // an image that cannot yet substantiate the proof.
    if proof.mode == RenderProofMode::PersistedMovement
        && !matches!(
            view.snapshot().phase,
            client_session::ClientPhase::ProvingMovement(client_session::ProofStage::Comparing)
        )
    {
        return;
    }
    let capture_frame = match proof.mode {
        RenderProofMode::Offline => CAPTURE_FRAME,
        RenderProofMode::LiveEntry
        | RenderProofMode::LiveMovement
        | RenderProofMode::PersistedMovement
        | RenderProofMode::PersistedMovementRejected => proof
            .ready_frame
            .expect("live proof is armed only after MovementReady")
            .saturating_add(CAPTURE_FRAME),
    };
    let presentation_settled = proof
        .capture_not_before
        .is_some_and(|not_before| Instant::now() >= not_before);
    if proof.requested && proof.output.exists() {
        let persisted_comparison_complete = proof.mode == RenderProofMode::PersistedMovement
            && matches!(
                view.snapshot().phase,
                client_session::ClientPhase::ProvingMovement(client_session::ProofStage::Comparing)
            )
            && view
                .snapshot()
                .movement_proof
                .is_some_and(client_session::MovementProofEvidence::passed);
        let expected_short_rejection = proof.mode == RenderProofMode::PersistedMovementRejected;
        if matches!(
            proof.mode,
            RenderProofMode::LiveEntry
                | RenderProofMode::LiveMovement
                | RenderProofMode::PersistedMovement
                | RenderProofMode::PersistedMovementRejected
        ) && view.snapshot().phase != client_session::ClientPhase::MovementReady
            && !persisted_comparison_complete
            && !expected_short_rejection
        {
            panic!("retained world session failed before proof sidecar could be written");
        }
        let sidecar = proof_sidecar(&view, &presentation, proof.mode);
        fs::write(proof.output.with_extension("json"), sidecar)
            .expect("proof sidecar should be writable");
        info!("rendered proof saved to {}", proof.output.display());
        exit.write(AppExit::Success);
    } else if !proof.requested && proof.frame >= capture_frame && presentation_settled {
        match proof.backend {
            CaptureBackend::Bevy => {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(proof.output.clone()));
                proof.requested = true;
            }
            CaptureBackend::External if !proof.ready_written => {
                fs::write(&proof.ready, "miazcore external compositor capture ready\n")
                    .expect("proof ready marker should be writable");
                info!(
                    "external compositor capture ready at {}",
                    proof.ready.display()
                );
                proof.ready_written = true;
            }
            CaptureBackend::External => {
                proof.requested = true;
            }
        }
    } else if proof.frame > capture_frame.saturating_add(TIMEOUT_FRAME) {
        panic!("timed out while waiting for the rendered proof artifact");
    }
}

fn proof_sidecar(
    view: &DiagnosticView,
    presentation: &DiagnosticPresentation,
    mode: RenderProofMode,
) -> String {
    if view.is_live_entry() {
        return live_proof_sidecar(view, presentation, mode);
    }
    let snapshot = view.snapshot();
    let character = json_string(snapshot.identity.character_name());
    format!(
        concat!(
            "{{\n",
            "  \"schema\": \"miazcore.render-proof.v1\",\n",
            "  \"phase\": \"Offline\",\n",
            "  \"network\": \"disabled\",\n",
            "  \"realm_id\": {},\n",
            "  \"client_build\": {},\n",
            "  \"character\": {},\n",
            "  \"rendered_pose\": {{ \"space\": \"offline-display\", \"east\": {:.3}, \"north\": {:.3}, \"elevation\": 0.0 }},\n",
            "  \"submitted_pose\": null,\n",
            "  \"realm_observed_pose\": null\n",
            "}}\n"
        ),
        snapshot.identity.realm_id(),
        snapshot.identity.client_build(),
        character,
        presentation.rendered_planar.x,
        presentation.rendered_planar.y,
    )
}

fn live_proof_sidecar(
    view: &DiagnosticView,
    presentation: &DiagnosticPresentation,
    mode: RenderProofMode,
) -> String {
    let snapshot = view.snapshot();
    let anchor = snapshot
        .entry_anchor
        .expect("live proof only writes after MovementReady");
    let rendered = presentation
        .rendered_pose()
        .expect("live proof projects the authoritative Entry Anchor");
    let observed = snapshot
        .realm_observed_pose
        .expect("MovementReady retains the Realm-observed Entry Anchor");
    let submitted = snapshot
        .submitted_pose
        .expect("MovementReady retains the initial Submitted Pose");
    let predicted = snapshot.predicted_pose.unwrap_or(anchor);
    format!(
        concat!(
            "{{\n",
            "  \"schema\": \"miazcore.live-render-proof.v1\",\n",
            "  \"phase\": {},\n",
            "  \"network\": \"reference-realm\",\n",
            "  \"realm_id\": {},\n",
            "  \"client_build\": {},\n",
            "  \"character\": {},\n",
            "  \"run_speed\": {:.3},\n",
            "  \"movement_publication\": \"{}\",\n",
            "  \"entry_anchor\": {},\n",
            "  \"predicted_pose\": {},\n",
            "  \"rendered_pose\": {},\n",
            "  \"submitted_pose\": {},\n",
            "  \"realm_observed_pose\": {},\n",
            "  \"failure_context\": {},\n",
            "  \"movement_proof\": {}\n",
            "}}\n"
        ),
        json_string(if mode == RenderProofMode::PersistedMovement {
            "PersistedMovementCompared"
        } else if mode == RenderProofMode::PersistedMovementRejected {
            "PersistedMovementRejected"
        } else {
            "MovementReady"
        }),
        snapshot.identity.realm_id(),
        snapshot.identity.client_build(),
        json_string(snapshot.identity.character_name()),
        snapshot
            .run_speed
            .expect("MovementReady has a positive run speed"),
        if matches!(
            mode,
            RenderProofMode::LiveMovement
                | RenderProofMode::PersistedMovement
                | RenderProofMode::PersistedMovementRejected
        ) {
            "bounded-ground"
        } else {
            "disabled"
        },
        pose_json(anchor),
        pose_json(predicted),
        pose_json(rendered),
        pose_json(submitted),
        pose_json(observed),
        snapshot.latest_failure.as_ref().map_or_else(
            || "null".to_owned(),
            |failure| json_string(failure.context()),
        ),
        snapshot.movement_proof.map_or_else(
            || "null".to_owned(),
            |proof| format!(
                "{{ \"source\": \"fresh-reconnect-login-verify-world\", \"expected\": {}, \"observed\": {}, \"delta_metres\": {}, \"tolerance_metres\": {:.3}, \"passed\": {} }}",
                pose_json(proof.expected),
                proof.observed.map_or_else(|| "null".to_owned(), pose_json),
                proof.delta_metres().map_or_else(|| "null".to_owned(), |delta| format!("{delta:.3}")),
                proof.tolerance_metres,
                proof.passed(),
            ),
        ),
    )
}

fn pose_json(pose: client_session::WorldPose) -> String {
    format!(
        "{{ \"map_id\": {}, \"east\": {:.3}, \"north\": {:.3}, \"elevation\": {:.3}, \"orientation\": {:.3} }}",
        pose.map_id, pose.east, pose.north, pose.elevation, pose.orientation,
    )
}

fn json_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\u{08}' => escaped.push_str("\\b"),
            '\u{0c}' => escaped.push_str("\\f"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character <= '\u{1f}' => {
                let _ = write!(escaped, "\\u{:04x}", u32::from(character));
            }
            character => escaped.push(character),
        }
    }
    escaped.push('"');
    escaped
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use client_session::{ClientPhase, ClientSnapshot, SanitizedIdentity, WorldPose};

    use crate::{DiagnosticMode, DiagnosticPresentation, DiagnosticView};

    use super::{RenderProofMode, json_string, proof_sidecar};

    #[test]
    fn sidecar_is_semantic_offline_evidence_without_secret_fields() {
        let identity =
            SanitizedIdentity::new(1, "Miazcore Reference Realm", "Miaztest", 12_340).unwrap();
        let view = DiagnosticView {
            snapshot: ClientSnapshot::offline(identity),
            recent_events: VecDeque::default(),
            mode: DiagnosticMode::Offline,
        };
        let sidecar = proof_sidecar(
            &view,
            &DiagnosticPresentation {
                rendered_planar: bevy::prelude::Vec2::new(2.4, -1.6),
                heading: 2.16,
                entry_anchor: None,
                rendered_pose: None,
            },
            RenderProofMode::Offline,
        );

        assert!(sidecar.contains("\"phase\": \"Offline\""));
        assert!(sidecar.contains("\"network\": \"disabled\""));
        assert!(sidecar.contains("\"space\": \"offline-display\""));
        assert!(sidecar.contains("\"submitted_pose\": null"));
        assert!(!sidecar.to_ascii_lowercase().contains("password"));
        assert!(!sidecar.to_ascii_lowercase().contains("session_key"));
    }

    #[test]
    fn sidecar_identity_is_json_escaped() {
        assert_eq!(json_string("Miaz\\\"test"), "\"Miaz\\\\\\\"test\"");
    }

    #[test]
    fn live_sidecar_keeps_entry_pose_truths_distinct_and_bounds_movement_claims() {
        let identity =
            SanitizedIdentity::new(1, "Miazcore Reference Realm", "Miaztest", 12_340).unwrap();
        let anchor = WorldPose {
            map_id: 0,
            east: -8949.95,
            north: -132.493,
            elevation: 83.5312,
            orientation: 0.0,
        };
        let mut snapshot = ClientSnapshot::offline(identity);
        snapshot.phase = ClientPhase::MovementReady;
        snapshot.entry_anchor = Some(anchor);
        snapshot.submitted_pose = Some(anchor);
        snapshot.realm_observed_pose = Some(anchor);
        snapshot.run_speed = Some(7.0);
        let view = DiagnosticView {
            snapshot,
            recent_events: VecDeque::default(),
            mode: DiagnosticMode::LiveEntry,
        };
        let sidecar = proof_sidecar(
            &view,
            &DiagnosticPresentation {
                rendered_planar: bevy::prelude::Vec2::ZERO,
                heading: 0.0,
                entry_anchor: Some(anchor),
                rendered_pose: Some(anchor),
            },
            RenderProofMode::LiveEntry,
        );

        assert!(sidecar.contains("\"phase\": \"MovementReady\""));
        assert!(sidecar.contains("\"movement_publication\": \"disabled\""));
        assert!(sidecar.contains("\"submitted_pose\": { \"map_id\": 0"));
        assert!(sidecar.contains("\"entry_anchor\": { \"map_id\": 0"));
        assert!(!sidecar.to_ascii_lowercase().contains("password"));
        assert!(!sidecar.to_ascii_lowercase().contains("session_key"));
    }
}
