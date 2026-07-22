use std::{
    error::Error,
    ffi::OsString,
    fmt,
    path::{Path, PathBuf},
    time::Duration,
};

use bevy::{prelude::*, time::TimeUpdateStrategy, window::WindowPlugin};
use client_bevy::{LearningClientPlugin, RenderProofPlugin, SessionBridge};
use client_session::{ClientConfig, LiveDiagnosticSession, OfflineSession};

fn main() {
    if let Err(error) = run() {
        eprintln!("learning client startup failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), StartupError> {
    let arguments = Arguments::parse(std::env::args_os().skip(1))?;
    let repository_root = std::env::current_dir().map_err(|_| StartupError::WorkingDirectory)?;
    ensure_repository_root(&repository_root)?;

    // Configuration and credentials are fully validated before Bevy or a session is constructed.
    let loaded = ClientConfig::reference_realm(&repository_root)?.load()?;
    let session = if arguments.offline || arguments.proof_output.is_some() {
        SessionBridge::new(OfflineSession::start(loaded)?)
    } else {
        SessionBridge::new(LiveDiagnosticSession::start(loaded)?)
    };

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Miazcore — Diagnostic World".into(),
            resolution: (1280, 720).into(),
            ..default()
        }),
        ..default()
    }))
    .insert_resource(session)
    .add_plugins(LearningClientPlugin::windowed());

    if let Some(output) = arguments.proof_output {
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / 60.0,
        )))
        .add_plugins(RenderProofPlugin::new(output));
    } else if let Some(output) = arguments.live_proof_output {
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / 60.0,
        )))
        .add_plugins(RenderProofPlugin::live_entry(output));
    }

    app.run();
    Ok(())
}

#[derive(Debug, Default, Eq, PartialEq)]
struct Arguments {
    proof_output: Option<PathBuf>,
    live_proof_output: Option<PathBuf>,
    offline: bool,
}

impl Arguments {
    fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Self, StartupError> {
        let mut arguments = arguments.into_iter();
        let mut parsed = Self::default();
        while let Some(argument) = arguments.next() {
            if argument == "--proof-output" {
                if parsed.proof_output.is_some() || parsed.live_proof_output.is_some() {
                    return Err(StartupError::DuplicateProofOutput);
                }
                parsed.proof_output = Some(
                    arguments
                        .next()
                        .map(PathBuf::from)
                        .ok_or(StartupError::MissingProofOutput)?,
                );
            } else if argument == "--live-proof-output" {
                if parsed.proof_output.is_some() || parsed.live_proof_output.is_some() {
                    return Err(StartupError::DuplicateProofOutput);
                }
                parsed.live_proof_output = Some(
                    arguments
                        .next()
                        .map(PathBuf::from)
                        .ok_or(StartupError::MissingProofOutput)?,
                );
            } else if argument == "--offline" {
                if parsed.offline {
                    return Err(StartupError::DuplicateOfflineMode);
                }
                parsed.offline = true;
            } else {
                return Err(StartupError::UnsupportedArgument);
            }
        }
        if parsed.offline && parsed.live_proof_output.is_some() {
            return Err(StartupError::OfflineLiveConflict);
        }
        Ok(parsed)
    }
}

#[derive(Debug)]
enum StartupError {
    UnsupportedArgument,
    MissingProofOutput,
    DuplicateProofOutput,
    DuplicateOfflineMode,
    OfflineLiveConflict,
    WorkingDirectory,
    NotRepositoryRoot,
    Configuration(client_session::ConfigError),
    Boundary(client_session::BoundaryError),
}

impl fmt::Display for StartupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedArgument => formatter.write_str("unsupported command-line argument"),
            Self::MissingProofOutput => {
                formatter.write_str("--proof-output requires a non-secret path")
            }
            Self::DuplicateProofOutput => {
                formatter.write_str("only one render-proof output may be supplied")
            }
            Self::DuplicateOfflineMode => {
                formatter.write_str("--offline may be supplied only once")
            }
            Self::OfflineLiveConflict => {
                formatter.write_str("--offline cannot be combined with a live render proof")
            }
            Self::WorkingDirectory => formatter.write_str("working directory is unavailable"),
            Self::NotRepositoryRoot => {
                formatter.write_str("run the Learning Client from the repository root")
            }
            Self::Configuration(error) => write!(formatter, "configuration: {error}"),
            Self::Boundary(error) => write!(formatter, "session: {error}"),
        }
    }
}

impl Error for StartupError {}

impl From<client_session::ConfigError> for StartupError {
    fn from(error: client_session::ConfigError) -> Self {
        Self::Configuration(error)
    }
}

impl From<client_session::BoundaryError> for StartupError {
    fn from(error: client_session::BoundaryError) -> Self {
        Self::Boundary(error)
    }
}

fn ensure_repository_root(path: &Path) -> Result<(), StartupError> {
    if path.join("Cargo.toml").is_file() && path.join("infra/azerothcore/realm").is_file() {
        Ok(())
    } else {
        Err(StartupError::NotRepositoryRoot)
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::{Arguments, StartupError};

    #[test]
    fn only_non_secret_render_proof_output_is_accepted() {
        assert_eq!(
            Arguments::parse([
                OsString::from("--proof-output"),
                OsString::from("artifacts/offline.png"),
            ])
            .unwrap(),
            Arguments {
                proof_output: Some("artifacts/offline.png".into()),
                live_proof_output: None,
                offline: false,
            }
        );
        assert_eq!(
            Arguments::parse([
                OsString::from("--live-proof-output"),
                OsString::from("artifacts/live.png"),
            ])
            .unwrap(),
            Arguments {
                proof_output: None,
                live_proof_output: Some("artifacts/live.png".into()),
                offline: false,
            }
        );
        assert!(matches!(
            Arguments::parse([
                OsString::from("--offline"),
                OsString::from("--live-proof-output"),
                OsString::from("artifacts/live.png"),
            ]),
            Err(StartupError::OfflineLiveConflict)
        ));
        assert!(matches!(
            Arguments::parse([OsString::from("--password")]),
            Err(StartupError::UnsupportedArgument)
        ));
    }

    #[test]
    fn unsupported_argument_error_never_echoes_argument_content() {
        let secret = "do-not-log-this-secret";
        let error = Arguments::parse([OsString::from(secret)]).unwrap_err();
        assert!(!error.to_string().contains(secret));
    }
}
