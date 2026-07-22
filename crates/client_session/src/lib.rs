//! Engine-independent application/session boundary for the Learning Client.

mod api;
mod boundary;
mod character;
mod config;
mod headless;
mod machine;
mod offline;
mod realm;
mod runtime;

pub use api::{
    ClientEvent, ClientEventKind, ClientFailure, ClientPhase, ClientSnapshot, CommandKind,
    ControlCommand, DiscoveredRealm, EntryStage, FailureCategory, IdentityError, MovementIntent,
    MovementIntentError, PoseSource, ProofStage, QueueCounters, Recovery, RecoveryAction,
    SanitizedIdentity, SanitizedText, SelectedCharacter, SemanticDiagnostic, WorldPose,
};
pub use boundary::BoundaryError;
pub use character::{CharacterSelectionEvidence, CharacterSelectionSession};
pub use config::{
    ClientConfig, ClientConfigSpec, ConfigError, CredentialFileKind, CredentialFileProblem,
    CredentialPaths, LoadedClientConfig,
};
pub use offline::OfflineSession;
pub use realm::{RealmDiscoveryEvidence, RealmDiscoverySession};

/// Lossless semantic control operations wait in a bounded FIFO of this size.
pub const CONTROL_CAPACITY: usize = 16;

/// Lossless semantic events wait in a bounded FIFO of this size.
pub const EVENT_CAPACITY: usize = 64;
