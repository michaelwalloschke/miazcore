//! Thin Bevy adapter and project-owned Diagnostic World presentation.
// Bevy constructs system parameters by value; references are not valid SystemParam types.
#![allow(clippy::needless_pass_by_value)]

mod bridge;
mod camera;
mod diagnostics;
mod input;
mod proof;
mod world;

use bevy::prelude::*;

pub use bridge::{DiagnosticView, SessionBridge};
pub use proof::RenderProofPlugin;
pub use world::OfflinePresentation;

use bridge::SessionBridgePlugin;
use camera::ChaseCameraPlugin;
use diagnostics::DiagnosticsPlugin;
use input::OfflineInputPlugin;
use world::DiagnosticWorldPlugin;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub enum ClientScheduleSet {
    Ingress,
    Input,
    Presentation,
    Camera,
    Diagnostics,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresentationMode {
    Headless,
    Windowed,
}

/// Composes the ordered session bridge and optional visual Diagnostic World.
pub struct LearningClientPlugin {
    mode: PresentationMode,
}

impl LearningClientPlugin {
    #[must_use]
    pub const fn headless() -> Self {
        Self {
            mode: PresentationMode::Headless,
        }
    }

    #[must_use]
    pub const fn windowed() -> Self {
        Self {
            mode: PresentationMode::Windowed,
        }
    }
}

impl Plugin for LearningClientPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            Update,
            (
                ClientScheduleSet::Ingress,
                ClientScheduleSet::Input,
                ClientScheduleSet::Presentation,
                ClientScheduleSet::Camera,
                ClientScheduleSet::Diagnostics,
            )
                .chain(),
        )
        .add_systems(Update, trace_ingress.in_set(ClientScheduleSet::Ingress))
        .add_systems(Update, trace_input.in_set(ClientScheduleSet::Input))
        .add_systems(
            Update,
            trace_presentation.in_set(ClientScheduleSet::Presentation),
        )
        .add_systems(Update, trace_camera.in_set(ClientScheduleSet::Camera))
        .add_systems(
            Update,
            trace_diagnostics.in_set(ClientScheduleSet::Diagnostics),
        )
        .add_plugins(SessionBridgePlugin);

        if self.mode == PresentationMode::Windowed {
            app.add_plugins((
                OfflineInputPlugin,
                DiagnosticWorldPlugin,
                ChaseCameraPlugin,
                DiagnosticsPlugin,
            ));
        }
    }
}

#[derive(Debug, Default, Resource)]
pub struct SystemOrderTrace(Vec<ClientScheduleSet>);

impl SystemOrderTrace {
    #[must_use]
    pub fn entries(&self) -> &[ClientScheduleSet] {
        &self.0
    }
}

fn trace_ingress(trace: Option<ResMut<SystemOrderTrace>>) {
    trace_set(trace, ClientScheduleSet::Ingress);
}

fn trace_input(trace: Option<ResMut<SystemOrderTrace>>) {
    trace_set(trace, ClientScheduleSet::Input);
}

fn trace_presentation(trace: Option<ResMut<SystemOrderTrace>>) {
    trace_set(trace, ClientScheduleSet::Presentation);
}

fn trace_camera(trace: Option<ResMut<SystemOrderTrace>>) {
    trace_set(trace, ClientScheduleSet::Camera);
}

fn trace_diagnostics(trace: Option<ResMut<SystemOrderTrace>>) {
    trace_set(trace, ClientScheduleSet::Diagnostics);
}

fn trace_set(mut trace: Option<ResMut<SystemOrderTrace>>, set: ClientScheduleSet) {
    if let Some(trace) = trace.as_deref_mut() {
        trace.0.push(set);
    }
}

pub(crate) fn input_axis(keys: &ButtonInput<KeyCode>, positive: KeyCode, negative: KeyCode) -> f32 {
    f32::from(keys.pressed(positive)) - f32::from(keys.pressed(negative))
}
