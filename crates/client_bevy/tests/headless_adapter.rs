use std::{
    fs,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use bevy::prelude::{App, MinimalPlugins};
use client_bevy::{
    ClientScheduleSet, DiagnosticView, LearningClientPlugin, SessionBridge, SystemOrderTrace,
};
use client_session::{
    ClientConfig, ClientConfigSpec, ClientPhase, CredentialPaths, OfflineSession,
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
