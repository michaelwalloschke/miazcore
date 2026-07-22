use std::{
    fs,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

use client_session::{
    ClientConfig, ClientConfigSpec, ClientEventKind, ClientPhase, CredentialPaths, MovementIntent,
    OfflineSession,
};

#[test]
fn credentials_enter_only_the_offline_session_boundary() {
    let credentials = TestCredentials::new();
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
    let session = OfflineSession::start(loaded).unwrap();

    let snapshot = session.snapshot();
    let events = session.drain_events();

    assert_eq!(snapshot.phase, ClientPhase::Offline);
    assert_eq!(snapshot.identity.realm_id(), 1);
    assert_eq!(snapshot.identity.realm_name(), "Miazcore Reference Realm");
    assert_eq!(snapshot.identity.character_name(), "Miaztest");
    assert!(snapshot.entry_anchor.is_none());
    assert!(snapshot.predicted_pose.is_none());
    assert!(snapshot.submitted_pose.is_none());
    assert!(snapshot.realm_observed_pose.is_none());
    assert!(events.iter().any(|event| matches!(
        event.kind,
        ClientEventKind::PhaseChanged {
            phase: ClientPhase::Offline
        }
    )));
    let formatted = format!("{snapshot:?} {events:?}");
    assert!(!formatted.contains("SYNTHETIC_ACCOUNT"));
    assert!(!formatted.contains("synthetic-password"));

    session.publish_movement_intent(MovementIntent::planar(1.0, 0.0).unwrap());
    let after_intent = session.snapshot();
    assert_eq!(after_intent.queue_counters.movement_revision, 1);
    assert!(after_intent.predicted_pose.is_none());

    session.shutdown().unwrap();
}

struct TestCredentials {
    root: std::path::PathBuf,
    account: std::path::PathBuf,
    password: std::path::PathBuf,
}

impl TestCredentials {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("miazcore-offline-test-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
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
