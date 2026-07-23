use std::f32::consts::TAU;

use bevy::prelude::*;

use crate::{ClientScheduleSet, DiagnosticView, camera::spawn_camera};

const CYAN: Color = Color::srgb(0.41, 0.85, 0.86);
const AMBER: Color = Color::srgb(0.94, 0.74, 0.41);
const PARKED_MARKER_HEIGHT: f32 = -1_000.0;
const CORRECTION_SNAP_DISTANCE_METRES: f32 = 5.0;
const RENDER_RECONCILIATION_METRES_PER_SECOND: f32 = 8.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Resource)]
pub struct DiagnosticPresentation {
    pub rendered_planar: Vec2,
    pub heading: f32,
    pub(crate) entry_anchor: Option<client_session::WorldPose>,
    pub(crate) rendered_pose: Option<client_session::WorldPose>,
}

pub(crate) fn offline_planar_to_scene(planar: Vec2, height: f32) -> Vec3 {
    Vec3::new(planar.x, height, -planar.y)
}

fn world_pose_to_scene(
    anchor: client_session::WorldPose,
    pose: client_session::WorldPose,
    height: f32,
) -> Option<Vec3> {
    (anchor.map_id == pose.map_id).then(|| {
        offline_planar_to_scene(
            Vec2::new(pose.east - anchor.east, pose.north - anchor.north),
            height,
        )
    })
}

fn parked_marker_translation() -> Vec3 {
    Vec3::new(0.0, PARKED_MARKER_HEIGHT, 0.0)
}

fn rendered_avatar_translation(presentation: &DiagnosticPresentation, live_entry: bool) -> Vec3 {
    if live_entry && presentation.entry_anchor.is_none() {
        parked_marker_translation()
    } else {
        offline_planar_to_scene(presentation.rendered_planar, 0.74)
    }
}

impl DiagnosticPresentation {
    pub fn set_proof_pose(&mut self) {
        self.rendered_planar = Vec2::new(2.4, -1.6);
        self.heading = 2.16;
        self.entry_anchor = None;
        self.rendered_pose = None;
    }

    pub(crate) fn rendered_pose(&self) -> Option<client_session::WorldPose> {
        self.rendered_pose
    }

    fn project_authoritative_entry(
        &mut self,
        snapshot: &client_session::ClientSnapshot,
        delta_seconds: f32,
    ) {
        let Some(anchor) = snapshot.entry_anchor else {
            return;
        };
        if self.entry_anchor != Some(anchor) {
            self.entry_anchor = Some(anchor);
            self.rendered_pose = Some(anchor);
            self.rendered_planar = Vec2::ZERO;
            self.heading = anchor.orientation;
        }
        let target = snapshot
            .correction_target
            .map(client_session::CorrectionTarget::pose)
            .or(snapshot.predicted_pose)
            .unwrap_or(anchor);
        let current = self.rendered_pose.unwrap_or(anchor);
        if target.map_id != anchor.map_id {
            return;
        }
        let correction_requires_snap = snapshot.correction_target.is_some()
            && (current.map_id != target.map_id
                || planar_distance(current, target) >= CORRECTION_SNAP_DISTANCE_METRES);
        if correction_requires_snap {
            self.apply_rendered_pose(anchor, target);
            return;
        }
        let distance = planar_distance(current, target);
        let step = (RENDER_RECONCILIATION_METRES_PER_SECOND * delta_seconds).max(0.0);
        let blend = if distance <= f32::EPSILON {
            1.0
        } else {
            (step / distance).min(1.0)
        };
        let rendered = client_session::WorldPose {
            map_id: target.map_id,
            east: current.east + (target.east - current.east) * blend,
            north: current.north + (target.north - current.north) * blend,
            elevation: anchor.elevation,
            orientation: current.orientation + (target.orientation - current.orientation) * blend,
        };
        self.apply_rendered_pose(anchor, rendered);
    }

    fn apply_rendered_pose(
        &mut self,
        anchor: client_session::WorldPose,
        rendered: client_session::WorldPose,
    ) {
        self.rendered_pose = Some(rendered);
        self.rendered_planar =
            Vec2::new(rendered.east - anchor.east, rendered.north - anchor.north);
        self.heading = rendered.orientation;
    }
}

fn planar_distance(left: client_session::WorldPose, right: client_session::WorldPose) -> f32 {
    (left.east - right.east).hypot(left.north - right.north)
}

#[derive(Component)]
struct RenderedAvatar;

#[derive(Component)]
struct SubmittedMarker;

#[derive(Component)]
struct RealmObservedMarker;

#[derive(Component)]
struct EntryAnchorMarker;

pub(crate) struct DiagnosticWorldPlugin;

impl Plugin for DiagnosticWorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DiagnosticPresentation>()
            .insert_resource(ClearColor(Color::srgb(0.018, 0.028, 0.024)))
            .insert_resource(GlobalAmbientLight {
                color: Color::srgb(0.32, 0.42, 0.36),
                brightness: 260.0,
                affects_lightmapped_meshes: true,
            })
            .add_systems(Startup, setup_diagnostic_world)
            .add_systems(
                Update,
                (project_authoritative_entry, project_pose_markers)
                    .chain()
                    .in_set(ClientScheduleSet::Presentation),
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
            Name::new("Entry Anchor marker"),
            EntryAnchorMarker,
            Mesh3d(cross_mesh.clone()),
            MeshMaterial3d(anchor_material.clone()),
            Transform::from_translation(parked_marker_translation()).with_scale(scale),
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

fn project_authoritative_entry(
    view: Res<DiagnosticView>,
    mut presentation: ResMut<DiagnosticPresentation>,
    time: Res<Time>,
) {
    if view.is_live_entry() {
        presentation.project_authoritative_entry(view.snapshot(), time.delta_secs());
    }
}

#[allow(clippy::type_complexity)]
fn project_pose_markers(
    presentation: Res<DiagnosticPresentation>,
    view: Res<DiagnosticView>,
    mut avatar: Single<
        &mut Transform,
        (
            With<RenderedAvatar>,
            Without<SubmittedMarker>,
            Without<RealmObservedMarker>,
            Without<EntryAnchorMarker>,
        ),
    >,
    mut submitted: Single<
        &mut Transform,
        (
            With<SubmittedMarker>,
            Without<RenderedAvatar>,
            Without<RealmObservedMarker>,
            Without<EntryAnchorMarker>,
        ),
    >,
    mut observed: Single<
        &mut Transform,
        (
            With<RealmObservedMarker>,
            Without<RenderedAvatar>,
            Without<SubmittedMarker>,
            Without<EntryAnchorMarker>,
        ),
    >,
    mut entry_anchor: Query<&mut Transform, With<EntryAnchorMarker>>,
) {
    avatar.translation = rendered_avatar_translation(&presentation, view.is_live_entry());
    if !(view.is_live_entry() && presentation.entry_anchor.is_none()) {
        avatar.rotation = Quat::from_rotation_y(presentation.heading);
    }

    let anchor = presentation.entry_anchor;
    if let (Some(anchor), Some(submitted_pose)) = (anchor, view.snapshot().submitted_pose) {
        submitted.translation = world_pose_to_scene(anchor, submitted_pose, 0.08)
            .unwrap_or_else(parked_marker_translation);
    } else {
        submitted.translation = parked_marker_translation();
    }

    if let (Some(anchor), Some(observed_pose)) = (anchor, view.snapshot().realm_observed_pose) {
        observed.translation = world_pose_to_scene(anchor, observed_pose, 0.34)
            .unwrap_or_else(parked_marker_translation);
    } else {
        observed.translation = parked_marker_translation();
    }

    let translation = if anchor.is_some() {
        Vec3::new(0.0, 0.055, 0.0)
    } else {
        parked_marker_translation()
    };
    for mut marker in &mut entry_anchor {
        marker.translation = translation;
    }
}

#[cfg(test)]
mod tests {
    use bevy::prelude::Vec3;
    use client_session::{ClientSnapshot, CorrectionTarget, SanitizedIdentity, WorldPose};

    use super::{
        DiagnosticPresentation, PARKED_MARKER_HEIGHT, rendered_avatar_translation,
        world_pose_to_scene,
    };

    #[test]
    fn entry_anchor_is_the_local_diagnostic_world_origin() {
        let anchor = WorldPose {
            map_id: 0,
            east: -8949.95,
            north: -132.493,
            elevation: 83.5312,
            orientation: 0.0,
        };
        assert_eq!(
            world_pose_to_scene(anchor, anchor, 0.34),
            Some(Vec3::new(0.0, 0.34, 0.0))
        );

        let other_map = WorldPose {
            map_id: 1,
            ..anchor
        };
        assert_eq!(world_pose_to_scene(anchor, other_map, 0.34), None);
    }

    #[test]
    fn live_avatar_is_parked_until_an_authoritative_entry_anchor_exists() {
        let presentation = DiagnosticPresentation::default();
        assert_eq!(
            rendered_avatar_translation(&presentation, true),
            Vec3::new(0.0, PARKED_MARKER_HEIGHT, 0.0)
        );
        assert_eq!(
            rendered_avatar_translation(&presentation, false),
            Vec3::new(0.0, 0.74, 0.0)
        );
    }

    #[test]
    fn scripted_correction_smooths_below_five_metres_and_snaps_at_the_boundary() {
        let identity = SanitizedIdentity::new(1, "Realm", "Character", 12_340).unwrap();
        let anchor = WorldPose {
            map_id: 0,
            east: 0.0,
            north: 0.0,
            elevation: 0.0,
            orientation: 0.0,
        };
        let mut snapshot = ClientSnapshot::offline(identity);
        snapshot.entry_anchor = Some(anchor);
        snapshot.correction_target = Some(CorrectionTarget::scripted(WorldPose {
            east: 4.0,
            ..anchor
        }));
        let mut presentation = DiagnosticPresentation::default();
        presentation.project_authoritative_entry(&snapshot, 0.25);
        assert_eq!(
            presentation.rendered_pose.unwrap().east.to_bits(),
            2.0_f32.to_bits()
        );

        snapshot.correction_target = Some(CorrectionTarget::scripted(WorldPose {
            east: 7.0,
            ..anchor
        }));
        presentation.project_authoritative_entry(&snapshot, 0.01);
        assert_eq!(
            presentation.rendered_pose.unwrap().east.to_bits(),
            7.0_f32.to_bits()
        );
        assert_eq!(snapshot.realm_observed_pose, None);
    }
}
