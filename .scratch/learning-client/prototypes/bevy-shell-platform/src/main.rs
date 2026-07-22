// Bevy system parameters are intentionally passed by value so the engine can
// construct them from the World; references are not valid SystemParam types.
#![allow(clippy::needless_pass_by_value)]

use std::{f32::consts::FRAC_PI_2, path::PathBuf};

use bevy::{
    app::AppExit,
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::WindowPlugin,
};
use bevy_shell_platform_proof::{
    adapter::{ClientAdapterSet, ClientEventQueue, ClientModel, install_client_adapter},
    model::{ClientEvent, Position},
};

const MOVE_SPEED: f32 = 3.5;
const PROOF_CAPTURE_FRAME: u32 = 45;
const PROOF_TIMEOUT_FRAME: u32 = 600;

#[derive(Component)]
struct Avatar;

#[derive(Component)]
struct StatusText;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
enum ShellSet {
    Input,
    Presentation,
}

#[derive(Debug, Resource)]
struct CameraRig {
    yaw: f32,
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

#[derive(Debug, Resource)]
struct ProofRun {
    output: Option<PathBuf>,
    frame: u32,
    requested: bool,
}

fn main() {
    let output = proof_output_from_args();
    prepare_proof_output(output.as_ref());

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "MiazCore — Bevy shell platform proof".into(),
            resolution: (1280, 720).into(),
            ..default()
        }),
        ..default()
    }))
    .insert_resource(ClearColor(Color::srgb(0.018, 0.025, 0.045)))
    .insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.28, 0.34, 0.50),
        brightness: 300.0,
        affects_lightmapped_meshes: true,
    })
    .init_resource::<CameraRig>()
    .insert_resource(ProofRun {
        output,
        frame: 0,
        requested: false,
    })
    .configure_sets(
        Update,
        (
            ShellSet::Input,
            ClientAdapterSet::ApplyEvents,
            ShellSet::Presentation,
        )
            .chain(),
    )
    .add_systems(Startup, setup_scene)
    .add_systems(
        Update,
        (enqueue_movement, script_proof, update_camera)
            .chain()
            .in_set(ShellSet::Input),
    )
    .add_systems(
        Update,
        (sync_avatar_and_ui, capture_proof).in_set(ShellSet::Presentation),
    );
    install_client_adapter(&mut app);
    app.run();
}

fn proof_output_from_args() -> Option<PathBuf> {
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--proof-output" {
            return args.next().map(PathBuf::from);
        }
    }
    None
}

fn prepare_proof_output(output: Option<&PathBuf>) {
    let Some(output) = output else {
        return;
    };
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).expect("proof artifact directory should be creatable");
    }
    if output.exists() {
        std::fs::remove_file(output).expect("old proof artifact should be replaceable");
    }
}

// Keeping this disposable scene in one place makes its deletion boundary
// obvious; it is data-heavy rather than control-flow-heavy.
#[allow(clippy::too_many_lines)]
fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut events: ResMut<ClientEventQueue>,
) {
    let ground = materials.add(StandardMaterial {
        base_color: Color::srgb(0.045, 0.075, 0.12),
        perceptual_roughness: 0.94,
        metallic: 0.04,
        ..default()
    });
    let grid_minor = materials.add(Color::srgb(0.08, 0.16, 0.23));
    let grid_major = materials.add(Color::srgb(0.08, 0.42, 0.52));

    commands.spawn((
        Name::new("Diagnostic ground"),
        Mesh3d(meshes.add(Plane3d::default().mesh().size(30.0, 30.0))),
        MeshMaterial3d(ground),
    ));

    for coordinate in -10_i16..=10_i16 {
        let material = if coordinate % 5 == 0 {
            grid_major.clone()
        } else {
            grid_minor.clone()
        };
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.025, 0.012, 20.0))),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(f32::from(coordinate), 0.008, 0.0),
        ));
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(20.0, 0.012, 0.025))),
            MeshMaterial3d(material),
            Transform::from_xyz(0.0, 0.008, f32::from(coordinate)),
        ));
    }

    let anchor_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.1, 0.75, 0.92),
        emissive: LinearRgba::new(0.04, 0.50, 0.70, 1.0),
        ..default()
    });
    for (position, scale) in [
        (Vec3::new(0.0, 0.06, 0.0), Vec3::new(1.4, 0.08, 0.10)),
        (Vec3::new(0.0, 0.06, 0.0), Vec3::new(0.10, 0.08, 1.4)),
    ] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::default())),
            MeshMaterial3d(anchor_material.clone()),
            Transform::from_translation(position).with_scale(scale),
        ));
    }

    commands.spawn((
        Name::new("Placeholder character"),
        Avatar,
        Mesh3d(meshes.add(Capsule3d::new(0.38, 0.72))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.95, 0.55, 0.18),
            perceptual_roughness: 0.42,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.74, 0.0),
    ));

    for (name, position, color) in [
        (
            "Entry Anchor",
            Vec3::new(-3.0, 0.35, -2.0),
            Color::srgb(0.16, 0.72, 0.92),
        ),
        (
            "Movement Envelope",
            Vec3::new(3.2, 0.55, 2.3),
            Color::srgb(0.66, 0.25, 0.92),
        ),
        (
            "Realm Marker",
            Vec3::new(-2.0, 0.8, 3.4),
            Color::srgb(0.22, 0.88, 0.54),
        ),
    ] {
        commands.spawn((
            Name::new(name),
            Mesh3d(meshes.add(Cuboid::new(0.65, 0.65, 0.65))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color,
                perceptual_roughness: 0.56,
                ..default()
            })),
            Transform::from_translation(position),
        ));
    }

    commands.spawn((
        DirectionalLight {
            illuminance: 8_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, -0.65, 0.0)),
    ));

    commands.spawn((
        Name::new("Chase orbit camera"),
        Camera3d::default(),
        Transform::from_xyz(7.0, 5.0, 8.0).looking_at(Vec3::Y, Vec3::Y),
    ));

    commands.spawn((
        StatusText,
        Text::new("WORLD READY"),
        TextFont {
            font_size: FontSize::Px(18.0),
            ..default()
        },
        TextColor(Color::srgb(0.84, 0.94, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: px(18),
            left: px(18),
            padding: UiRect::all(px(14)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.015, 0.025, 0.055, 0.90)),
    ));

    commands.spawn((
        Text::new("WASD  MOVE    ARROWS / DRAG  ORBIT    Q / E  ZOOM"),
        TextFont {
            font_size: FontSize::Px(14.0),
            ..default()
        },
        TextColor(Color::srgb(0.55, 0.72, 0.80)),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(18),
            left: px(18),
            padding: UiRect::all(px(10)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.015, 0.025, 0.055, 0.84)),
    ));

    events.push(ClientEvent::EnterWorld {
        anchor: Position::new(0.0, 0.0, 0.0),
    });
}

fn enqueue_movement(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut events: ResMut<ClientEventQueue>,
) {
    let east = axis(&keys, KeyCode::KeyD, KeyCode::KeyA);
    let north = axis(&keys, KeyCode::KeyW, KeyCode::KeyS);
    let input = Vec2::new(east, north).normalize_or_zero();
    if input != Vec2::ZERO {
        let distance = MOVE_SPEED * time.delta_secs();
        events.push(ClientEvent::PredictPlanar {
            east: input.x * distance,
            north: input.y * distance,
        });
    }
}

fn script_proof(
    proof: Res<ProofRun>,
    mut scripted: Local<bool>,
    mut events: ResMut<ClientEventQueue>,
    mut rig: ResMut<CameraRig>,
) {
    if proof.output.is_some() && !*scripted {
        events.push(ClientEvent::PredictPlanar {
            east: 1.25,
            north: 0.75,
        });
        rig.yaw -= 0.24;
        rig.pitch = -0.48;
        rig.distance = 10.0;
        *scripted = true;
    }
}

fn axis(keys: &ButtonInput<KeyCode>, positive: KeyCode, negative: KeyCode) -> f32 {
    f32::from(keys.pressed(positive)) - f32::from(keys.pressed(negative))
}

fn update_camera(
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    time: Res<Time>,
    mut rig: ResMut<CameraRig>,
    avatar: Single<&Transform, (With<Avatar>, Without<Camera3d>)>,
    mut camera: Single<&mut Transform, (With<Camera3d>, Without<Avatar>)>,
) {
    let seconds = time.delta_secs();
    rig.yaw += axis(&keys, KeyCode::ArrowLeft, KeyCode::ArrowRight) * seconds * 1.4;
    rig.pitch += axis(&keys, KeyCode::ArrowDown, KeyCode::ArrowUp) * seconds * 1.0;
    rig.distance += axis(&keys, KeyCode::KeyE, KeyCode::KeyQ) * seconds * 5.0;

    if mouse_buttons.pressed(MouseButton::Left) {
        rig.yaw -= mouse_motion.delta.x * 0.004;
        rig.pitch -= mouse_motion.delta.y * 0.004;
    }

    rig.pitch = rig.pitch.clamp(-FRAC_PI_2 + 0.12, -0.08);
    rig.distance = rig.distance.clamp(4.0, 18.0);

    let focus = avatar.translation + Vec3::Y * 0.65;
    let orbit = Quat::from_euler(EulerRot::YXZ, rig.yaw, rig.pitch, 0.0);
    camera.translation = focus + orbit * Vec3::Z * rig.distance;
    camera.look_at(focus, Vec3::Y);
}

fn sync_avatar_and_ui(
    model: Res<ClientModel>,
    mut avatar: Single<&mut Transform, (With<Avatar>, Without<Camera3d>)>,
    mut status: Single<&mut Text, With<StatusText>>,
) {
    let state = model.state();
    avatar.translation = Vec3::new(
        state.rendered.east,
        state.rendered.elevation + 0.74,
        -state.rendered.north,
    );
    status.0 = format!(
        "WORLD READY  /  PLACEHOLDER REALM\n\nRENDERED    {:>6.2}  {:>6.2}  {:>5.2}\nSUBMITTED   {:>6.2}  {:>6.2}  {:>5.2}\nOBSERVED    {:>6.2}  {:>6.2}  {:>5.2}\n\nENGINE      BEVY 0.19.0\nADAPTER     THIN / HEADLESS-TESTED",
        state.rendered.east,
        state.rendered.north,
        state.rendered.elevation,
        state.submitted.east,
        state.submitted.north,
        state.submitted.elevation,
        state.realm_observed.east,
        state.realm_observed.north,
        state.realm_observed.elevation,
    );
}

fn capture_proof(
    mut commands: Commands,
    mut proof: ResMut<ProofRun>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(output) = proof.output.clone() else {
        return;
    };
    proof.frame += 1;

    if proof.requested && output.exists() {
        info!("rendered proof saved to {}", output.display());
        exit.write(AppExit::Success);
    } else if proof.frame == PROOF_CAPTURE_FRAME {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(output));
        proof.requested = true;
    } else if proof.frame > PROOF_TIMEOUT_FRAME {
        panic!("timed out while waiting for the rendered proof artifact");
    }
}
