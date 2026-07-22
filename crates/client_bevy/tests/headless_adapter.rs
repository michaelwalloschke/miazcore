use std::{
    fs,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::atomic::{AtomicU64, Ordering},
    sync::{Arc, Mutex},
    time::Duration,
};

use bevy::prelude::{App, MinimalPlugins};
use client_bevy::{
    ClientScheduleSet, DiagnosticSession, DiagnosticView, LearningClientPlugin, SessionBridge,
    SystemOrderTrace,
};
use client_session::{
    BoundaryError, ClientConfig, ClientConfigSpec, ClientEvent, ClientEventKind, ClientPhase,
    ControlCommand, CredentialPaths, OfflineSession, SanitizedIdentity, WorldPose,
};

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn minimal_plugins_projects_offline_state_in_the_contractual_order() {
    let credentials = TestCredentials::new();
    let session = offline_session(&credentials);
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(SessionBridge::new(session))
        .insert_resource(SystemOrderTrace::default())
        .add_plugins(LearningClientPlugin::headless());

    app.update();

    assert_eq!(
        app.world().resource::<SystemOrderTrace>().entries(),
        &[
            ClientScheduleSet::Ingress,
            ClientScheduleSet::Input,
            ClientScheduleSet::Presentation,
            ClientScheduleSet::Camera,
            ClientScheduleSet::Diagnostics,
        ]
    );
    let view = app.world().resource::<DiagnosticView>();
    assert_eq!(view.snapshot().phase, ClientPhase::Offline);
    assert_eq!(view.snapshot().identity.character_name(), "Miaztest");
    assert_eq!(view.recent_events().count(), 2);
}

#[test]
fn offline_projection_has_no_network_pose_or_speed_claims() {
    let credentials = TestCredentials::new();
    let session = offline_session(&credentials);
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(SessionBridge::new(session))
        .add_plugins(LearningClientPlugin::headless());

    app.update();

    let snapshot = app.world().resource::<DiagnosticView>().snapshot();
    assert!(snapshot.entry_anchor.is_none());
    assert!(snapshot.predicted_pose.is_none());
    assert!(snapshot.submitted_pose.is_none());
    assert!(snapshot.realm_observed_pose.is_none());
    assert!(snapshot.run_speed.is_none());
}

#[test]
fn minimal_plugins_projects_live_entry_truth_and_only_starts_the_complete_operation() {
    let identity =
        SanitizedIdentity::new(1, "Miazcore Reference Realm", "Miaztest", 12_340).unwrap();
    let anchor = WorldPose {
        map_id: 0,
        east: -8949.95,
        north: -132.493,
        elevation: 83.5312,
        orientation: 0.0,
    };
    let mut snapshot = client_session::ClientSnapshot::offline(identity.clone());
    snapshot.phase = ClientPhase::MovementReady;
    snapshot.entry_anchor = Some(anchor);
    snapshot.realm_observed_pose = Some(anchor);
    snapshot.run_speed = Some(7.0);
    let fake = FakeLiveSession::new(
        snapshot,
        vec![ClientEvent {
            sequence: 3,
            kind: ClientEventKind::PhaseChanged {
                phase: ClientPhase::MovementReady,
            },
        }],
    );
    let controls = Arc::clone(&fake.controls);

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(SessionBridge::new(fake))
        .insert_resource(SystemOrderTrace::default())
        .add_plugins(LearningClientPlugin::headless());

    app.world()
        .resource::<SessionBridge>()
        .start_entry()
        .unwrap();
    app.update();

    let view = app.world().resource::<DiagnosticView>();
    assert!(view.is_live_entry());
    assert_eq!(view.snapshot().phase, ClientPhase::MovementReady);
    assert_eq!(view.snapshot().entry_anchor, Some(anchor));
    assert_eq!(view.snapshot().realm_observed_pose, Some(anchor));
    assert_eq!(view.snapshot().run_speed, Some(7.0));
    assert_eq!(view.recent_events().count(), 1);
    assert_eq!(
        controls.lock().unwrap().as_slice(),
        &[ControlCommand::StartEntry]
    );
}

fn offline_session(credentials: &TestCredentials) -> OfflineSession {
    let loaded = ClientConfig::new(ClientConfigSpec {
        realm_id: 1,
        realm_name: "Miazcore Reference Realm".to_owned(),
        character_name: "Miaztest".to_owned(),
        client_build: 12_340,
        login_endpoint: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3724),
        world_endpoint: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8085),
        connect_timeout: Duration::from_secs(5),
        io_timeout: Duration::from_secs(5),
        credentials: CredentialPaths::new(&credentials.account, &credentials.password),
    })
    .unwrap()
    .load()
    .unwrap();
    OfflineSession::start(loaded).unwrap()
}

struct TestCredentials {
    root: std::path::PathBuf,
    account: std::path::PathBuf,
    password: std::path::PathBuf,
}

impl TestCredentials {
    fn new() -> Self {
        let unique = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "miazcore-bevy-test-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        let account = root.join("account");
        let password = root.join("password");
        write_secret(&account, b"SYNTHETIC_ACCOUNT\n");
        write_secret(&password, b"synthetic-password\n");
        Self {
            root,
            account,
            password,
        }
    }
}

impl Drop for TestCredentials {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

fn write_secret(path: &std::path::Path, value: &[u8]) {
    fs::write(path, value).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
}

struct FakeLiveSession {
    snapshot: client_session::ClientSnapshot,
    events: Mutex<Vec<ClientEvent>>,
    controls: Arc<Mutex<Vec<ControlCommand>>>,
}

impl FakeLiveSession {
    fn new(snapshot: client_session::ClientSnapshot, events: Vec<ClientEvent>) -> Self {
        Self {
            snapshot,
            events: Mutex::new(events),
            controls: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl DiagnosticSession for FakeLiveSession {
    fn snapshot(&self) -> client_session::ClientSnapshot {
        self.snapshot.clone()
    }

    fn drain_events(&self) -> Vec<ClientEvent> {
        std::mem::take(&mut *self.events.lock().unwrap())
    }

    fn send_control(&self, command: ControlCommand) -> Result<(), BoundaryError> {
        self.controls.lock().unwrap().push(command);
        Ok(())
    }

    fn is_live_entry(&self) -> bool {
        true
    }
}
