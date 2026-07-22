use std::{fmt::Write as _, fs, path::PathBuf};

use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
};

use crate::{ClientScheduleSet, DiagnosticView, camera::CameraRig, world::OfflinePresentation};

// Leave enough presented frames for Metal pipeline creation on a cold shader cache.
const CAPTURE_FRAME: u32 = 180;
const TIMEOUT_FRAME: u32 = 900;

pub struct RenderProofPlugin {
    output: PathBuf,
}

impl RenderProofPlugin {
    #[must_use]
    pub fn new(output: impl Into<PathBuf>) -> Self {
        Self {
            output: output.into(),
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
}

fn script_render_proof(
    mut proof: ResMut<RenderProofState>,
    mut presentation: ResMut<OfflinePresentation>,
    mut camera: ResMut<CameraRig>,
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
    presentation.set_proof_pose();
    camera.set_proof_view();
    proof.scripted = true;
}

fn capture_render_proof(
    mut commands: Commands,
    mut proof: ResMut<RenderProofState>,
    presentation: Res<OfflinePresentation>,
    view: Res<DiagnosticView>,
    mut exit: MessageWriter<AppExit>,
) {
    proof.frame += 1;
    if proof.requested && proof.output.exists() {
        let sidecar = proof_sidecar(&view, &presentation);
        fs::write(proof.output.with_extension("json"), sidecar)
            .expect("proof sidecar should be writable");
        info!("rendered proof saved to {}", proof.output.display());
        exit.write(AppExit::Success);
    } else if proof.frame == CAPTURE_FRAME {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(proof.output.clone()));
        proof.requested = true;
    } else if proof.frame > TIMEOUT_FRAME {
        panic!("timed out while waiting for the rendered proof artifact");
    }
}

fn proof_sidecar(view: &DiagnosticView, presentation: &OfflinePresentation) -> String {
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

    use client_session::{ClientSnapshot, SanitizedIdentity};

    use crate::{DiagnosticView, OfflinePresentation};

    use super::{json_string, proof_sidecar};

    #[test]
    fn sidecar_is_semantic_offline_evidence_without_secret_fields() {
        let identity =
            SanitizedIdentity::new(1, "Miazcore Reference Realm", "Miaztest", 12_340).unwrap();
        let view = DiagnosticView {
            snapshot: ClientSnapshot::offline(identity),
            recent_events: VecDeque::default(),
        };
        let sidecar = proof_sidecar(
            &view,
            &OfflinePresentation {
                rendered_planar: bevy::prelude::Vec2::new(2.4, -1.6),
                heading: 2.16,
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
}
