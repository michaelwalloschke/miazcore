use std::{
    error::Error,
    fs, io,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use client_session::{
    ClientConfig, ClientConfigSpec, ClientPhase, ControlCommand, CredentialPaths,
    LiveDiagnosticSession, WorldPose,
};

const PEER_CHARACTER: &str = "Miazpeer";
const READY_DEADLINE: Duration = Duration::from_secs(45);
// The shell harness signals both clean shutdowns before it polls Realm-side
// persistence. This remains a bounded guard against a lost orchestration
// signal, rather than a second logout-settlement deadline.
const SHUTDOWN_DEADLINE: Duration = Duration::from_secs(90);

fn main() -> Result<(), Box<dyn Error>> {
    match Arguments::parse(std::env::args_os().skip(1))? {
        Arguments::Pair {
            peer_secret_root,
            ready_file,
            primary_shutdown_file,
            peer_shutdown_file,
        } => run_pair(
            &peer_secret_root,
            &ready_file,
            &primary_shutdown_file,
            &peer_shutdown_file,
        ),
        Arguments::Duplicate { result_file } => run_duplicate(&result_file),
    }
}

enum Arguments {
    Pair {
        peer_secret_root: PathBuf,
        ready_file: PathBuf,
        primary_shutdown_file: PathBuf,
        peer_shutdown_file: PathBuf,
    },
    Duplicate {
        result_file: PathBuf,
    },
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
        let as_strings = values
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        match as_strings.first().map(String::as_str) {
            Some("pair") if values.len() == 5 => Ok(Self::Pair {
                peer_secret_root: values[1].clone(),
                ready_file: values[2].clone(),
                primary_shutdown_file: values[3].clone(),
                peer_shutdown_file: values[4].clone(),
            }),
            Some("duplicate") if values.len() == 2 => Ok(Self::Duplicate {
                result_file: values[1].clone(),
            }),
            _ => Err(io::Error::other(
                "usage: measure_multi_session {pair PEER_SECRET_ROOT READY_FILE PRIMARY_STOP_FILE PEER_STOP_FILE|duplicate RESULT_FILE}",
            )),
        }
    }
}

fn run_pair(
    peer_secret_root: &Path,
    ready_file: &Path,
    primary_shutdown_file: &Path,
    peer_shutdown_file: &Path,
) -> Result<(), Box<dyn Error>> {
    let root = std::env::current_dir()?;
    let primary = ClientConfig::reference_realm(&root)?;
    let peer = peer_config(&primary, peer_secret_root)?;
    let primary_session = LiveDiagnosticSession::start(primary.load()?)?;
    let peer_session = LiveDiagnosticSession::start(peer.load()?)?;
    primary_session.send_control(ControlCommand::StartEntry)?;
    peer_session.send_control(ControlCommand::StartEntry)?;

    let primary_snapshot = wait_for_ready(&primary_session, "primary")?;
    let peer_snapshot = wait_for_ready(&peer_session, "peer")?;
    let primary_character = selected(&primary_snapshot, "primary")?;
    let peer_character = selected(&peer_snapshot, "peer")?;
    let primary_anchor = anchor(&primary_snapshot, "primary")?;
    let peer_anchor = anchor(&peer_snapshot, "peer")?;
    write_file(
        ready_file,
        &format!(
            "{{\"schema\":\"miazcore.multi-session-research.v1\",\"phase\":\"both-ready\",\"primary\":{{\"name\":\"{}\",\"guid\":\"{:x}\",\"map_id\":{}}},\"peer\":{{\"name\":\"{}\",\"guid\":\"{:x}\",\"map_id\":{}}},\"same_map\":{},\"horizontal_distance_metres\":{:.3}}}\n",
            primary_character.name(),
            primary_character.guid(),
            primary_anchor.map_id,
            peer_character.name(),
            peer_character.guid(),
            peer_anchor.map_id,
            primary_anchor.map_id == peer_anchor.map_id,
            horizontal_distance(primary_anchor, peer_anchor),
        ),
    )?;
    wait_for_file(primary_shutdown_file, "primary shutdown")?;
    primary_session.shutdown()?;
    write_file(
        &primary_shutdown_file.with_extension("observed"),
        "primary-disconnected\n",
    )?;
    wait_for_file(peer_shutdown_file, "peer shutdown")?;
    peer_session.shutdown()?;
    write_file(
        &peer_shutdown_file.with_extension("observed"),
        "peer-disconnected\n",
    )?;
    Ok(())
}

fn run_duplicate(result_file: &Path) -> Result<(), Box<dyn Error>> {
    let root = std::env::current_dir()?;
    let first = LiveDiagnosticSession::start(ClientConfig::reference_realm(&root)?.load()?)?;
    first.send_control(ControlCommand::StartEntry)?;
    let _ = wait_for_ready(&first, "first duplicate")?;
    let second = LiveDiagnosticSession::start(ClientConfig::reference_realm(&root)?.load()?)?;
    second.send_control(ControlCommand::StartEntry)?;
    thread::sleep(Duration::from_secs(3));
    let first_snapshot = first.snapshot();
    let second_snapshot = second.snapshot();
    write_file(
        result_file,
        &format!(
            "{{\"schema\":\"miazcore.multi-session-research.v1\",\"phase\":\"duplicate-settled\",\"first\":\"{}\",\"second\":\"{}\"}}\n",
            phase_label(&first_snapshot.phase),
            phase_label(&second_snapshot.phase),
        ),
    )?;
    first.shutdown()?;
    second.shutdown()?;
    Ok(())
}

fn peer_config(
    primary: &ClientConfig,
    peer_secret_root: &Path,
) -> Result<ClientConfig, client_session::ConfigError> {
    ClientConfig::new(ClientConfigSpec {
        realm_id: primary.identity().realm_id(),
        realm_name: primary.identity().realm_name().to_owned(),
        character_name: PEER_CHARACTER.to_owned(),
        client_build: primary.identity().client_build(),
        login_endpoint: primary.login_endpoint(),
        world_endpoint: primary.world_endpoint(),
        connect_timeout: primary.connect_timeout(),
        io_timeout: primary.io_timeout(),
        credentials: CredentialPaths::new(
            peer_secret_root.join("account"),
            peer_secret_root.join("password"),
        ),
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
        {
            return Ok(snapshot);
        }
        if Instant::now() >= deadline {
            return Err(io::Error::other(format!("{label} did not reach MovementReady")).into());
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn selected(
    snapshot: &client_session::ClientSnapshot,
    label: &str,
) -> Result<client_session::SelectedCharacter, Box<dyn Error>> {
    snapshot
        .selected_character
        .clone()
        .ok_or_else(|| io::Error::other(format!("{label} did not select a Character")).into())
}

fn anchor(
    snapshot: &client_session::ClientSnapshot,
    label: &str,
) -> Result<WorldPose, Box<dyn Error>> {
    snapshot
        .entry_anchor
        .ok_or_else(|| io::Error::other(format!("{label} did not observe an Entry Anchor")).into())
}

fn horizontal_distance(first: WorldPose, second: WorldPose) -> f32 {
    (first.east - second.east).hypot(first.north - second.north)
}

fn wait_for_file(path: &Path, description: &str) -> io::Result<()> {
    let deadline = Instant::now() + SHUTDOWN_DEADLINE;
    while !path.is_file() {
        if Instant::now() >= deadline {
            return Err(io::Error::other(format!(
                "timed out waiting for {description}"
            )));
        }
        thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}

fn write_file(path: &Path, contents: &str) -> io::Result<()> {
    fs::write(path, contents)
}

fn phase_label(phase: &ClientPhase) -> &'static str {
    match phase {
        ClientPhase::Offline => "offline",
        ClientPhase::Entering(_) => "entering",
        ClientPhase::MovementReady => "movement-ready",
        ClientPhase::Failed(_) => "failed",
        ClientPhase::ProvingMovement(_) => "proving-movement",
    }
}

#[cfg(test)]
mod tests {
    use super::Arguments;

    #[test]
    fn parser_accepts_only_non_secret_paths_and_known_modes() {
        assert!(matches!(
            Arguments::parse(["pair", "tmp/peer", "ready", "primary-stop", "peer-stop"]),
            Ok(Arguments::Pair { .. })
        ));
        assert!(matches!(
            Arguments::parse(["duplicate", "result.json"]),
            Ok(Arguments::Duplicate { .. })
        ));
        assert!(Arguments::parse(["pair", "missing"]).is_err());
        assert!(Arguments::parse(["unknown", "result.json"]).is_err());
    }
}
