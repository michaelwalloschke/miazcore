use std::{
    error::Error,
    fs, io,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use client_session::{
    ClientConfig, ClientConfigSpec, ClientPhase, ControlCommand, CredentialPaths,
    LiveDiagnosticSession, MovementIntent,
};

const PEER_CHARACTER: &str = "Miazpeer";
const READY_DEADLINE: Duration = Duration::from_secs(45);

fn main() -> Result<(), Box<dyn Error>> {
    let Arguments {
        peer_secret_root,
        transcript_file,
        result_file,
    } = Arguments::parse(std::env::args_os().skip(1))?;
    let started_at = Instant::now();
    let root = std::env::current_dir()?;
    let primary_config = ClientConfig::reference_realm(&root)?;
    let peer_config = peer_config(&primary_config, &peer_secret_root)?;

    let primary = LiveDiagnosticSession::start_with_remote_trace(
        primary_config.load()?,
        transcript_file.clone(),
    )?;
    primary.send_control(ControlCommand::StartEntry)?;
    let primary_snapshot = wait_for_ready(&primary, "observer")?;
    let observer_ready_after_ms = elapsed_ms(started_at);

    let peer = LiveDiagnosticSession::start(peer_config.load()?)?;
    peer.send_control(ControlCommand::StartEntry)?;
    let peer_snapshot = wait_for_ready(&peer, "peer")?;
    let peer_ready_after_ms = elapsed_ms(started_at);
    let primary_character = primary_snapshot
        .selected_character
        .ok_or_else(|| io::Error::other("observer has no selected Character"))?;
    let peer_character = peer_snapshot
        .selected_character
        .ok_or_else(|| io::Error::other("peer has no selected Character"))?;
    let anchor = peer_snapshot
        .entry_anchor
        .ok_or_else(|| io::Error::other("peer has no Entry Anchor"))?;

    let move_start_after_ms = elapsed_ms(started_at);
    peer.publish_movement_intent(
        MovementIntent::planar(1.0, 0.0).map_err(|_| io::Error::other("invalid trace intent"))?,
    )?;
    thread::sleep(Duration::from_millis(550));
    peer.publish_movement_intent(MovementIntent::idle())?;
    let move_stop_after_ms = elapsed_ms(started_at);
    thread::sleep(Duration::from_millis(250));
    let submitted_stop = peer
        .snapshot()
        .submitted_pose
        .filter(|_| peer.snapshot().submitted_pose_is_stopped)
        .ok_or_else(|| io::Error::other("peer has no stopped Submitted Pose"))?;
    let proof_start_after_ms = elapsed_ms(started_at);
    peer.send_control(ControlCommand::BeginMovementProof)?;

    // AzerothCore's ordinary saving logout is deliberately delayed. The
    // observer remains connected through that lifecycle boundary, then writes
    // its bounded transcript only after explicit shutdown below.
    thread::sleep(Duration::from_secs(23));
    primary.shutdown()?;
    peer.shutdown()?;

    if !transcript_file.is_file() {
        return Err(io::Error::other("remote transcript was not written").into());
    }
    fs::write(
        result_file,
        format!(
            "{{\"schema\":\"miazcore.remote-trace-run.v1\",\"observer_guid\":\"{:x}\",\"peer_guid\":\"{:x}\",\"map_id\":{},\"peer_anchor\":{{\"east\":{:.3},\"north\":{:.3},\"elevation\":{:.3}}},\"submitted_stop\":{{\"map_id\":{},\"east\":{:.3},\"north\":{:.3},\"elevation\":{:.3},\"orientation\":{:.3}}},\"timeline\":{{\"observer_ready_after_ms\":{},\"peer_ready_after_ms\":{},\"move_start_after_ms\":{},\"move_stop_after_ms\":{},\"proof_start_after_ms\":{}}}}}\n",
            primary_character.guid(),
            peer_character.guid(),
            anchor.map_id,
            anchor.east,
            anchor.north,
            anchor.elevation,
            submitted_stop.map_id,
            submitted_stop.east,
            submitted_stop.north,
            submitted_stop.elevation,
            submitted_stop.orientation,
            observer_ready_after_ms,
            peer_ready_after_ms,
            move_start_after_ms,
            move_stop_after_ms,
            proof_start_after_ms,
        ),
    )?;
    Ok(())
}

fn elapsed_ms(started_at: Instant) -> u64 {
    started_at
        .elapsed()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

struct Arguments {
    peer_secret_root: PathBuf,
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
        if values.len() != 3 {
            return Err(io::Error::other(
                "usage: trace_remote_world_updates PEER_SECRET_ROOT TRANSCRIPT_FILE RESULT_FILE",
            ));
        }
        Ok(Self {
            peer_secret_root: values[0].clone(),
            transcript_file: values[1].clone(),
            result_file: values[2].clone(),
        })
    }
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

#[cfg(test)]
mod tests {
    use super::Arguments;

    #[test]
    fn parser_accepts_only_the_three_non_secret_paths() {
        assert!(Arguments::parse(["peer", "trace", "result"]).is_ok());
        assert!(Arguments::parse(["peer", "trace"]).is_err());
    }
}
