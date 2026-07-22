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
    ControlCommand, CredentialPaths, EntryStage, FailureCategory, RecoveryAction,
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
        "nonexistent-account" => verify_failure(
            snapshot,
            evidence.events(),
            ExpectedFailure {
                category: FailureCategory::Authentication,
                stage: "login authentication",
                context: "fixture account authentication was rejected",
                recovery: RecoveryAction::CheckCredentials,
                stages: &[EntryStage::LoginConnection, EntryStage::LoginAuthentication],
                character: None,
            },
        )?,
        "absent-character" => verify_failure(
            snapshot,
            evidence.events(),
            ExpectedFailure {
                category: FailureCategory::Configuration,
                stage: "character selection",
                context: "configured character was absent from the authenticated realm",
                recovery: RecoveryAction::FixConfiguration,
                stages: &[
                    EntryStage::LoginConnection,
                    EntryStage::LoginAuthentication,
                    EntryStage::RealmSelection,
                    EntryStage::WorldAuthentication,
                    EntryStage::CharacterSelection,
                ],
                character: Some(ABSENT_CHARACTER),
            },
        )?,
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
        || entering_stages(events)
            != [
                EntryStage::LoginConnection,
                EntryStage::LoginAuthentication,
                EntryStage::RealmSelection,
                EntryStage::WorldAuthentication,
                EntryStage::CharacterSelection,
            ]
        || !has_success_events(events)
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

#[derive(Clone, Copy)]
struct ExpectedFailure<'a> {
    category: FailureCategory,
    stage: &'a str,
    context: &'a str,
    recovery: RecoveryAction,
    stages: &'a [EntryStage],
    character: Option<&'a str>,
}

fn verify_failure(
    snapshot: &client_session::ClientSnapshot,
    events: &[client_session::ClientEvent],
    expected: ExpectedFailure<'_>,
) -> Result<(), Box<dyn Error>> {
    let failure = snapshot
        .latest_failure
        .as_ref()
        .ok_or_else(|| io::Error::other("negative probe unexpectedly produced no failure"))?;
    let rejected = events
        .iter()
        .filter(|event| matches!(event.kind, ClientEventKind::CommandRejected { .. }))
        .count();
    let diagnostic_matches = snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message() == failure.context());
    let character_matches = expected.character.is_none_or(|expected_character| {
        snapshot.identity.character_name() == expected_character
            && failure.context().contains("configured character")
    });
    if failure.category() != expected.category
        || failure.stage() != expected.stage
        || failure.context() != expected.context
        || failure.recommended_recovery() != expected.recovery
        || !matches!(snapshot.phase, ClientPhase::Failed(_))
        || entering_stages(events) != expected.stages
        || rejected != 1
        || !diagnostic_matches
        || !character_matches
        || events.iter().any(|event| {
            matches!(
                event.kind,
                ClientEventKind::CharacterSelected { .. }
                    | ClientEventKind::PhaseChanged {
                        phase: ClientPhase::Entering(EntryStage::Bootstrap),
                    }
            )
        })
        || !matches!(
            events.last().map(|event| &event.kind),
            Some(ClientEventKind::Disconnected)
        )
    {
        return Err(io::Error::other(format!(
            "negative probe contract mismatch: expected {:?}, got {:?}",
            expected.category,
            failure.category()
        ))
        .into());
    }
    println!(
        "character selection negative probe: PASS category={:?} stage={} configured_character={} recovery={:?} retries=0 disconnected=true player_login_sent=false",
        failure.category(),
        failure.stage(),
        expected.character.unwrap_or("not-applicable"),
        failure.recommended_recovery(),
    );
    Ok(())
}

fn entering_stages(events: &[client_session::ClientEvent]) -> Vec<EntryStage> {
    events
        .iter()
        .filter_map(|event| match &event.kind {
            ClientEventKind::PhaseChanged {
                phase: ClientPhase::Entering(stage),
            } => Some(*stage),
            _ => None,
        })
        .collect()
}

fn has_success_events(events: &[client_session::ClientEvent]) -> bool {
    events
        .iter()
        .any(|event| matches!(event.kind, ClientEventKind::RealmDiscovered { .. }))
        && events
            .iter()
            .filter(|event| matches!(event.kind, ClientEventKind::CharacterSelected { .. }))
            .count()
            == 1
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
