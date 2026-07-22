//! Engine-independent application/session boundary for the Learning Client.

mod api;
mod config;
mod offline;

pub use api::{
    ClientEvent, ClientEventKind, ClientFailure, ClientPhase, ClientSnapshot, CommandKind,
    ControlCommand, EntryStage, FailureCategory, IdentityError, MovementIntent,
    MovementIntentError, PoseSource, ProofStage, QueueCounters, Recovery, RecoveryAction,
    SanitizedIdentity, SanitizedText, SemanticDiagnostic, WorldPose,
};
pub use config::{
    ClientConfig, ClientConfigSpec, ConfigError, CredentialFileKind, CredentialFileProblem,
    CredentialPaths, LoadedClientConfig,
};
pub use offline::{BoundaryError, OfflineSession};

/// Lossless semantic control operations wait in a bounded FIFO of this size.
pub const CONTROL_CAPACITY: usize = 16;

/// Lossless semantic events wait in a bounded FIFO of this size.
pub const EVENT_CAPACITY: usize = 64;
