use std::{error::Error, fmt, net::SocketAddr};

/// A scalar pose expressed in the Learning Client's engine-independent world vocabulary.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WorldPose {
    pub map_id: u32,
    pub east: f32,
    pub north: f32,
    pub elevation: f32,
    pub orientation: f32,
}

impl WorldPose {
    #[must_use]
    pub const fn origin(map_id: u32) -> Self {
        Self {
            map_id,
            east: 0.0,
            north: 0.0,
            elevation: 0.0,
            orientation: 0.0,
        }
    }
}

/// Validation failure for a value allowed to cross the sanitized application boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityError {
    Empty,
    TooLong,
    ControlCharacter,
}

impl fmt::Display for IdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("value must not be empty"),
            Self::TooLong => formatter.write_str("value exceeds the diagnostic length limit"),
            Self::ControlCharacter => {
                formatter.write_str("value contains a forbidden control character")
            }
        }
    }
}

impl Error for IdentityError {}

/// Text that has passed the boundary's diagnostic-safety checks.
#[derive(Clone, Eq, Hash, PartialEq)]
pub struct SanitizedText(String);

impl SanitizedText {
    const MAX_LEN: usize = 160;

    /// Validate text before it crosses the application boundary.
    ///
    /// # Errors
    ///
    /// Returns an error when the value is empty, too long, or contains control characters.
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, IdentityError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(IdentityError::Empty);
        }
        if value.chars().count() > Self::MAX_LEN {
            return Err(IdentityError::TooLong);
        }
        if value.chars().any(char::is_control) {
            return Err(IdentityError::ControlCharacter);
        }
        Ok(Self(value))
    }

    pub(crate) fn from_static(value: &'static str) -> Self {
        Self(value.to_owned())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SanitizedText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("SanitizedText")
            .field(&self.0)
            .finish()
    }
}

impl fmt::Display for SanitizedText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Non-secret identity projected into Bevy and diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SanitizedIdentity {
    realm_id: u32,
    realm_name: SanitizedText,
    character_name: SanitizedText,
    client_build: u16,
}

impl SanitizedIdentity {
    /// Construct the non-secret identity projected into diagnostics.
    ///
    /// # Errors
    ///
    /// Returns an error when either displayed name fails diagnostic sanitization.
    pub fn new(
        realm_id: u32,
        realm_name: impl Into<String>,
        character_name: impl Into<String>,
        client_build: u16,
    ) -> Result<Self, IdentityError> {
        Ok(Self {
            realm_id,
            realm_name: SanitizedText::new(realm_name)?,
            character_name: SanitizedText::new(character_name)?,
            client_build,
        })
    }

    #[must_use]
    pub const fn realm_id(&self) -> u32 {
        self.realm_id
    }

    #[must_use]
    pub fn realm_name(&self) -> &str {
        self.realm_name.as_str()
    }

    #[must_use]
    pub fn character_name(&self) -> &str {
        self.character_name.as_str()
    }

    #[must_use]
    pub const fn client_build(&self) -> u16 {
        self.client_build
    }
}

/// Sanitized result of authenticated realm discovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredRealm {
    realm_id: u32,
    realm_name: SanitizedText,
    client_build: u16,
    endpoint: SocketAddr,
}

impl DiscoveredRealm {
    pub(crate) fn new(
        realm_id: u32,
        realm_name: impl Into<String>,
        client_build: u16,
        endpoint: SocketAddr,
    ) -> Result<Self, IdentityError> {
        Ok(Self {
            realm_id,
            realm_name: SanitizedText::new(realm_name)?,
            client_build,
            endpoint,
        })
    }

    #[must_use]
    pub const fn realm_id(&self) -> u32 {
        self.realm_id
    }

    #[must_use]
    pub fn realm_name(&self) -> &str {
        self.realm_name.as_str()
    }

    #[must_use]
    pub const fn client_build(&self) -> u16 {
        self.client_build
    }

    #[must_use]
    pub const fn endpoint(&self) -> SocketAddr {
        self.endpoint
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryStage {
    LoginConnection,
    LoginAuthentication,
    RealmSelection,
    WorldAuthentication,
    CharacterSelection,
    Bootstrap,
    ControlSynchronization,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProofStage {
    SavingLogout,
    WaitingOffline,
    Reconnecting,
    Comparing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureCategory {
    Configuration,
    Authentication,
    Transport,
    ProtocolIncompatibility,
    UnsupportedSelfControl,
    Timeout,
    InternalBackpressure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryAction {
    FixConfiguration,
    CheckCredentials,
    CheckReferenceRealm,
    RetryExplicitly,
    RestartClient,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Recovery {
    pub category: FailureCategory,
    pub action: RecoveryAction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClientPhase {
    Offline,
    Entering(EntryStage),
    MovementReady,
    ProvingMovement(ProofStage),
    Failed(Recovery),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandKind {
    StartEntry,
    BeginMovementProof,
    Disconnect,
    RetryEntry,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlCommand {
    StartEntry,
    BeginMovementProof,
    Disconnect,
    RetryEntry,
}

impl ControlCommand {
    #[must_use]
    pub const fn kind(self) -> CommandKind {
        match self {
            Self::StartEntry => CommandKind::StartEntry,
            Self::BeginMovementProof => CommandKind::BeginMovementProof,
            Self::Disconnect => CommandKind::Disconnect,
            Self::RetryEntry => CommandKind::RetryEntry,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MovementIntentError {
    NonFinite,
}

impl fmt::Display for MovementIntentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("movement intent must contain finite values")
    }
}

impl Error for MovementIntentError {}

/// Latest replaceable planar movement intent in world coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MovementIntent {
    pub east: f32,
    pub north: f32,
    pub engaged: bool,
}

impl MovementIntent {
    #[must_use]
    pub const fn idle() -> Self {
        Self {
            east: 0.0,
            north: 0.0,
            engaged: false,
        }
    }

    /// Normalize a finite planar direction without allowing its magnitude to exceed one.
    ///
    /// # Errors
    ///
    /// Returns an error when either component is not finite.
    pub fn planar(east: f32, north: f32) -> Result<Self, MovementIntentError> {
        if !east.is_finite() || !north.is_finite() {
            return Err(MovementIntentError::NonFinite);
        }
        let length = east.hypot(north);
        if length <= f32::EPSILON {
            return Ok(Self::idle());
        }
        let scale = length.max(1.0);
        Ok(Self {
            east: east / scale,
            north: north / scale,
            engaged: true,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientFailure {
    category: FailureCategory,
    stage: SanitizedText,
    context: SanitizedText,
    recommended_recovery: RecoveryAction,
}

impl ClientFailure {
    pub(crate) fn new(
        category: FailureCategory,
        stage: &'static str,
        context: &'static str,
        recommended_recovery: RecoveryAction,
    ) -> Self {
        Self {
            category,
            stage: SanitizedText::from_static(stage),
            context: SanitizedText::from_static(context),
            recommended_recovery,
        }
    }

    #[must_use]
    pub const fn category(&self) -> FailureCategory {
        self.category
    }

    #[must_use]
    pub fn stage(&self) -> &str {
        self.stage.as_str()
    }

    #[must_use]
    pub fn context(&self) -> &str {
        self.context.as_str()
    }

    #[must_use]
    pub const fn recommended_recovery(&self) -> RecoveryAction {
        self.recommended_recovery
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PoseSource {
    EntryObservation,
    MovementWrite,
    ReconnectObservation,
    Correction,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ClientEventKind {
    PhaseChanged {
        phase: ClientPhase,
    },
    IdentityConfigured {
        identity: SanitizedIdentity,
    },
    RealmDiscovered {
        realm: DiscoveredRealm,
    },
    PoseObserved {
        source: PoseSource,
        pose: WorldPose,
    },
    MovementSubmitted {
        pose: WorldPose,
    },
    CommandRejected {
        command: CommandKind,
        failure: ClientFailure,
    },
    Disconnected,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClientEvent {
    pub sequence: u64,
    pub kind: ClientEventKind,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct QueueCounters {
    pub control_queued: usize,
    pub event_queued: usize,
    pub movement_revision: u64,
    pub snapshot_revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticDiagnostic {
    sequence: u64,
    message: SanitizedText,
}

impl SemanticDiagnostic {
    pub(crate) fn new(sequence: u64, message: &'static str) -> Self {
        Self {
            sequence,
            message: SanitizedText::from_static(message),
        }
    }

    pub(crate) fn from_failure(sequence: u64, failure: &ClientFailure) -> Self {
        Self {
            sequence,
            message: failure.context.clone(),
        }
    }

    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    #[must_use]
    pub fn message(&self) -> &str {
        self.message.as_str()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClientSnapshot {
    pub identity: SanitizedIdentity,
    pub phase: ClientPhase,
    pub discovered_realm: Option<DiscoveredRealm>,
    pub entry_anchor: Option<WorldPose>,
    pub predicted_pose: Option<WorldPose>,
    pub submitted_pose: Option<WorldPose>,
    pub realm_observed_pose: Option<WorldPose>,
    pub run_speed: Option<f32>,
    pub queue_counters: QueueCounters,
    pub latest_failure: Option<ClientFailure>,
    pub diagnostics: Vec<SemanticDiagnostic>,
}

impl ClientSnapshot {
    #[must_use]
    pub fn offline(identity: SanitizedIdentity) -> Self {
        Self {
            identity,
            phase: ClientPhase::Offline,
            discovered_realm: None,
            entry_anchor: None,
            predicted_pose: None,
            submitted_pose: None,
            realm_observed_pose: None,
            run_speed: None,
            queue_counters: QueueCounters::default(),
            latest_failure: None,
            diagnostics: vec![SemanticDiagnostic::new(
                1,
                "Offline presentation source active",
            )],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ClientEvent, ClientEventKind, ClientFailure, ClientPhase, ClientSnapshot, CommandKind,
        ControlCommand, DiscoveredRealm, EntryStage, FailureCategory, IdentityError,
        MovementIntent, MovementIntentError, PoseSource, ProofStage, Recovery, RecoveryAction,
        SanitizedIdentity, SanitizedText, SemanticDiagnostic, WorldPose,
    };
    use crate::{
        BoundaryError, ConfigError, CredentialFileKind, CredentialFileProblem, QueueCounters,
    };

    #[test]
    fn sanitized_text_rejects_empty_long_and_control_values() {
        assert!(SanitizedText::new("   ").is_err());
        assert!(SanitizedText::new("x".repeat(161)).is_err());
        assert!(SanitizedText::new("line\nbreak").is_err());
        assert_eq!(SanitizedText::new("Miaztest").unwrap().as_str(), "Miaztest");
    }

    #[test]
    fn movement_intent_normalizes_and_rejects_non_finite_values() {
        assert_eq!(
            MovementIntent::planar(0.0, 0.0).unwrap(),
            MovementIntent::idle()
        );
        let intent = MovementIntent::planar(3.0, 4.0).unwrap();
        assert!((intent.east - 0.6).abs() < f32::EPSILON);
        assert!((intent.north - 0.8).abs() < f32::EPSILON);
        assert!(intent.engaged);
        assert!(MovementIntent::planar(f32::NAN, 0.0).is_err());
    }

    #[test]
    fn every_public_semantic_format_is_structurally_secret_free() {
        let identity =
            SanitizedIdentity::new(1, "Miazcore Reference Realm", "Miaztest", 12_340).unwrap();
        let pose = WorldPose {
            map_id: 0,
            east: 1.0,
            north: 2.0,
            elevation: 3.0,
            orientation: 4.0,
        };
        let failure = ClientFailure::new(
            FailureCategory::Configuration,
            "offline",
            "network capability deferred",
            RecoveryAction::RestartClient,
        );
        let events = semantic_events(&identity, &failure, pose);
        let discovered = DiscoveredRealm::new(
            1,
            "Miazcore Reference Realm",
            12_340,
            "127.0.0.1:8085".parse().unwrap(),
        )
        .unwrap();
        let snapshot = populated_snapshot(identity.clone(), discovered, failure.clone(), pose);

        let values = [
            format!(
                "{:?}",
                [
                    ControlCommand::StartEntry,
                    ControlCommand::BeginMovementProof,
                    ControlCommand::Disconnect,
                    ControlCommand::RetryEntry,
                ]
            ),
            format!("{events:?}"),
            format!("{snapshot:?}"),
            format!("{:?}", snapshot.diagnostics),
            public_phase_formats(),
            public_error_formats(),
            format!(
                "{} {} {} {} {} {} {} {} {}",
                failure.category() as u8,
                failure.stage(),
                failure.context(),
                failure.recommended_recovery() as u8,
                identity.realm_id(),
                identity.realm_name(),
                identity.character_name(),
                identity.client_build(),
                snapshot.diagnostics[0].message(),
            ),
        ];

        for formatted in values {
            assert!(!formatted.contains("never-print-this-password"));
            assert!(!formatted.contains("never-print-this-session-key"));
        }
    }

    fn semantic_events(
        identity: &SanitizedIdentity,
        failure: &ClientFailure,
        pose: WorldPose,
    ) -> Vec<ClientEvent> {
        [
            ClientEventKind::PhaseChanged {
                phase: ClientPhase::Offline,
            },
            ClientEventKind::IdentityConfigured {
                identity: identity.clone(),
            },
            ClientEventKind::RealmDiscovered {
                realm: DiscoveredRealm::new(
                    1,
                    "Miazcore Reference Realm",
                    12_340,
                    "127.0.0.1:8085".parse().unwrap(),
                )
                .unwrap(),
            },
            ClientEventKind::PoseObserved {
                source: PoseSource::EntryObservation,
                pose,
            },
            ClientEventKind::MovementSubmitted { pose },
            ClientEventKind::CommandRejected {
                command: CommandKind::StartEntry,
                failure: failure.clone(),
            },
            ClientEventKind::Disconnected,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, kind)| ClientEvent {
            sequence: u64::try_from(index).unwrap(),
            kind,
        })
        .collect()
    }

    fn populated_snapshot(
        identity: SanitizedIdentity,
        discovered: DiscoveredRealm,
        failure: ClientFailure,
        pose: WorldPose,
    ) -> ClientSnapshot {
        let mut snapshot = ClientSnapshot::offline(identity);
        snapshot.discovered_realm = Some(discovered);
        snapshot.entry_anchor = Some(pose);
        snapshot.predicted_pose = Some(pose);
        snapshot.submitted_pose = Some(pose);
        snapshot.realm_observed_pose = Some(pose);
        snapshot.run_speed = Some(7.0);
        snapshot.queue_counters = QueueCounters {
            control_queued: 1,
            event_queued: 2,
            movement_revision: 3,
            snapshot_revision: 4,
        };
        snapshot.latest_failure = Some(failure);
        snapshot
            .diagnostics
            .push(SemanticDiagnostic::new(2, "safe semantic diagnostic"));
        snapshot
    }

    fn public_phase_formats() -> String {
        format!(
            "{:?}",
            [
                ClientPhase::Offline,
                ClientPhase::Entering(EntryStage::LoginConnection),
                ClientPhase::MovementReady,
                ClientPhase::ProvingMovement(ProofStage::SavingLogout),
                ClientPhase::Failed(Recovery {
                    category: FailureCategory::InternalBackpressure,
                    action: RecoveryAction::RestartClient,
                }),
            ]
        )
    }

    fn public_error_formats() -> String {
        format!(
            "{} {} {} {} {} {} {} {} {} {} {} {} {} {}",
            IdentityError::Empty,
            IdentityError::TooLong,
            IdentityError::ControlCharacter,
            MovementIntentError::NonFinite,
            BoundaryError::ControlBackpressure,
            BoundaryError::EventBackpressure,
            BoundaryError::WorkerStopped,
            BoundaryError::WorkerPanicked,
            ConfigError::InvalidIdentity(IdentityError::Empty),
            ConfigError::UnsupportedBuild {
                configured: 1,
                required: 12_340,
            },
            ConfigError::NonLoopbackEndpoint,
            ConfigError::InvalidTimeout,
            ConfigError::DuplicateCredentialPath,
            ConfigError::CredentialFile {
                kind: CredentialFileKind::Password,
                problem: CredentialFileProblem::InvalidCharacters,
            },
        )
    }
}
