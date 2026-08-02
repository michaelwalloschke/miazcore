//! Closed, parent-owned coordination boundary for the shared-host proof.
//!
//! This is intentionally not a general remote-control API: the only accepted
//! inputs are one next revision and one of four bounded commands.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use bevy::{app::AppExit, prelude::*};
use client_session::{
    ClientEvent, ClientEventKind, ClientPhase, MovementIntent, RemoteAvatarChange,
};
use serde::{Deserialize, Serialize};

use crate::{ClientScheduleSet, DiagnosticView, SessionBridge};

const COMMAND_FILE: &str = "command.json";
const SIDECAR_FILE: &str = "sidecar.json";
const ROLE_TURN_DURATION: Duration = Duration::from_millis(420);
const PROJECTION_SNAP_VISIBLE_FOR: Duration = Duration::from_secs(3);

#[derive(Clone, Debug)]
pub struct SharedHostProofConfig {
    profile: &'static str,
    directory: PathBuf,
    attempt_id: String,
}

impl SharedHostProofConfig {
    /// Admit an existing profile-bound directory below the parent-owned
    /// repository `.scratch` workspace. Credentials and network endpoints are
    /// deliberately absent from this interface.
    ///
    /// # Errors
    ///
    /// Returns only a redacted admission error.
    pub fn admit(
        repository_root: &Path,
        profile: &'static str,
        directory: PathBuf,
    ) -> Result<Self, String> {
        if !matches!(profile, "pair-a" | "pair-b") {
            return Err("shared-host proof requires a closed Pair profile".to_owned());
        }
        let scratch = fs::canonicalize(repository_root.join(".scratch"))
            .map_err(|_| "shared-host proof requires repository .scratch workspace".to_owned())?;
        let metadata = fs::symlink_metadata(&directory)
            .map_err(|_| "shared-host proof directory must be parent-created".to_owned())?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err("shared-host proof directory must be a real directory".to_owned());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o077 != 0 {
                return Err("shared-host proof directory is not private to its parent".to_owned());
            }
        }
        let directory = fs::canonicalize(directory)
            .map_err(|_| "shared-host proof directory is unavailable".to_owned())?;
        if !directory.starts_with(&scratch)
            || directory.file_name().and_then(|name| name.to_str()) != Some(profile)
        {
            return Err(
                "shared-host proof directory is not owned by the admitted profile".to_owned(),
            );
        }
        let admission_path = directory.join("admission.json");
        let admission_metadata = fs::symlink_metadata(&admission_path)
            .map_err(|_| "shared-host proof admission is missing".to_owned())?;
        if admission_metadata.file_type().is_symlink() || !admission_metadata.is_file() {
            return Err("shared-host proof admission must be a regular file".to_owned());
        }
        if admission_metadata.len() > 512 {
            return Err("shared-host proof admission is oversized".to_owned());
        }
        let admission_bytes = fs::read(&admission_path)
            .map_err(|_| "shared-host proof admission is unreadable".to_owned())?;
        if [
            b"password".as_slice(),
            b"credential",
            b"endpoint",
            b"account",
        ]
        .iter()
        .any(|forbidden| {
            admission_bytes
                .windows(forbidden.len())
                .any(|window| window.eq_ignore_ascii_case(forbidden))
        }) {
            return Err("shared-host proof admission contains forbidden vocabulary".to_owned());
        }
        let admission: AdmissionDocument = serde_json::from_slice(&admission_bytes)
            .map_err(|_| "shared-host proof admission is malformed".to_owned())?;
        if admission.schema != "miazcore.shared-host-proof-admission.v1"
            || admission.profile != profile
            || !valid_token(&admission.attempt_id)
        {
            return Err("shared-host proof admission does not bind this profile".to_owned());
        }
        let attempt_id = admission.attempt_id;
        Ok(Self {
            profile,
            directory,
            attempt_id,
        })
    }

    #[must_use]
    pub const fn profile(&self) -> &'static str {
        self.profile
    }
}

pub struct SharedHostProofPlugin {
    config: SharedHostProofConfig,
}

impl SharedHostProofPlugin {
    #[must_use]
    pub const fn new(config: SharedHostProofConfig) -> Self {
        Self { config }
    }
}

impl Plugin for SharedHostProofPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SharedHostProofInputGate)
            .insert_resource(SharedHostProofState::new(self.config.clone()))
            .add_systems(
                Update,
                drive_proof_controls.in_set(ClientScheduleSet::Input),
            )
            .add_systems(
                Update,
                publish_sidecar.in_set(ClientScheduleSet::Diagnostics),
            );
    }
}

/// Presence of this resource means the parent owns all movement publication.
/// Camera and focus controls remain local, but keyboard movement is gated.
#[derive(Default, Resource)]
pub(crate) struct SharedHostProofInputGate;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Command {
    Idle,
    PerformRoleTurn,
    ShowProjectionSnap,
    RequestCleanShutdown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct CommandDocument {
    revision: u64,
    command: Command,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CommandGeneration {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(not(unix))]
    modified: std::time::SystemTime,
    #[cfg(not(unix))]
    length: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CommandEnvelope {
    document: CommandDocument,
    generation: CommandGeneration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AcceptedCommand {
    revision: u64,
    command: Command,
    generation: CommandGeneration,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdmissionDocument {
    schema: String,
    attempt_id: String,
    profile: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum TerminalState {
    Active,
    Offline,
    Failed,
}

#[derive(Clone, Debug, Serialize)]
struct EventEvidence {
    sequence: u64,
    kind: EventKind,
    remote: Option<RemoteEvidence>,
    submitted: Option<SubmittedEvidence>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
enum EventKind {
    RemoteCreated,
    RemoteUpdated,
    RemoteRemoved,
    RemoteFaulted,
    MovementSubmitted,
}

#[derive(Clone, Debug, Serialize)]
struct RemoteEvidence {
    guid: String,
    realm_observed_pose: Option<Pose>,
    fault_category: Option<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
struct SubmittedEvidence {
    pose: Pose,
    stopped: bool,
}

#[derive(Clone, Debug, Serialize)]
struct Terminal {
    state: TerminalState,
    failure_category: Option<&'static str>,
    acknowledged_revision: Option<u64>,
    command_result: Option<Command>,
    projection_snap_acknowledged: bool,
}

#[derive(Clone, Debug, Serialize)]
struct Sidecar<'a> {
    schema: &'static str,
    attempt_id: &'a str,
    profile: &'a str,
    guid: Option<String>,
    entry_anchor: Option<Pose>,
    movement_ready: bool,
    events: Vec<EventEvidence>,
    terminal: Terminal,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct Pose {
    map_id: u32,
    east: f32,
    north: f32,
    elevation: f32,
    orientation: f32,
}

impl From<client_session::WorldPose> for Pose {
    fn from(value: client_session::WorldPose) -> Self {
        Self {
            map_id: value.map_id,
            east: value.east,
            north: value.north,
            elevation: value.elevation,
            orientation: value.orientation,
        }
    }
}

#[allow(clippy::struct_excessive_bools)] // explicit bounded proof lifecycle latches
#[derive(Resource)]
struct SharedHostProofState {
    config: SharedHostProofConfig,
    revision: u64,
    acknowledged: Option<AcceptedCommand>,
    pending: Option<AcceptedCommand>,
    role_turn_started: Option<Instant>,
    stop_requested_snapshot_revision: Option<u64>,
    stop_requested_after_sequence: u64,
    projection_snap: bool,
    projection_snap_restore_at: Option<Instant>,
    role_turn_completed: bool,
    projection_snap_completed: bool,
    shutdown_requested: bool,
    entry_requested: bool,
    terminal: TerminalState,
    failure_category: Option<&'static str>,
    event_history: Vec<EventEvidence>,
    last_event_sequence: u64,
    last_sidecar: Option<String>,
    frozen_revision: Option<u64>,
}

impl SharedHostProofState {
    fn new(config: SharedHostProofConfig) -> Self {
        Self {
            config,
            revision: 0,
            acknowledged: None,
            pending: None,
            role_turn_started: None,
            stop_requested_snapshot_revision: None,
            stop_requested_after_sequence: 0,
            projection_snap: false,
            projection_snap_restore_at: None,
            role_turn_completed: false,
            projection_snap_completed: false,
            shutdown_requested: false,
            entry_requested: false,
            terminal: TerminalState::Active,
            failure_category: None,
            event_history: Vec::new(),
            last_event_sequence: 0,
            last_sidecar: None,
            frozen_revision: None,
        }
    }

    fn command_path(&self) -> PathBuf {
        self.config.directory.join(COMMAND_FILE)
    }

    fn sidecar_path(&self) -> PathBuf {
        self.config.directory.join(SIDECAR_FILE)
    }

    fn acknowledge(&mut self, accepted: AcceptedCommand) {
        self.revision = accepted.revision;
        self.acknowledged = Some(accepted);
        self.pending = None;
    }

    fn frozen_sidecar_path(&self, revision: u64) -> PathBuf {
        self.config
            .directory
            .join(format!("sidecar.revision-{revision}.json"))
    }
}

#[allow(clippy::too_many_lines)] // one closed command state machine is auditable
fn drive_proof_controls(
    mut state: ResMut<SharedHostProofState>,
    view: Res<DiagnosticView>,
    session: Res<SessionBridge>,
    mut remote_avatar: ResMut<crate::remote_avatar::RemoteAvatarPresentation>,
    proof_ingress: Res<crate::bridge::ProofEventIngress>,
) {
    if state.terminal != TerminalState::Active {
        return;
    }
    if state.shutdown_requested && view.snapshot().phase == ClientPhase::Offline {
        state.terminal = TerminalState::Offline;
        return;
    }
    if matches!(view.snapshot().phase, ClientPhase::Failed(_)) {
        state.terminal = TerminalState::Failed;
        state.failure_category = view
            .snapshot()
            .latest_failure
            .as_ref()
            .map(|failure| match failure.category() {
                client_session::FailureCategory::Configuration => "configuration",
                client_session::FailureCategory::Authentication => "authentication",
                client_session::FailureCategory::Transport => "transport",
                client_session::FailureCategory::ProtocolIncompatibility => {
                    "protocol-incompatibility"
                }
                client_session::FailureCategory::UnsupportedSelfControl => {
                    "unsupported-self-control"
                }
                client_session::FailureCategory::Timeout => "timeout",
                client_session::FailureCategory::InternalBackpressure => "internal-backpressure",
            })
            .or(Some("proof-control"));
        return;
    }
    if state
        .projection_snap_restore_at
        .is_some_and(|deadline| Instant::now() >= deadline)
    {
        remote_avatar.restore_scripted_projection_snap();
        state.projection_snap_restore_at = None;
    }
    if !state.entry_requested && view.snapshot().phase == ClientPhase::Offline {
        if session.start_entry().is_err() {
            state.terminal = TerminalState::Failed;
        } else {
            state.entry_requested = true;
        }
        return;
    }
    if let Some(accepted) = state.pending
        && accepted.command == Command::PerformRoleTurn
        && state
            .role_turn_started
            .is_some_and(|started| started.elapsed() >= ROLE_TURN_DURATION)
    {
        if session
            .publish_movement_intent(MovementIntent::idle())
            .is_err()
        {
            state.terminal = TerminalState::Failed;
        } else {
            state.role_turn_started = None;
            state.stop_requested_snapshot_revision =
                Some(view.snapshot().queue_counters.snapshot_revision);
            state.stop_requested_after_sequence = proof_ingress
                .0
                .last()
                .map_or(state.last_event_sequence, |event| event.sequence);
        }
        return;
    }
    if let Some(accepted) = state.pending
        && accepted.command == Command::PerformRoleTurn
        && state
            .stop_requested_snapshot_revision
            .is_some_and(|revision| {
                view.snapshot().queue_counters.snapshot_revision > revision
                    && view.snapshot().submitted_pose_is_stopped
                    && view.snapshot().submitted_pose.is_some()
            })
        && has_post_stop_submission(
            &proof_ingress.0,
            state.stop_requested_after_sequence,
            view.snapshot(),
        )
    {
        state.stop_requested_snapshot_revision = None;
        state.stop_requested_after_sequence = 0;
        state.role_turn_completed = true;
        state.acknowledge(accepted);
        return;
    }
    if state.pending.is_some() {
        return;
    }
    let Ok(command) = read_command(&state) else {
        state.terminal = TerminalState::Failed;
        return;
    };
    let Some(command) = command else { return };
    match revision_decision(state.revision, state.acknowledged, &command) {
        RevisionDecision::IgnoreAcknowledged => return,
        RevisionDecision::Reject => {
            state.terminal = TerminalState::Failed;
            state.failure_category = Some("proof-control");
            return;
        }
        RevisionDecision::AcceptNext => {}
    }
    if command.document.revision != state.revision.saturating_add(1) {
        state.terminal = TerminalState::Failed;
        return;
    }
    let accepted = AcceptedCommand {
        revision: command.document.revision,
        command: command.document.command,
        generation: command.generation,
    };
    match accepted.command {
        Command::Idle => state.acknowledge(accepted),
        Command::PerformRoleTurn => {
            if state.role_turn_completed
                || view.snapshot().phase != ClientPhase::MovementReady
                || session
                    .publish_movement_intent(
                        MovementIntent::planar(1.0, 0.0).expect("finite role intent"),
                    )
                    .is_err()
            {
                state.terminal = TerminalState::Failed;
                return;
            }
            state.pending = Some(accepted);
            state.role_turn_started = Some(Instant::now());
        }
        Command::ShowProjectionSnap => {
            // Presentation acknowledgement only. It does not publish a move
            // or mutate any Realm-observed Remote Avatar fact.
            if state.projection_snap_completed || !remote_avatar.show_scripted_projection_snap() {
                state.terminal = TerminalState::Failed;
                return;
            }
            state.projection_snap = true;
            state.projection_snap_completed = true;
            state.projection_snap_restore_at = Some(Instant::now() + PROJECTION_SNAP_VISIBLE_FOR);
            state.acknowledge(accepted);
        }
        Command::RequestCleanShutdown => {
            if session.send_disconnect().is_err() {
                state.terminal = TerminalState::Failed;
                return;
            }
            state.shutdown_requested = true;
            state.acknowledge(accepted);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RevisionDecision {
    IgnoreAcknowledged,
    AcceptNext,
    Reject,
}

fn revision_decision(
    current: u64,
    acknowledged: Option<AcceptedCommand>,
    command: &CommandEnvelope,
) -> RevisionDecision {
    if acknowledged.is_some_and(|value| {
        value.revision == command.document.revision
            && value.command == command.document.command
            && value.generation == command.generation
    }) {
        RevisionDecision::IgnoreAcknowledged
    } else if command.document.revision == current.saturating_add(1) {
        RevisionDecision::AcceptNext
    } else {
        RevisionDecision::Reject
    }
}

fn has_post_stop_submission(
    events: &[ClientEvent],
    after_sequence: u64,
    snapshot: &client_session::ClientSnapshot,
) -> bool {
    events.iter().any(|event| {
        event.sequence > after_sequence
            && matches!(event.kind, ClientEventKind::MovementSubmitted { pose, stopped: true } if snapshot.submitted_pose == Some(pose))
    })
}

fn read_command(state: &SharedHostProofState) -> Result<Option<CommandEnvelope>, ()> {
    let path = state.command_path();
    // A parent replaces commands atomically.  A replacement racing this small
    // read window is normal, so retry once; a second unstable generation is a
    // fail-closed control-channel fault rather than a torn acceptance.
    for _ in 0..2 {
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) if !metadata.file_type().is_symlink() && metadata.is_file() => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Ok(_) | Err(_) => return Err(()),
        };
        if metadata.len() > 512 {
            return Err(());
        }
        let Ok(text) = fs::read_to_string(&path) else {
            return Err(());
        };
        let metadata_after = fs::symlink_metadata(&path).map_err(|_| ())?;
        if metadata_after.file_type().is_symlink()
            || !metadata_after.is_file()
            || command_generation(&metadata_after) != command_generation(&metadata)
        {
            continue;
        }
        let document = serde_json::from_str(&text).map_err(|_| ())?;
        return Ok(Some(CommandEnvelope {
            document,
            generation: command_generation(&metadata),
        }));
    }
    Err(())
}

fn command_generation(metadata: &fs::Metadata) -> CommandGeneration {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        CommandGeneration {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }
    #[cfg(not(unix))]
    {
        CommandGeneration {
            modified: metadata
                .modified()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
            length: metadata.len(),
        }
    }
}

fn publish_sidecar(
    mut state: ResMut<SharedHostProofState>,
    view: Res<DiagnosticView>,
    proof_ingress: Res<crate::bridge::ProofEventIngress>,
    mut exit: MessageWriter<AppExit>,
) {
    let snapshot = view.snapshot();
    if snapshot.phase == ClientPhase::MovementReady
        && snapshot
            .selected_character
            .as_ref()
            .is_none_or(|character| character.guid() == 0)
    {
        state.terminal = TerminalState::Failed;
        state.failure_category = Some("proof-control");
    }
    accumulate_events(&mut state, &proof_ingress.0);
    let terminal = Terminal {
        state: state.terminal.clone(),
        failure_category: (state.terminal == TerminalState::Failed)
            .then_some(state.failure_category.unwrap_or("proof-control")),
        acknowledged_revision: state.acknowledged.map(|accepted| accepted.revision),
        command_result: state.acknowledged.map(|accepted| accepted.command),
        projection_snap_acknowledged: state.projection_snap,
    };
    let sidecar = Sidecar {
        schema: "miazcore.shared-host-replication-sidecar.v1",
        attempt_id: &state.config.attempt_id,
        profile: state.config.profile,
        guid: snapshot
            .selected_character
            .as_ref()
            .filter(|character| character.guid() != 0)
            .map(|character| format!("{:016x}", character.guid())),
        entry_anchor: snapshot.entry_anchor.map(Pose::from),
        movement_ready: snapshot.phase == ClientPhase::MovementReady,
        events: state.event_history.clone(),
        terminal,
    };
    let serialized = serde_json::to_string_pretty(&sidecar).expect("sidecar is serializable");
    if state.last_sidecar.as_ref() != Some(&serialized) {
        if atomic_write(&state.sidecar_path(), serialized.as_bytes()).is_err() {
            state.terminal = TerminalState::Failed;
            exit.write(AppExit::error());
            return;
        }
        state.last_sidecar = Some(serialized);
    }
    if let Some(accepted) = state.acknowledged
        && state.frozen_revision != Some(accepted.revision)
        && (accepted.command != Command::RequestCleanShutdown
            || state.terminal == TerminalState::Offline)
    {
        let Some(serialized) = state.last_sidecar.as_deref() else {
            state.terminal = TerminalState::Failed;
            state.failure_category = Some("proof-sidecar");
            exit.write(AppExit::error());
            return;
        };
        if atomic_create(
            &state.frozen_sidecar_path(accepted.revision),
            serialized.as_bytes(),
        )
        .is_err()
        {
            state.terminal = TerminalState::Failed;
            state.failure_category = Some("proof-sidecar");
            exit.write(AppExit::error());
            return;
        }
        state.frozen_revision = Some(accepted.revision);
    }
    if state.terminal != TerminalState::Active {
        exit.write(if state.terminal == TerminalState::Offline {
            AppExit::Success
        } else {
            AppExit::error()
        });
    }
}

fn accumulate_events(state: &mut SharedHostProofState, events: &[ClientEvent]) {
    for event in events {
        if event.sequence <= state.last_event_sequence {
            continue;
        }
        state.last_event_sequence = event.sequence;
        let evidence = match event.kind {
            ClientEventKind::RemoteAvatar { change } => match change {
                RemoteAvatarChange::Created {
                    id,
                    realm_observed_pose,
                } => remote_event(
                    event.sequence,
                    EventKind::RemoteCreated,
                    id,
                    Some(realm_observed_pose),
                    None,
                ),
                RemoteAvatarChange::Updated {
                    id,
                    realm_observed_pose,
                } => remote_event(
                    event.sequence,
                    EventKind::RemoteUpdated,
                    id,
                    Some(realm_observed_pose),
                    None,
                ),
                RemoteAvatarChange::Removed { id, .. } => {
                    remote_event(event.sequence, EventKind::RemoteRemoved, id, None, None)
                }
                RemoteAvatarChange::Faulted { id, category } => remote_event(
                    event.sequence,
                    EventKind::RemoteFaulted,
                    id,
                    None,
                    Some(match category {
                        client_session::RemoteAvatarFaultCategory::InvalidPose => "invalid-pose",
                        client_session::RemoteAvatarFaultCategory::UnsupportedMovement => {
                            "unsupported-movement"
                        }
                        client_session::RemoteAvatarFaultCategory::InconsistentLifecycle => {
                            "inconsistent-lifecycle"
                        }
                    }),
                ),
            },
            ClientEventKind::MovementSubmitted { pose, stopped } => EventEvidence {
                sequence: event.sequence,
                kind: EventKind::MovementSubmitted,
                remote: None,
                submitted: Some(SubmittedEvidence {
                    pose: Pose::from(pose),
                    stopped,
                }),
            },
            _ => continue,
        };
        state.event_history.push(evidence);
    }
}

fn remote_event(
    sequence: u64,
    kind: EventKind,
    id: client_session::RemoteAvatarId,
    pose: Option<client_session::WorldPose>,
    fault_category: Option<&'static str>,
) -> EventEvidence {
    EventEvidence {
        sequence,
        kind,
        remote: Some(RemoteEvidence {
            guid: id.display_shorthand(),
            realm_observed_pose: pose.map(Pose::from),
            fault_category,
        }),
        submitted: None,
    }
}

fn atomic_write(path: &Path, content: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().expect("sidecar has a parent");
    let temporary = parent.join(format!(".{SIDECAR_FILE}.{}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(content)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    fs::rename(temporary, path)
}

fn atomic_create(path: &Path, content: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().expect("sidecar has a parent");
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("sidecar");
    let temporary = parent.join(format!(".{name}.{}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(content)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    fs::hard_link(&temporary, path)?;
    fs::remove_file(temporary)
}

fn valid_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{
        Command, CommandDocument, EventEvidence, EventKind, Pose, SharedHostProofConfig, Sidecar,
        Terminal, TerminalState, atomic_create, atomic_write, read_command,
    };

    #[test]
    fn post_stop_acknowledgement_requires_a_new_matching_submitted_event() {
        let identity = client_session::SanitizedIdentity::new(1, "Realm", "Local", 12_340).unwrap();
        let pose = client_session::WorldPose::origin(0);
        let mut snapshot = client_session::ClientSnapshot::offline(identity);
        snapshot.submitted_pose = Some(pose);
        snapshot.submitted_pose_is_stopped = true;
        let matching = client_session::ClientEvent {
            sequence: 8,
            kind: client_session::ClientEventKind::MovementSubmitted {
                pose,
                stopped: true,
            },
        };
        assert!(!super::has_post_stop_submission(
            std::slice::from_ref(&matching),
            8,
            &snapshot
        ));
        assert!(super::has_post_stop_submission(&[matching], 7, &snapshot));
    }

    #[test]
    fn command_document_is_closed_and_uses_only_admitted_commands() {
        let command: CommandDocument =
            serde_json::from_str(r#"{"revision":1,"command":"perform-role-turn"}"#).unwrap();
        assert_eq!(command.revision, 1);
        assert_eq!(command.command, Command::PerformRoleTurn);
        assert!(
            serde_json::from_str::<CommandDocument>(
                r#"{"revision":1,"command":"perform-role-turn","endpoint":"x"}"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<CommandDocument>(r#"{"revision":1,"command":"move-anywhere"}"#)
                .is_err()
        );
    }

    #[test]
    fn proof_directory_must_be_precreated_under_scratch_and_profile_bound() {
        let root = temporary_directory("proof-admission");
        let pair_a = root.join(".scratch/run-1/pair-a");
        fs::create_dir_all(&pair_a).unwrap();
        restrict_directory(&pair_a);
        write_admission(&pair_a, "pair-a");
        assert!(SharedHostProofConfig::admit(&root, "pair-a", pair_a).is_ok());
        let pair_b = root.join(".scratch/run-1/pair-b");
        fs::create_dir_all(&pair_b).unwrap();
        restrict_directory(&pair_b);
        write_admission(&pair_b, "pair-b");
        assert!(SharedHostProofConfig::admit(&root, "pair-a", pair_b).is_err());
    }

    #[test]
    fn sidecar_publish_is_atomic_and_leaves_no_temp_sibling() {
        let directory = temporary_directory("atomic-sidecar");
        let sidecar = directory.join("sidecar.json");
        atomic_write(&sidecar, b"{}").unwrap();
        assert_eq!(fs::read_to_string(&sidecar).unwrap(), "{}\n");
        assert!(
            !directory
                .join(format!(".sidecar.json.{}.tmp", std::process::id()))
                .exists()
        );
    }

    #[test]
    fn acknowledged_revision_snapshot_is_created_once_and_never_overwritten() {
        let directory = temporary_directory("frozen-sidecar");
        let snapshot = directory.join("sidecar.revision-1.json");
        atomic_create(&snapshot, b"first").unwrap();
        assert_eq!(fs::read_to_string(&snapshot).unwrap(), "first\n");
        assert!(atomic_create(&snapshot, b"replacement").is_err());
        assert_eq!(fs::read_to_string(&snapshot).unwrap(), "first\n");
    }

    #[test]
    fn malformed_and_stale_commands_are_never_accepted_as_progress() {
        let root = temporary_directory("command-revision");
        let directory = root.join(".scratch/attempt-1/pair-a");
        fs::create_dir_all(&directory).unwrap();
        restrict_directory(&directory);
        write_admission(&directory, "pair-a");
        let config = SharedHostProofConfig::admit(&root, "pair-a", directory.clone()).unwrap();
        fs::write(directory.join("command.json"), "{not json").unwrap();
        assert!(read_command(&super::SharedHostProofState::new(config.clone())).is_err());
        fs::write(
            directory.join("command.json"),
            r#"{"revision":0,"command":"idle"}"#,
        )
        .unwrap();
        let command = read_command(&super::SharedHostProofState::new(config))
            .unwrap()
            .unwrap();
        assert_eq!(command.document.revision, 0);
    }

    #[test]
    fn command_generation_distinguishes_unchanged_acknowledgement_from_rewritten_duplicate() {
        let root = temporary_directory("command-generation");
        let directory = root.join(".scratch/attempt-1/pair-a");
        fs::create_dir_all(&directory).unwrap();
        restrict_directory(&directory);
        write_admission(&directory, "pair-a");
        let config = SharedHostProofConfig::admit(&root, "pair-a", directory.clone()).unwrap();
        let state = super::SharedHostProofState::new(config);
        fs::write(
            directory.join("command.json"),
            r#"{"revision":1,"command":"idle"}"#,
        )
        .unwrap();
        let first = read_command(&state).unwrap().unwrap();
        let acknowledged = super::AcceptedCommand {
            revision: 1,
            command: Command::Idle,
            generation: first.generation,
        };
        assert_eq!(
            super::revision_decision(1, Some(acknowledged), &first),
            super::RevisionDecision::IgnoreAcknowledged
        );
        let replacement = directory.join("replacement.json");
        fs::write(&replacement, r#"{"revision":1,"command":"idle"}"#).unwrap();
        fs::rename(replacement, directory.join("command.json")).unwrap();
        let duplicate = read_command(&state).unwrap().unwrap();
        assert_eq!(
            super::revision_decision(1, Some(acknowledged), &duplicate),
            super::RevisionDecision::Reject
        );
    }

    #[test]
    fn sidecar_vocabulary_excludes_credentials_and_local_character_names() {
        let sidecar = Sidecar {
            schema: "miazcore.shared-host-replication-sidecar.v1",
            attempt_id: "attempt-1",
            profile: "pair-a",
            guid: Some("0000000000000001".to_owned()),
            entry_anchor: Some(Pose {
                map_id: 0,
                east: 1.0,
                north: 2.0,
                elevation: 3.0,
                orientation: 0.0,
            }),
            movement_ready: true,
            events: vec![EventEvidence {
                sequence: 1,
                kind: EventKind::RemoteCreated,
                remote: None,
                submitted: None,
            }],
            terminal: Terminal {
                state: TerminalState::Active,
                failure_category: None,
                acknowledged_revision: Some(1),
                command_result: Some(Command::Idle),
                projection_snap_acknowledged: false,
            },
        };
        let text = serde_json::to_string(&sidecar).unwrap();
        for forbidden in ["password", "credential", "Miazpaira", "endpoint"] {
            assert!(!text.contains(forbidden));
        }
    }

    fn temporary_directory(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("miazcore-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn write_admission(path: &std::path::Path, profile: &str) {
        fs::write(
            path.join("admission.json"),
            format!(
                r#"{{"schema":"miazcore.shared-host-proof-admission.v1","attempt_id":"attempt-1","profile":"{profile}"}}"#
            ),
        )
        .unwrap();
    }

    #[cfg(unix)]
    fn restrict_directory(path: &std::path::Path) {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }

    #[cfg(not(unix))]
    fn restrict_directory(_: &std::path::Path) {}
}
