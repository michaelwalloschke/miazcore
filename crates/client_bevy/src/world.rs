use std::f32::consts::TAU;

use bevy::prelude::*;

use crate::{ClientScheduleSet, DiagnosticView, camera::spawn_camera};

const CYAN: Color = Color::srgb(0.41, 0.85, 0.86);
const AMBER: Color = Color::srgb(0.94, 0.74, 0.41);
const PARKED_MARKER_HEIGHT: f32 = -1_000.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Resource)]
pub struct OfflinePresentation {
    pub rendered_planar: Vec2,
    pub heading: f32,
}

pub(crate) fn offline_planar_to_scene(planar: Vec2, height: f32) -> Vec3 {
    Vec3::new(planar.x, height, -planar.y)
}

fn world_pose_to_scene(pose: client_session::WorldPose, height: f32) -> Vec3 {
    offline_planar_to_scene(Vec2::new(pose.east, pose.north), height)
}

fn parked_marker_translation() -> Vec3 {
    Vec3::new(0.0, PARKED_MARKER_HEIGHT, 0.0)
}

impl OfflinePresentation {
    pub fn set_proof_pose(&mut self) {
        self.rendered_planar = Vec2::new(2.4, -1.6);
        self.heading = 2.16;
    }
}

#[derive(Component)]
struct RenderedAvatar;

#[derive(Component)]
struct SubmittedMarker;

#[derive(Component)]
struct RealmObservedMarker;

pub(crate) struct DiagnosticWorldPlugin;

impl Plugin for DiagnosticWorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OfflinePresentation>()
            .insert_resource(ClearColor(Color::srgb(0.018, 0.028, 0.024)))
            .insert_resource(GlobalAmbientLight {
                color: Color::srgb(0.32, 0.42, 0.36),
                brightness: 260.0,
                affects_lightmapped_meshes: true,
            })
            .add_systems(Startup, setup_diagnostic_world)
            .add_systems(
                Update,
                project_pose_markers.in_set(ClientScheduleSet::Presentation),
            );
    }
}

#[allow(clippy::too_many_lines)]
fn setup_diagnostic_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let ground_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.035, 0.065, 0.052),
        perceptual_roughness: 0.96,
        metallic: 0.02,
        ..default()
    });
    commands.spawn((
        Name::new("Project-owned Diagnostic ground"),
        Mesh3d(meshes.add(Plane3d::default().mesh().size(32.0, 32.0))),
        MeshMaterial3d(ground_material),
    ));

    let grid_minor_material = materials.add(Color::srgb(0.075, 0.15, 0.12));
    let grid_major_material = materials.add(Color::srgb(0.19, 0.48, 0.38));
    let grid_mesh_x = meshes.add(Cuboid::new(0.018, 0.012, 24.0));
    let grid_mesh_z = meshes.add(Cuboid::new(24.0, 0.012, 0.018));
    for coordinate in -12_i16..=12_i16 {
        let material = if coordinate % 5 == 0 {
            grid_major_material.clone()
        } else {
            grid_minor_material.clone()
        };
        commands.spawn((
            Mesh3d(grid_mesh_x.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(f32::from(coordinate), 0.008, 0.0),
        ));
        commands.spawn((
            Mesh3d(grid_mesh_z.clone()),
            MeshMaterial3d(material),
            Transform::from_xyz(0.0, 0.008, f32::from(coordinate)),
        ));
    }

    let anchor_material = materials.add(StandardMaterial {
        base_color: CYAN,
        emissive: LinearRgba::new(0.05, 0.48, 0.46, 1.0),
        ..default()
    });
    let cross_mesh = meshes.add(Cuboid::default());
    for scale in [Vec3::new(1.35, 0.06, 0.08), Vec3::new(0.08, 0.06, 1.35)] {
        commands.spawn((
            Name::new("Offline display origin"),
            Mesh3d(cross_mesh.clone()),
            MeshMaterial3d(anchor_material.clone()),
            Transform::from_xyz(0.0, 0.055, 0.0).with_scale(scale),
        ));
    }

    let envelope_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.41, 0.85, 0.86, 0.65),
        emissive: LinearRgba::new(0.03, 0.25, 0.27, 1.0),
        ..default()
    });
    let envelope_segment = meshes.add(Cuboid::new(0.72, 0.025, 0.035));
    for index in 0_u16..48 {
        let angle = f32::from(index) / 48.0 * TAU;
        commands.spawn((
            Name::new("Five-metre offline display guide"),
            Mesh3d(envelope_segment.clone()),
            MeshMaterial3d(envelope_material.clone()),
            Transform::from_xyz(angle.cos() * 5.0, 0.035, angle.sin() * 5.0)
                .with_rotation(Quat::from_rotation_y(-angle)),
        ));
    }

    commands.spawn((
        Name::new("Rendered Pose placeholder"),
        RenderedAvatar,
        Mesh3d(meshes.add(Capsule3d::new(0.38, 0.72))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: CYAN,
            emissive: LinearRgba::new(0.02, 0.23, 0.24, 1.0),
            perceptual_roughness: 0.38,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.74, 0.0),
    ));

    commands.spawn((
        Name::new("Submitted Pose marker"),
        SubmittedMarker,
        Mesh3d(meshes.add(Torus::new(0.46, 0.52))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: CYAN,
            emissive: LinearRgba::new(0.02, 0.30, 0.31, 1.0),
            ..default()
        })),
        Transform::from_translation(parked_marker_translation())
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
    ));

    commands.spawn((
        Name::new("Realm-observed Pose marker"),
        RealmObservedMarker,
        Mesh3d(meshes.add(Sphere::new(0.27).mesh().ico(2).expect("valid ico sphere"))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: AMBER,
            emissive: LinearRgba::new(0.28, 0.15, 0.03, 1.0),
            perceptual_roughness: 0.5,
            ..default()
        })),
        Transform::from_translation(parked_marker_translation()),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 8_500.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.82, -0.58, 0.0)),
    ));
    spawn_camera(&mut commands);
}

#[allow(clippy::type_complexity)]
fn project_pose_markers(
    presentation: Res<OfflinePresentation>,
    view: Res<DiagnosticView>,
    mut avatar: Single<
        &mut Transform,
        (
            With<RenderedAvatar>,
            Without<SubmittedMarker>,
            Without<RealmObservedMarker>,
        ),
    >,
    mut submitted: Single<
        &mut Transform,
        (
            With<SubmittedMarker>,
            Without<RenderedAvatar>,
            Without<RealmObservedMarker>,
        ),
    >,
    mut observed: Single<
        &mut Transform,
        (
            With<RealmObservedMarker>,
            Without<RenderedAvatar>,
            Without<SubmittedMarker>,
        ),
    >,
) {
    avatar.translation = offline_planar_to_scene(presentation.rendered_planar, 0.74);
    avatar.rotation = Quat::from_rotation_y(presentation.heading);

    if let Some(submitted_pose) = view.snapshot().submitted_pose {
        submitted.translation = world_pose_to_scene(submitted_pose, 0.08);
    } else {
        submitted.translation = parked_marker_translation();
    }

    if let Some(observed_pose) = view.snapshot().realm_observed_pose {
        observed.translation = world_pose_to_scene(observed_pose, 0.34);
    } else {
        observed.translation = parked_marker_translation();
    }
}
