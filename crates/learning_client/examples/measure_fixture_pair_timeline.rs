use std::{
    error::Error,
    fs, io,
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

use client_session::{ClientPhase, ControlCommand, LiveDiagnosticSession, ProofStage, WorldPose};

#[path = "../src/fixture_profile.rs"]
mod fixture_profile;
use fixture_profile::FixtureProfile;

const READY_DEADLINE: Duration = Duration::from_secs(45);
const STOP_DEADLINE: Duration = Duration::from_secs(8);
const LOGOUT_COMPLETION_DEADLINE: Duration = Duration::from_secs(25);
const POST_LOGOUT_OBSERVATION_WINDOW: Duration = Duration::from_secs(1);

/// Runs one real, serial Pair A/B turn while one controller clock timestamps
/// both local commands and the observer's semantic remote-frame transcript.
///
/// The transcript is deliberately persisted only after the observer exits; it
/// contains no raw World traffic, credentials, session material, or names.
fn main() -> Result<(), Box<dyn Error>> {
    let Arguments {
        transcript_file,
        result_file,
    } = Arguments::parse(std::env::args_os().skip(1))?;
    let repository_root = std::env::current_dir()?;
    let started_at = Instant::now();

    let observer = LiveDiagnosticSession::start_with_remote_trace(
        load_profile(&repository_root, FixtureProfile::PairA)?,
        transcript_file.clone(),
        started_at,
    )?;
    observer.send_control(ControlCommand::StartEntry)?;
    let observer_snapshot = wait_for_ready(&observer, "pair-a observer")?;
    let observer_ready_after_ms = elapsed_ms(started_at);

    let mover =
        LiveDiagnosticSession::start(load_profile(&repository_root, FixtureProfile::PairB)?)?;
    mover.send_control(ControlCommand::StartEntry)?;
    let mover_snapshot = wait_for_ready(&mover, "pair-b mover")?;
    let mover_ready_after_ms = elapsed_ms(started_at);

    let observer_character = selected(&observer_snapshot, "pair-a observer")?;
    let mover_character = selected(&mover_snapshot, "pair-b mover")?;
    let observer_anchor = anchor(&observer_snapshot, "pair-a observer")?;
    let mover_anchor = anchor(&mover_snapshot, "pair-b mover")?;
    validate_pair_start(observer_anchor, mover_anchor)?;

    let move_start_after_ms = elapsed_ms(started_at);
    mover.send_control(ControlCommand::ScriptedMovementProofStart)?;
    let submitted_stop = wait_for_stopped_submission(&mover, mover_anchor)?;
    let move_stop_after_ms = elapsed_ms(started_at);

    let logout_requested_after_ms = elapsed_ms(started_at);
    mover.send_control(ControlCommand::BeginMovementProof)?;
    wait_for_saving_logout_complete(&mover)?;
    // The retained observer, not the controlled client's later reconnect
    // comparison, proves remote removal. Keep it alive long enough to consume
    // a just-completed logout's remaining complete World frames.
    thread::sleep(POST_LOGOUT_OBSERVATION_WINDOW);
    let logout_observation_window_after_ms = elapsed_ms(started_at);
    mover.shutdown()?;
    observer.shutdown()?;

    if !transcript_file.is_file() {
        return Err(io::Error::other("shared-clock observer transcript was not written").into());
    }
    write_result(
        result_file,
        observer_character.guid(),
        mover_character.guid(),
        observer_anchor,
        mover_anchor,
        submitted_stop,
        Timeline {
            observer_ready_after_ms,
            mover_ready_after_ms,
            move_start_after_ms,
            move_stop_after_ms,
            logout_requested_after_ms,
            logout_observation_window_after_ms,
        },
    )?;
    Ok(())
}

fn load_profile(
    repository_root: &std::path::Path,
    profile: FixtureProfile,
) -> Result<client_session::LoadedClientConfig, Box<dyn Error>> {
    Ok(fixture_profile::configuration(repository_root, profile)?.load()?)
}

fn selected(
    snapshot: &client_session::ClientSnapshot,
    label: &str,
) -> Result<client_session::SelectedCharacter, io::Error> {
    snapshot
        .selected_character
        .clone()
        .ok_or_else(|| io::Error::other(format!("{label} has no selected Character")))
}

fn anchor(snapshot: &client_session::ClientSnapshot, label: &str) -> Result<WorldPose, io::Error> {
    snapshot
        .entry_anchor
        .ok_or_else(|| io::Error::other(format!("{label} has no Entry Anchor")))
}

fn validate_pair_start(observer: WorldPose, mover: WorldPose) -> Result<(), io::Error> {
    let valid = observer.map_id == mover.map_id
        && (mover.east - observer.east - 3.0).abs() <= 0.001
        && (mover.north - observer.north).abs() <= 0.001
        && (mover.elevation - observer.elevation).abs() <= 0.001
        && (mover.orientation - observer.orientation).abs() <= 0.001;
    valid.then_some(()).ok_or_else(|| {
        io::Error::other("Fixture Pair Entry Anchors do not match placement contract")
    })
}

fn wait_for_ready(
    session: &LiveDiagnosticSession,
    label: &str,
) -> Result<client_session::ClientSnapshot, Box<dyn Error>> {
    let deadline = Instant::now() + READY_DEADLINE;
    loop {
        let snapshot = session.snapshot();
        if let Some(failure) = snapshot.latest_failure.as_ref() {
            return Err(io::Error::other(format!(
                "{label} failed at {}: {}",
                failure.stage(),
                failure.context(),
            ))
            .into());
        }
        if snapshot.phase == ClientPhase::MovementReady
            && snapshot.selected_character.is_some()
            && snapshot.entry_anchor.is_some()
            && snapshot.run_speed.is_some()
        {
            return Ok(snapshot);
        }
        if Instant::now() >= deadline {
            return Err(io::Error::other(format!("{label} did not reach MovementReady")).into());
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for_stopped_submission(
    session: &LiveDiagnosticSession,
    anchor: WorldPose,
) -> Result<WorldPose, Box<dyn Error>> {
    let deadline = Instant::now() + STOP_DEADLINE;
    loop {
        let snapshot = session.snapshot();
        if let Some(failure) = snapshot.latest_failure.as_ref() {
            return Err(io::Error::other(format!(
                "pair-b mover failed at {}: {}",
                failure.stage(),
                failure.context(),
            ))
            .into());
        }
        if let Some(pose) = snapshot
            .submitted_pose
            .filter(|_| snapshot.submitted_pose_is_stopped)
        {
            let distance = (pose.east - anchor.east).hypot(pose.north - anchor.north);
            if (2.0..=4.0).contains(&distance) {
                return Ok(pose);
            }
            return Err(io::Error::other(
                "scripted Pair move was outside the two-to-four-metre contract",
            )
            .into());
        }
        if Instant::now() >= deadline {
            return Err(io::Error::other("pair-b mover did not submit a stopped pose").into());
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for_saving_logout_complete(session: &LiveDiagnosticSession) -> Result<(), Box<dyn Error>> {
    let deadline = Instant::now() + LOGOUT_COMPLETION_DEADLINE;
    loop {
        let snapshot = session.snapshot();
        if let Some(failure) = snapshot.latest_failure.as_ref() {
            return Err(io::Error::other(format!(
                "pair-b saving logout failed at {}: {}",
                failure.stage(),
                failure.context(),
            ))
            .into());
        }
        if matches!(
            snapshot.phase,
            ClientPhase::ProvingMovement(ProofStage::WaitingOffline)
        ) || session.drain_events().into_iter().any(|event| {
            matches!(
                event.kind,
                client_session::ClientEventKind::PhaseChanged {
                    phase: ClientPhase::ProvingMovement(ProofStage::WaitingOffline)
                }
            )
        }) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(
                io::Error::other("pair-b saving logout did not reach WaitingOffline").into(),
            );
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn elapsed_ms(started_at: Instant) -> u64 {
    started_at
        .elapsed()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[derive(Clone, Copy)]
#[allow(clippy::struct_field_names)] // wire-facing names make the shared clock explicit
struct Timeline {
    observer_ready_after_ms: u64,
    mover_ready_after_ms: u64,
    move_start_after_ms: u64,
    move_stop_after_ms: u64,
    logout_requested_after_ms: u64,
    logout_observation_window_after_ms: u64,
}

#[allow(clippy::too_many_arguments)]
fn write_result(
    result_file: PathBuf,
    observer_guid: u64,
    mover_guid: u64,
    observer_anchor: WorldPose,
    mover_anchor: WorldPose,
    submitted_stop: WorldPose,
    timeline: Timeline,
) -> io::Result<()> {
    let contents = format!(
        concat!(
            "{{\"schema\":\"miazcore.fixture-pair-timeline-run.v1\",",
            "\"observer_guid\":\"{observer_guid:x}\",\"mover_guid\":\"{mover_guid:x}\",",
            "\"map_id\":{},",
            "\"observer_anchor\":{{\"east\":{:.3},\"north\":{:.3},\"elevation\":{:.3},\"orientation\":{:.3}}},",
            "\"mover_anchor\":{{\"east\":{:.3},\"north\":{:.3},\"elevation\":{:.3},\"orientation\":{:.3}}},",
            "\"submitted_stop\":{{\"map_id\":{},\"east\":{:.3},\"north\":{:.3},\"elevation\":{:.3},\"orientation\":{:.3}}},",
            "\"timeline\":{{\"observer_ready_after_ms\":{},\"mover_ready_after_ms\":{},",
            "\"move_start_after_ms\":{},\"move_stop_after_ms\":{},",
            "\"logout_requested_after_ms\":{},\"logout_observation_window_after_ms\":{}}}}}\n"
        ),
        observer_anchor.map_id,
        observer_anchor.east,
        observer_anchor.north,
        observer_anchor.elevation,
        observer_anchor.orientation,
        mover_anchor.east,
        mover_anchor.north,
        mover_anchor.elevation,
        mover_anchor.orientation,
        submitted_stop.map_id,
        submitted_stop.east,
        submitted_stop.north,
        submitted_stop.elevation,
        submitted_stop.orientation,
        timeline.observer_ready_after_ms,
        timeline.mover_ready_after_ms,
        timeline.move_start_after_ms,
        timeline.move_stop_after_ms,
        timeline.logout_requested_after_ms,
        timeline.logout_observation_window_after_ms,
        observer_guid = observer_guid,
        mover_guid = mover_guid,
    );
    fs::write(result_file, contents)
}

struct Arguments {
    transcript_file: PathBuf,
    result_file: PathBuf,
}

impl Arguments {
    fn parse<I, T>(arguments: I) -> io::Result<Self>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString>,
    {
        let values = arguments
            .into_iter()
            .map(Into::into)
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        if values.len() != 2 {
            return Err(io::Error::other(
                "usage: measure_fixture_pair_timeline TRANSCRIPT_FILE RESULT_FILE",
            ));
        }
        Ok(Self {
            transcript_file: values[0].clone(),
            result_file: values[1].clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Arguments, validate_pair_start};
    use client_session::WorldPose;

    #[test]
    fn parser_accepts_only_non_secret_output_paths() {
        assert!(Arguments::parse(["transcript", "result"]).is_ok());
        assert!(Arguments::parse(["transcript"]).is_err());
        assert!(Arguments::parse(["transcript", "result", "unexpected"]).is_err());
    }

    #[test]
    fn fixture_pair_start_requires_the_declared_three_metre_relation() {
        let observer = WorldPose {
            map_id: 0,
            east: -8949.95,
            north: -132.493,
            elevation: 83.531,
            orientation: 0.0,
        };
        let mover = WorldPose {
            east: -8946.95,
            ..observer
        };
        assert!(validate_pair_start(observer, mover).is_ok());
        assert!(
            validate_pair_start(
                observer,
                WorldPose {
                    east: -8946.94,
                    ..observer
                }
            )
            .is_err()
        );
    }
}
