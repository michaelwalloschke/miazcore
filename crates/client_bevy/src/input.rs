use bevy::{prelude::*, window::PrimaryWindow};

use crate::{
    ClientScheduleSet, DiagnosticView, SessionBridge, camera::CameraRig, input_axis,
    world::DiagnosticPresentation,
};

const OFFLINE_DISPLAY_SPEED: f32 = 3.5;
const DISPLAY_ENVELOPE_RADIUS: f32 = 5.0;

pub(crate) struct OfflineInputPlugin;

impl Plugin for OfflineInputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            collect_presentation_input.in_set(ClientScheduleSet::Input),
        );
    }
}

fn collect_presentation_input(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Res<CameraRig>,
    view: Res<DiagnosticView>,
    session: Res<SessionBridge>,
    mut presentation: ResMut<DiagnosticPresentation>,
) {
    let right = input_axis(&keys, KeyCode::KeyD, KeyCode::KeyA);
    let forward = input_axis(&keys, KeyCode::KeyW, KeyCode::KeyS);
    let local = Vec2::new(right, forward).normalize_or_zero();
    if view.is_live_entry() {
        let intent = if window.focused
            && view.snapshot().phase == client_session::ClientPhase::MovementReady
        {
            camera_relative_intent(local, camera.yaw)
        } else {
            client_session::MovementIntent::idle()
        };
        // A lossless edge is queued only when moving toggles; steady input
        // remains a replaceable mailbox value.
        let _ = session.publish_movement_intent(intent);
        return;
    }
    if !window.focused
        || view.snapshot().phase != client_session::ClientPhase::Offline
        || local == Vec2::ZERO
    {
        return;
    }
    advance_offline_presentation(
        &mut presentation,
        local,
        camera.yaw,
        OFFLINE_DISPLAY_SPEED * time.delta_secs(),
    );
}

fn camera_relative_intent(local: Vec2, camera_yaw: f32) -> client_session::MovementIntent {
    let direction = Vec2::new(
        local.x * camera_yaw.cos() - local.y * camera_yaw.sin(),
        local.x * camera_yaw.sin() + local.y * camera_yaw.cos(),
    )
    .normalize_or_zero();
    client_session::MovementIntent::planar(direction.x, direction.y)
        .expect("Bevy input directions are finite")
}

pub(crate) fn advance_offline_presentation(
    presentation: &mut DiagnosticPresentation,
    local_input: Vec2,
    camera_yaw: f32,
    distance: f32,
) {
    let direction = Vec2::new(
        local_input.x * camera_yaw.cos() - local_input.y * camera_yaw.sin(),
        local_input.x * camera_yaw.sin() + local_input.y * camera_yaw.cos(),
    )
    .normalize_or_zero();
    if direction == Vec2::ZERO || !distance.is_finite() || distance <= 0.0 {
        return;
    }
    let candidate = presentation.rendered_planar + direction * distance;
    presentation.rendered_planar = candidate.clamp_length_max(DISPLAY_ENVELOPE_RADIUS);
    presentation.heading = direction.x.atan2(direction.y);
}

#[cfg(test)]
mod tests {
    use bevy::prelude::Vec2;

    use super::{DISPLAY_ENVELOPE_RADIUS, advance_offline_presentation, camera_relative_intent};
    use crate::world::DiagnosticPresentation;

    #[test]
    fn offline_motion_is_camera_relative_and_stays_inside_display_envelope() {
        let mut presentation = DiagnosticPresentation::default();

        advance_offline_presentation(&mut presentation, Vec2::Y, std::f32::consts::FRAC_PI_2, 2.0);
        assert!((presentation.rendered_planar.x + 2.0).abs() < 0.000_1);
        assert!(presentation.rendered_planar.y.abs() < 0.000_1);

        for _ in 0..10 {
            advance_offline_presentation(&mut presentation, Vec2::Y, 0.0, 2.0);
        }
        assert!(presentation.rendered_planar.length() <= DISPLAY_ENVELOPE_RADIUS);
    }

    #[test]
    fn invalid_or_idle_steps_do_not_change_presentation() {
        let mut presentation = DiagnosticPresentation::default();
        advance_offline_presentation(&mut presentation, Vec2::ZERO, 0.0, 3.0);
        advance_offline_presentation(&mut presentation, Vec2::Y, 0.0, f32::NAN);
        advance_offline_presentation(&mut presentation, Vec2::Y, 0.0, -1.0);
        assert_eq!(presentation, DiagnosticPresentation::default());
    }

    #[test]
    fn live_intent_is_camera_relative_and_normalized() {
        let intent = camera_relative_intent(Vec2::Y, std::f32::consts::FRAC_PI_2);
        assert!(intent.engaged());
        assert!((intent.east() + 1.0).abs() < 0.000_1);
        assert!(intent.north().abs() < 0.000_1);
    }
}
