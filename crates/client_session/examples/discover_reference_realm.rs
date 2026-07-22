use std::{error::Error, io};

use client_session::{
    ClientConfig, ClientEventKind, ClientPhase, ControlCommand, EntryStage, RealmDiscoverySession,
};

fn main() -> Result<(), Box<dyn Error>> {
    let repository_root = std::env::current_dir()?;
    let loaded = ClientConfig::reference_realm(repository_root)?.load()?;
    let session = RealmDiscoverySession::start(loaded)?;
    session.send_control(ControlCommand::StartEntry)?;
    let evidence = session.wait()?;
    let snapshot = evidence.snapshot();

    if let Some(failure) = &snapshot.latest_failure {
        return Err(io::Error::other(format!(
            "realm discovery failed: {:?} / {} / {:?}",
            failure.category(),
            failure.context(),
            failure.recommended_recovery()
        ))
        .into());
    }
    let realm = snapshot
        .discovered_realm
        .as_ref()
        .ok_or_else(|| io::Error::other("realm discovery produced no verified realm"))?;
    if snapshot.phase != ClientPhase::Offline
        || realm.realm_id() != 1
        || realm.realm_name() != "Miazcore Reference Realm"
        || realm.client_build() != 12_340
        || realm.endpoint() != "127.0.0.1:8085".parse()?
        || !has_expected_semantic_path(evidence.events())
    {
        return Err(io::Error::other("realm discovery evidence did not match the contract").into());
    }

    println!(
        "realm discovery: PASS realm={} name={} build={} endpoint={} semantic_events={} disconnected=true",
        realm.realm_id(),
        realm.realm_name(),
        realm.client_build(),
        realm.endpoint(),
        evidence.events().len(),
    );
    Ok(())
}

fn has_expected_semantic_path(events: &[client_session::ClientEvent]) -> bool {
    let expected = [
        ClientPhase::Entering(EntryStage::LoginConnection),
        ClientPhase::Entering(EntryStage::LoginAuthentication),
        ClientPhase::Entering(EntryStage::RealmSelection),
    ];
    let phases = events.iter().filter_map(|event| match &event.kind {
        ClientEventKind::PhaseChanged { phase } => Some(phase),
        _ => None,
    });
    let mut expected_index = 0;
    for phase in phases {
        if expected.get(expected_index) == Some(phase) {
            expected_index += 1;
        }
    }
    expected_index == expected.len()
        && events
            .iter()
            .any(|event| matches!(event.kind, ClientEventKind::RealmDiscovered { .. }))
        && matches!(
            events.last().map(|event| &event.kind),
            Some(ClientEventKind::Disconnected)
        )
}
