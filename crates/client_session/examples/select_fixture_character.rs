use std::{
    error::Error,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use client_session::{
    CharacterSelectionSession, ClientConfig, ClientConfigSpec, ClientEventKind, ClientPhase,
    ControlCommand, CredentialPaths, EntryStage, FailureCategory,
};

const ABSENT_CHARACTER: &str = "Miazmissing";

fn main() -> Result<(), Box<dyn Error>> {
    let mode = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "success".to_owned());
    let repository_root = std::env::current_dir()?;
    let mut temporary_credentials = None;
    let config = match mode.as_str() {
        "success" => ClientConfig::reference_realm(&repository_root)?,
        "absent-character" => configured(
            &repository_root,
            ABSENT_CHARACTER,
            reference_credential_paths(&repository_root),
        )?,
        "nonexistent-account" => {
            let credentials = TemporaryCredentials::create()?;
            let config = configured(&repository_root, "Miaztest", credentials.paths())?;
            temporary_credentials = Some(credentials);
            config
        }
        _ => {
            return Err(io::Error::other(
                "usage: select_fixture_character [success|nonexistent-account|absent-character]",
            )
            .into());
        }
    };

    let session = CharacterSelectionSession::start(config.load()?)?;
    session.send_control(ControlCommand::StartEntry)?;
    let evidence = session.wait()?;
    drop(temporary_credentials);
    let snapshot = evidence.snapshot();

    match mode.as_str() {
        "success" => verify_success(snapshot, evidence.events())?,
        "nonexistent-account" => verify_failure(snapshot, FailureCategory::Authentication)?,
        "absent-character" => verify_failure(snapshot, FailureCategory::Configuration)?,
        _ => unreachable!(),
    }
    Ok(())
}

fn verify_success(
    snapshot: &client_session::ClientSnapshot,
    events: &[client_session::ClientEvent],
) -> Result<(), Box<dyn Error>> {
    if let Some(failure) = &snapshot.latest_failure {
        return Err(io::Error::other(format!(
            "character selection failed: {:?} / {} / {:?}",
            failure.category(),
            failure.context(),
            failure.recommended_recovery()
        ))
        .into());
    }
    let character = snapshot
        .selected_character
        .as_ref()
        .ok_or_else(|| io::Error::other("character selection produced no selected character"))?;
    if snapshot.phase != ClientPhase::Offline
        || character.name() != "Miaztest"
        || !has_expected_semantic_path(events)
    {
        return Err(
            io::Error::other("character-selection evidence did not match the contract").into(),
        );
    }
    println!(
        "character selection: PASS name={} level={} map={} area={} semantic_events={} disconnected=true player_login_sent=false",
        character.name(),
        character.level(),
        character.map_id(),
        character.area_id(),
        events.len(),
    );
    Ok(())
}

fn verify_failure(
    snapshot: &client_session::ClientSnapshot,
    expected: FailureCategory,
) -> Result<(), Box<dyn Error>> {
    let failure = snapshot
        .latest_failure
        .as_ref()
        .ok_or_else(|| io::Error::other("negative probe unexpectedly produced no failure"))?;
    if failure.category() != expected || !matches!(snapshot.phase, ClientPhase::Failed(_)) {
        return Err(io::Error::other(format!(
            "negative probe category mismatch: expected {expected:?}, got {:?}",
            failure.category()
        ))
        .into());
    }
    println!(
        "character selection negative probe: PASS category={:?} stage={} recovery={:?}",
        failure.category(),
        failure.stage(),
        failure.recommended_recovery(),
    );
    Ok(())
}

fn has_expected_semantic_path(events: &[client_session::ClientEvent]) -> bool {
    let expected = [
        ClientPhase::Entering(EntryStage::LoginConnection),
        ClientPhase::Entering(EntryStage::LoginAuthentication),
        ClientPhase::Entering(EntryStage::RealmSelection),
        ClientPhase::Entering(EntryStage::WorldAuthentication),
        ClientPhase::Entering(EntryStage::CharacterSelection),
    ];
    let mut expected_index = 0;
    for phase in events.iter().filter_map(|event| match &event.kind {
        ClientEventKind::PhaseChanged { phase } => Some(phase),
        _ => None,
    }) {
        if expected.get(expected_index) == Some(phase) {
            expected_index += 1;
        }
    }
    expected_index == expected.len()
        && events
            .iter()
            .any(|event| matches!(event.kind, ClientEventKind::RealmDiscovered { .. }))
        && events
            .iter()
            .any(|event| matches!(event.kind, ClientEventKind::CharacterSelected { .. }))
        && matches!(
            events.last().map(|event| &event.kind),
            Some(ClientEventKind::Disconnected)
        )
}

fn configured(
    repository_root: &Path,
    character_name: &str,
    credentials: CredentialPaths,
) -> Result<ClientConfig, client_session::ConfigError> {
    let reference = ClientConfig::reference_realm(repository_root)?;
    ClientConfig::new(ClientConfigSpec {
        realm_id: reference.identity().realm_id(),
        realm_name: reference.identity().realm_name().to_owned(),
        character_name: character_name.to_owned(),
        client_build: reference.identity().client_build(),
        login_endpoint: reference.login_endpoint(),
        world_endpoint: reference.world_endpoint(),
        connect_timeout: reference.connect_timeout(),
        io_timeout: reference.io_timeout(),
        credentials,
    })
}

fn reference_credential_paths(repository_root: &Path) -> CredentialPaths {
    let root = repository_root.join("infra/azerothcore/secrets");
    CredentialPaths::new(root.join("fixture-account"), root.join("fixture-password"))
}

struct TemporaryCredentials {
    root: PathBuf,
    paths: CredentialPaths,
}

impl TemporaryCredentials {
    fn create() -> io::Result<Self> {
        let root = std::env::temp_dir().join(format!(
            "miazcore-character-probe-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(io::Error::other)?
                .as_nanos()
        ));
        fs::create_dir(&root)?;
        let account = root.join("account");
        let password = root.join("password");
        write_private(
            &account,
            format!("MIAZNONE{}", std::process::id()).as_bytes(),
        )?;
        write_private(&password, b"NOTAREALFIXTUREPASSWORD")?;
        Ok(Self {
            root,
            paths: CredentialPaths::new(account, password),
        })
    }

    fn paths(&self) -> CredentialPaths {
        self.paths.clone()
    }
}

impl Drop for TemporaryCredentials {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.paths.account());
        let _ = fs::remove_file(self.paths.password());
        let _ = fs::remove_dir(&self.root);
    }
}

fn write_private(path: &Path, value: &[u8]) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(path)?;
    file.write_all(value)
}
