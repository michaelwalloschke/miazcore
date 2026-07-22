use bevy::prelude::{App, MinimalPlugins};
use bevy_shell_platform_proof::{
    adapter::{ClientEventQueue, ClientModel, install_client_adapter},
    model::{ClientEvent, Position, WorldPhase},
};

#[test]
fn scripted_engine_free_events_run_through_a_headless_bevy_app() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    install_client_adapter(&mut app);

    let anchor = Position::new(10.0, -4.0, 1.25);
    let observed = Position::new(10.5, -3.75, 1.25);
    app.world_mut().resource_mut::<ClientEventQueue>().extend([
        ClientEvent::EnterWorld { anchor },
        ClientEvent::PredictPlanar {
            east: 1.0,
            north: 2.0,
        },
        ClientEvent::MarkSubmitted,
        ClientEvent::ObserveRealm { position: observed },
        ClientEvent::PredictPlanar {
            east: -0.25,
            north: 0.5,
        },
    ]);

    app.update();

    let state = app.world().resource::<ClientModel>().state();
    assert_eq!(state.phase, WorldPhase::Ready);
    assert_eq!(state.rendered, Position::new(10.75, -1.5, 1.25));
    assert_eq!(state.submitted, Position::new(11.0, -2.0, 1.25));
    assert_eq!(state.realm_observed, observed);
}
