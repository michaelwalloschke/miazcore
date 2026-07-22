use std::{
    error::Error,
    io,
    time::{Duration, Instant},
};

use client_session::{
    ClientConfig, ClientEventKind, ClientPhase, ControlCommand, EntryStage, MovementReadySession,
};

fn main() -> Result<(), Box<dyn Error>> {
    let repository_root = std::env::current_dir()?;
    let session =
        MovementReadySession::start(ClientConfig::reference_realm(&repository_root)?.load()?)?;
    session.send_control(ControlCommand::StartEntry)?;

    let deadline = Instant::now() + Duration::from_mins(3);
    loop {
        let snapshot = session.snapshot();
        if let Some(failure) = snapshot.latest_failure.as_ref() {
            let message = format!(
                "world entry failed: {:?} / {} / {} / {:?}",
                failure.category(),
                failure.stage(),
                failure.context(),
                failure.recommended_recovery()
            );
            session.shutdown()?;
            return Err(io::Error::other(message).into());
        }
        if snapshot.entry_anchor.is_some()
            && snapshot.run_speed.is_some()
            && matches!(
                snapshot.phase,
                ClientPhase::MovementReady | ClientPhase::Offline
            )
        {
            break;
        }
        if Instant::now() >= deadline {
            session.shutdown()?;
            return Err(io::Error::other("world entry evidence deadline expired").into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    let evidence = session.wait()?;
    verify(evidence.snapshot(), evidence.events())
}

fn verify(
    snapshot: &client_session::ClientSnapshot,
    events: &[client_session::ClientEvent],
) -> Result<(), Box<dyn Error>> {
    let character = snapshot
        .selected_character
        .as_ref()
        .ok_or_else(|| io::Error::other("world entry produced no selected character"))?;
    let anchor = snapshot
        .entry_anchor
        .ok_or_else(|| io::Error::other("world entry produced no Entry Anchor"))?;
    let run_speed = snapshot
        .run_speed
        .ok_or_else(|| io::Error::other("world entry produced no run speed"))?;
    let stages = events
        .iter()
        .filter_map(|event| match event.kind {
            ClientEventKind::PhaseChanged {
                phase: ClientPhase::Entering(stage),
            } => Some(stage),
            _ => None,
        })
        .collect::<Vec<_>>();
    let expected = [
        EntryStage::LoginConnection,
        EntryStage::LoginAuthentication,
        EntryStage::RealmSelection,
        EntryStage::WorldAuthentication,
        EntryStage::CharacterSelection,
        EntryStage::Bootstrap,
        EntryStage::ControlSynchronization,
    ];
    let ready_count = events
        .iter()
        .filter(|event| {
            matches!(
                event.kind,
                ClientEventKind::PhaseChanged {
                    phase: ClientPhase::MovementReady
                }
            )
        })
        .count();
    let pose_count = events
        .iter()
        .filter(|event| matches!(event.kind, ClientEventKind::PoseObserved { .. }))
        .count();
    let movement_events = events
        .iter()
        .filter(|event| matches!(event.kind, ClientEventKind::MovementSubmitted { .. }))
        .count();

    if snapshot.phase != ClientPhase::Offline
        || character.name() != "Miaztest"
        || stages != expected
        || ready_count != 1
        || pose_count != 1
        || movement_events != 0
        || snapshot.queue_counters.movement_revision != 0
        || !run_speed.is_finite()
        || run_speed <= 0.0
        || !matches!(
            events.last().map(|event| &event.kind),
            Some(ClientEventKind::Disconnected)
        )
    {
        return Err(io::Error::other("MovementReady evidence did not match the contract").into());
    }

    println!(
        "world entry: PASS name={} guid={:#x} map={} anchor=({:.3},{:.3},{:.3},{:.3}) run_speed={:.3} ready_events={} movement_events={} disconnected=true",
        character.name(),
        character.guid(),
        anchor.map_id,
        anchor.east,
        anchor.north,
        anchor.elevation,
        anchor.orientation,
        run_speed,
        ready_count,
        movement_events,
    );
    Ok(())
}
