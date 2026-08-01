use std::{
    error::Error,
    io,
    time::{Duration, Instant},
};

use client_session::{ClientEventKind, ClientPhase, ControlCommand, MovementReadySession};
#[path = "../src/fixture_profile.rs"]
mod fixture_profile;
use fixture_profile::FixtureProfile;

fn main() -> Result<(), Box<dyn Error>> {
    let profile = match std::env::args().nth(1).as_deref() {
        Some("pair-a") => FixtureProfile::PairA,
        Some("pair-b") => FixtureProfile::PairB,
        _ => return Err(io::Error::other("usage: fixture_pair_ready {pair-a|pair-b}").into()),
    };
    let root = std::env::current_dir()?;
    let session =
        MovementReadySession::start(fixture_profile::configuration(root, profile)?.load()?)?;
    session.send_control(ControlCommand::StartEntry)?;
    let deadline = Instant::now() + Duration::from_secs(45);
    let mut reached_movement_ready = false;
    loop {
        let snapshot = session.snapshot();
        reached_movement_ready |= snapshot.phase == ClientPhase::MovementReady;
        reached_movement_ready |= session.drain_events().into_iter().any(|event| {
            matches!(
                event.kind,
                ClientEventKind::PhaseChanged {
                    phase: ClientPhase::MovementReady
                }
            )
        });
        if snapshot.entry_anchor.is_some() && snapshot.run_speed.is_some() && reached_movement_ready
        {
            let character = snapshot
                .selected_character
                .as_ref()
                .ok_or_else(|| io::Error::other("missing character"))?;
            let anchor = snapshot
                .entry_anchor
                .ok_or_else(|| io::Error::other("missing anchor"))?;
            if character.name() != profile.character_name() || character.guid() == 0 {
                return Err(io::Error::other("profile identity mismatch").into());
            }
            println!(
                "PAIR_READY profile={} guid={:#x} map={} east={:.3} north={:.3} elevation={:.3} orientation={:.3}",
                if profile == FixtureProfile::PairA {
                    "pair-a"
                } else {
                    "pair-b"
                },
                character.guid(),
                anchor.map_id,
                anchor.east,
                anchor.north,
                anchor.elevation,
                anchor.orientation
            );
            session.shutdown()?;
            return Ok(());
        }
        if let Some(failure) = snapshot.latest_failure.as_ref() {
            session.shutdown()?;
            return Err(io::Error::other(format!(
                "MovementReady failed: {:?}/{:?}",
                failure.category(),
                failure.stage()
            ))
            .into());
        }
        if Instant::now() >= deadline {
            session.shutdown()?;
            return Err(io::Error::other("MovementReady failed").into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
