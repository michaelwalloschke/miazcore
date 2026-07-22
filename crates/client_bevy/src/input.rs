use bevy::{prelude::*, window::PrimaryWindow};

use crate::{
    ClientScheduleSet, DiagnosticView, camera::CameraRig, input_axis, world::OfflinePresentation,
};

const OFFLINE_DISPLAY_SPEED: f32 = 3.5;
const DISPLAY_ENVELOPE_RADIUS: f32 = 5.0;

pub(crate) struct OfflineInputPlugin;

impl Plugin for OfflineInputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            collect_offline_presentation_input.in_set(ClientScheduleSet::Input),
        );
    }
}

fn collect_offline_presentation_input(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Res<CameraRig>,
    view: Res<DiagnosticView>,
    mut presentation: ResMut<OfflinePresentation>,
) {
    if !window.focused || view.snapshot().phase != client_session::ClientPhase::Offline {
        return;
    }
    let right = input_axis(&keys, KeyCode::KeyD, KeyCode::KeyA);
    let forward = input_axis(&keys, KeyCode::KeyW, KeyCode::KeyS);
    let local = Vec2::new(right, forward).normalize_or_zero();
    if local == Vec2::ZERO {
        return;
    }
    advance_offline_presentation(
        &mut presentation,
        local,
        camera.yaw,
        OFFLINE_DISPLAY_SPEED * time.delta_secs(),
    );
}

pub(crate) fn advance_offline_presentation(
    presentation: &mut OfflinePresentation,
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

    use super::{DISPLAY_ENVELOPE_RADIUS, advance_offline_presentation};
    use crate::world::OfflinePresentation;

    #[test]
    fn offline_motion_is_camera_relative_and_stays_inside_display_envelope() {
        let mut presentation = OfflinePresentation::default();

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
        let mut presentation = OfflinePresentation::default();
        advance_offline_presentation(&mut presentation, Vec2::ZERO, 0.0, 3.0);
        advance_offline_presentation(&mut presentation, Vec2::Y, 0.0, f32::NAN);
        advance_offline_presentation(&mut presentation, Vec2::Y, 0.0, -1.0);
        assert_eq!(presentation, OfflinePresentation::default());
    }
}
