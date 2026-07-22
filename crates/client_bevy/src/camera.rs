use std::f32::consts::FRAC_PI_2;

use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
    window::PrimaryWindow,
};

use crate::{
    ClientScheduleSet, input_axis,
    world::{DiagnosticPresentation, offline_planar_to_scene},
};

#[derive(Debug, Resource)]
pub(crate) struct CameraRig {
    pub(crate) yaw: f32,
    pitch: f32,
    distance: f32,
}

impl Default for CameraRig {
    fn default() -> Self {
        Self {
            yaw: -0.72,
            pitch: -0.42,
            distance: 11.0,
        }
    }
}

impl CameraRig {
    pub(crate) fn set_proof_view(&mut self) {
        self.yaw = -0.96;
        self.pitch = -0.48;
        self.distance = 10.5;
    }
}

#[derive(Component)]
struct ChaseCamera;

pub(crate) struct ChaseCameraPlugin;

impl Plugin for ChaseCameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraRig>().add_systems(
            Update,
            update_chase_camera.in_set(ClientScheduleSet::Camera),
        );
    }
}

pub(crate) fn spawn_camera(commands: &mut Commands) {
    commands.spawn((
        Name::new("Diagnostic chase-orbit camera"),
        ChaseCamera,
        Camera3d::default(),
        Transform::from_xyz(7.0, 5.0, 8.0).looking_at(Vec3::Y, Vec3::Y),
    ));
}

#[allow(clippy::too_many_arguments)]
fn update_chase_camera(
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    time: Res<Time>,
    window: Single<&Window, With<PrimaryWindow>>,
    presentation: Res<DiagnosticPresentation>,
    mut rig: ResMut<CameraRig>,
    mut camera: Single<&mut Transform, With<ChaseCamera>>,
) {
    if window.focused {
        let seconds = time.delta_secs();
        rig.yaw += input_axis(&keys, KeyCode::ArrowLeft, KeyCode::ArrowRight) * seconds * 1.4;
        rig.pitch += input_axis(&keys, KeyCode::ArrowDown, KeyCode::ArrowUp) * seconds;
        rig.distance += input_axis(&keys, KeyCode::KeyE, KeyCode::KeyQ) * seconds * 5.0;
        rig.distance -= mouse_scroll.delta.y * 0.8;
        if mouse_buttons.pressed(MouseButton::Right) {
            rig.yaw -= mouse_motion.delta.x * 0.004;
            rig.pitch -= mouse_motion.delta.y * 0.004;
        }
    }

    rig.pitch = rig.pitch.clamp(-FRAC_PI_2 + 0.12, -0.08);
    rig.distance = rig.distance.clamp(4.0, 18.0);
    let focus = offline_planar_to_scene(presentation.rendered_planar, 0.70);
    let orbit = Quat::from_euler(EulerRot::YXZ, rig.yaw, rig.pitch, 0.0);
    camera.translation = focus + orbit * Vec3::Z * rig.distance;
    camera.look_at(focus, Vec3::Y);
}
