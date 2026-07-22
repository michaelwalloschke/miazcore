use std::{fmt::Write as _, fs, path::PathBuf};

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

pub struct RenderProofPlugin {
    output: PathBuf,
    mode: RenderProofMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RenderProofMode {
    Offline,
    LiveEntry,
}

impl RenderProofPlugin {
    #[must_use]
    pub fn new(output: impl Into<PathBuf>) -> Self {
        Self {
            output: output.into(),
            mode: RenderProofMode::Offline,
        }
    }

    /// Produce a bounded Metal proof after the real session reaches
    /// `MovementReady` through the same complete command as the visible action.
    #[must_use]
    pub fn live_entry(output: impl Into<PathBuf>) -> Self {
        Self {
            output: output.into(),
            mode: RenderProofMode::LiveEntry,
        }
    }
}

impl Plugin for RenderProofPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(RenderProofState {
            output: self.output.clone(),
            frame: 0,
            requested: false,
            scripted: false,
            ready_frame: None,
            mode: self.mode,
        })
        .add_systems(Update, script_render_proof.in_set(ClientScheduleSet::Input))
        .add_systems(
            Update,
            capture_render_proof.in_set(ClientScheduleSet::Diagnostics),
        );
    }
}

#[derive(Resource)]
struct RenderProofState {
    output: PathBuf,
    frame: u32,
    requested: bool,
    scripted: bool,
    ready_frame: Option<u32>,
    mode: RenderProofMode,
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
    match proof.mode {
        RenderProofMode::Offline => {
            presentation.set_proof_pose();
            camera.set_proof_view();
        }
        RenderProofMode::LiveEntry => {
            assert!(
                session.is_live_entry(),
                "live proof requires a live entry session"
            );
            session
                .start_entry()
                .expect("live proof session should accept the complete entry operation");
        }
    }
    proof.scripted = true;
}

fn capture_render_proof(
    mut commands: Commands,
    mut proof: ResMut<RenderProofState>,
    presentation: Res<DiagnosticPresentation>,
    view: Res<DiagnosticView>,
    mut exit: MessageWriter<AppExit>,
) {
    proof.frame += 1;
    if proof.mode == RenderProofMode::LiveEntry && proof.ready_frame.is_none() {
        match view.snapshot().phase {
            client_session::ClientPhase::MovementReady => {
                proof.ready_frame = Some(proof.frame);
            }
            client_session::ClientPhase::Failed(_) => {
                panic!("live proof session failed before MovementReady")
            }
            _ if proof.frame > TIMEOUT_FRAME => {
                panic!("timed out while waiting for MovementReady")
            }
            _ => return,
        }
    }
    let capture_frame = match proof.mode {
        RenderProofMode::Offline => CAPTURE_FRAME,
        RenderProofMode::LiveEntry => proof
            .ready_frame
            .expect("live proof is armed only after MovementReady")
            .saturating_add(CAPTURE_FRAME),
    };
    if proof.requested && proof.output.exists() {
        let sidecar = proof_sidecar(&view, &presentation);
        fs::write(proof.output.with_extension("json"), sidecar)
            .expect("proof sidecar should be writable");
        info!("rendered proof saved to {}", proof.output.display());
        exit.write(AppExit::Success);
    } else if proof.frame == capture_frame {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(proof.output.clone()));
        proof.requested = true;
    } else if proof.frame > capture_frame.saturating_add(TIMEOUT_FRAME) {
        panic!("timed out while waiting for the rendered proof artifact");
    }
}

fn proof_sidecar(view: &DiagnosticView, presentation: &DiagnosticPresentation) -> String {
    if view.is_live_entry() {
        return live_proof_sidecar(view, presentation);
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

fn live_proof_sidecar(view: &DiagnosticView, presentation: &DiagnosticPresentation) -> String {
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
    format!(
        concat!(
            "{{\n",
            "  \"schema\": \"miazcore.live-render-proof.v1\",\n",
            "  \"phase\": \"MovementReady\",\n",
            "  \"network\": \"reference-realm\",\n",
            "  \"realm_id\": {},\n",
            "  \"client_build\": {},\n",
            "  \"character\": {},\n",
            "  \"run_speed\": {:.3},\n",
            "  \"movement_publication\": \"disabled\",\n",
            "  \"entry_anchor\": {},\n",
            "  \"rendered_pose\": {},\n",
            "  \"submitted_pose\": {},\n",
            "  \"realm_observed_pose\": {}\n",
            "}}\n"
        ),
        snapshot.identity.realm_id(),
        snapshot.identity.client_build(),
        json_string(snapshot.identity.character_name()),
        snapshot
            .run_speed
            .expect("MovementReady has a positive run speed"),
        pose_json(anchor),
        pose_json(rendered),
        pose_json(submitted),
        pose_json(observed),
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

    use super::{json_string, proof_sidecar};

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
    fn live_sidecar_keeps_entry_pose_truths_distinct_and_disables_movement() {
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
        );

        assert!(sidecar.contains("\"phase\": \"MovementReady\""));
        assert!(sidecar.contains("\"movement_publication\": \"disabled\""));
        assert!(sidecar.contains("\"submitted_pose\": { \"map_id\": 0"));
        assert!(sidecar.contains("\"entry_anchor\": { \"map_id\": 0"));
        assert!(!sidecar.to_ascii_lowercase().contains("password"));
        assert!(!sidecar.to_ascii_lowercase().contains("session_key"));
    }
}
