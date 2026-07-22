use std::{
    io::{self, Read, Write},
    net::{Shutdown, TcpStream},
    sync::mpsc,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use client_protocol::{
    AcoreMovementInfo, CMSG_CHAR_ENUM, CMSG_FORCE_RUN_SPEED_CHANGE_ACK, CMSG_MOVE_SET_CAN_FLY_ACK,
    CMSG_PLAYER_LOGIN, CMSG_TIME_SYNC_RESP, LoginChallengeResponse, LoginProofResponse,
    ProtocolError, REALM_LIST_REQUEST, SMSG_AUTH_RESPONSE, SMSG_CHAR_ENUM,
    SMSG_COMPRESSED_UPDATE_OBJECT, SMSG_FORCE_RUN_SPEED_CHANGE, SMSG_LOGIN_VERIFY_WORLD,
    SMSG_MOVE_UNSET_CAN_FLY, SMSG_TIME_SYNC_REQ, SMSG_UPDATE_OBJECT, WorldAuthResponse,
    WorldClientStream, WorldEntryLocation, WorldServerStream, calculate_srp_client_proof,
    decode_authoritative_self_update, decode_character_enumeration, decode_force_run_speed_change,
    decode_login_verify_world, decode_time_sync_request, decode_unset_can_fly,
    decode_unsupported_self_control_guid, decode_world_auth_challenge, decode_world_auth_response,
    encode_force_run_speed_change_ack, encode_logon_challenge, encode_logon_proof,
    encode_move_set_can_fly_ack, encode_player_login, encode_time_sync_response,
    encode_world_auth_session_frame, read_logon_challenge_response, read_logon_proof_response,
    read_plain_world_server_frame, read_realm_list_response,
};
use zeroize::Zeroizing;

use crate::{
    ClientConfig, ClientFailure, ClientPhase, ControlCommand, DiscoveredRealm, EntryStage,
    FailureCategory, LoadedClientConfig, RecoveryAction, SelectedCharacter, WorldPose,
    api::SelectedCharacterFields,
    boundary::{BoundaryError, WorkerBoundary},
    config::CredentialMaterial,
    machine::EntryMachine,
};

trait LoginTransport: Read + Write + Send {
    fn close(&mut self) -> io::Result<()>;
}

trait TransportFactory: Send {
    type Transport: LoginTransport;

    fn connect_login(&mut self, config: &ClientConfig) -> io::Result<Self::Transport>;

    fn connect_world(&mut self, config: &ClientConfig) -> io::Result<Self::Transport> {
        self.connect_login(config)
    }
}

trait MonotonicClock: Send {
    fn now(&mut self) -> Duration;
}

trait EntropySource: Send {
    fn fill(&mut self, destination: &mut [u8]) -> Result<(), ()>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkerTarget {
    RealmDiscovery,
    CharacterSelection,
    MovementReady,
}

impl WorkerTarget {
    const fn supports_explicit_retry(self) -> bool {
        matches!(self, Self::MovementReady)
    }

    const fn expected_command(self, attempt_failed: bool) -> ControlCommand {
        if self.supports_explicit_retry() && attempt_failed {
            ControlCommand::RetryEntry
        } else {
            ControlCommand::StartEntry
        }
    }

    const fn connection_count(self) -> u32 {
        match self {
            Self::RealmDiscovery => 1,
            Self::CharacterSelection | Self::MovementReady => 2,
        }
    }

    const fn io_count(self) -> u32 {
        match self {
            Self::RealmDiscovery => 4,
            Self::CharacterSelection => 8,
            Self::MovementReady => 64,
        }
    }

    const fn stops_after_realm(self) -> bool {
        matches!(self, Self::RealmDiscovery)
    }

    const fn stops_after_character(self) -> bool {
        matches!(self, Self::CharacterSelection)
    }
}

enum EntryAttemptOutcome {
    RealmDiscovered,
    CharacterSelected(SelectedCharacter),
    MovementReady,
}

pub(crate) fn spawn_production_worker(
    loaded: LoadedClientConfig,
    mut boundary: WorkerBoundary,
    target: WorkerTarget,
) -> Result<JoinHandle<()>, BoundaryError> {
    thread::Builder::new()
        .name("miazcore-realm-discovery".to_owned())
        .spawn(move || {
            let (config, mut credentials) = loaded.into_parts();
            credentials.normalize_for_login();
            run_worker_loop_for(
                &config,
                &credentials,
                &mut boundary,
                &mut TcpTransportFactory,
                &mut SystemClock::new(),
                &mut SystemEntropy,
                target,
            );
            boundary.mark_stopped();
        })
        .map_err(|_| BoundaryError::WorkerStopped)
}

#[cfg(test)]
fn run_worker_loop<F, C, E>(
    config: &ClientConfig,
    credentials: &CredentialMaterial,
    boundary: &mut WorkerBoundary,
    factory: &mut F,
    clock: &mut C,
    entropy: &mut E,
) where
    F: TransportFactory,
    C: MonotonicClock,
    E: EntropySource,
{
    run_worker_loop_for(
        config,
        credentials,
        boundary,
        factory,
        clock,
        entropy,
        WorkerTarget::RealmDiscovery,
    );
}

#[allow(clippy::too_many_arguments)]
fn run_worker_loop_for<F, C, E>(
    config: &ClientConfig,
    credentials: &CredentialMaterial,
    boundary: &mut WorkerBoundary,
    factory: &mut F,
    clock: &mut C,
    entropy: &mut E,
    target: WorkerTarget,
) where
    F: TransportFactory,
    C: MonotonicClock,
    E: EntropySource,
{
    let mut movement_attempt_failed = false;
    loop {
        match boundary.receive_control(Duration::from_millis(20)) {
            Ok(command) => {
                boundary.control_consumed();
                if command == ControlCommand::Disconnect || boundary.is_shutdown() {
                    boundary.disconnect();
                    return;
                }
                let expected_command = target.expected_command(movement_attempt_failed);
                if command != expected_command {
                    let failure = ClientFailure::new(
                        FailureCategory::Configuration,
                        "world entry",
                        if movement_attempt_failed {
                            "failed world entry requires explicit retry or disconnect"
                        } else {
                            "command is outside the current headless entry capability"
                        },
                        if movement_attempt_failed {
                            RecoveryAction::RetryExplicitly
                        } else {
                            RecoveryAction::RestartClient
                        },
                    );
                    if !boundary.reject(command.kind(), failure) {
                        return;
                    }
                    continue;
                }
                if command == ControlCommand::RetryEntry {
                    boundary.reset_for_retry();
                }
                match run_entry_attempt(
                    config,
                    credentials,
                    boundary,
                    factory,
                    clock,
                    entropy,
                    target,
                ) {
                    Ok(outcome) => {
                        let published = match outcome {
                            EntryAttemptOutcome::CharacterSelected(character) => {
                                boundary.selected(character)
                            }
                            EntryAttemptOutcome::RealmDiscovered
                            | EntryAttemptOutcome::MovementReady => true,
                        };
                        if published {
                            boundary.disconnect();
                        }
                    }
                    Err(DiscoveryError::Failure(failure)) => {
                        boundary.fail(command.kind(), failure);
                        if target.supports_explicit_retry() {
                            movement_attempt_failed = true;
                            continue;
                        }
                    }
                    Err(DiscoveryError::Cancelled) => {
                        boundary.discard_pending_controls();
                        boundary.disconnect();
                    }
                    Err(DiscoveryError::Boundary) => {}
                }
                return;
            }
            Err(mpsc::RecvTimeoutError::Timeout) if boundary.is_shutdown() => return,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn run_entry_attempt<F, C, E>(
    config: &ClientConfig,
    credentials: &CredentialMaterial,
    boundary: &mut WorkerBoundary,
    factory: &mut F,
    clock: &mut C,
    entropy: &mut E,
    target: WorkerTarget,
) -> Result<EntryAttemptOutcome, DiscoveryError>
where
    F: TransportFactory,
    C: MonotonicClock,
    E: EntropySource,
{
    check_cancelled(boundary)?;
    let started = clock.now();
    let budget = config
        .connect_timeout()
        .saturating_mul(target.connection_count())
        .checked_add(config.io_timeout().saturating_mul(target.io_count()))
        .ok_or_else(timeout_failure)?;
    let deadline = started.checked_add(budget).ok_or_else(timeout_failure)?;
    let mut machine = EntryMachine::new();
    transition(boundary, machine.begin(), EntryStage::LoginConnection)?;

    let mut transport = factory.connect_login(config).map_err(|error| {
        io_failure(
            error.kind(),
            "login connection",
            RecoveryAction::CheckReferenceRealm,
        )
    })?;
    let login_result = exchange(
        config,
        credentials,
        boundary,
        &mut machine,
        &mut transport,
        clock,
        entropy,
        deadline,
    );
    let _ = transport.close();
    let (realm, session_key) = match login_result {
        Ok(result) => result,
        Err(error) => {
            machine.fail();
            return Err(error);
        }
    };
    machine
        .realm_discovered()
        .map_err(|_| internal_transition_failure())?;
    if !boundary.discovered(realm) {
        return Err(DiscoveryError::Boundary);
    }
    if target.stops_after_realm() {
        machine
            .complete_after_realm()
            .map_err(|_| internal_transition_failure())?;
        return Ok(EntryAttemptOutcome::RealmDiscovered);
    }

    transition(
        boundary,
        machine.authenticating_world(),
        EntryStage::WorldAuthentication,
    )?;
    let mut world_transport = factory.connect_world(config).map_err(|error| {
        world_io_failure(
            error.kind(),
            "world connection",
            RecoveryAction::CheckReferenceRealm,
        )
    })?;
    let result = select_character(
        config,
        credentials,
        boundary,
        &mut machine,
        &mut world_transport,
        clock,
        entropy,
        deadline,
        &session_key,
        target,
    );
    let _ = world_transport.close();
    if result.is_err() {
        machine.fail();
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn exchange<T, C, E>(
    config: &ClientConfig,
    credentials: &CredentialMaterial,
    boundary: &mut WorkerBoundary,
    machine: &mut EntryMachine,
    transport: &mut T,
    clock: &mut C,
    entropy: &mut E,
    deadline: Duration,
) -> Result<(DiscoveredRealm, Zeroizing<[u8; 40]>), DiscoveryError>
where
    T: LoginTransport,
    C: MonotonicClock,
    E: EntropySource,
{
    let session_key = authenticate(
        credentials,
        boundary,
        machine,
        transport,
        clock,
        entropy,
        deadline,
    )?;
    let realm = select_realm(config, boundary, machine, transport, clock, deadline)?;
    Ok((realm, session_key))
}

#[allow(clippy::too_many_arguments)]
fn authenticate<T, C, E>(
    credentials: &CredentialMaterial,
    boundary: &mut WorkerBoundary,
    machine: &mut EntryMachine,
    transport: &mut T,
    clock: &mut C,
    entropy: &mut E,
    deadline: Duration,
) -> Result<Zeroizing<[u8; 40]>, DiscoveryError>
where
    T: LoginTransport,
    C: MonotonicClock,
    E: EntropySource,
{
    check_cancelled(boundary)?;
    check_deadline(clock, deadline)?;
    transition(
        boundary,
        machine.authenticating(),
        EntryStage::LoginAuthentication,
    )?;
    let challenge_request = Zeroizing::new(encode_logon_challenge(credentials.account()).map_err(
        |error| protocol_failure(error, "login challenge", RecoveryAction::FixConfiguration),
    )?);
    transport.write_all(&challenge_request).map_err(|error| {
        io_failure(
            error.kind(),
            "login authentication",
            RecoveryAction::CheckReferenceRealm,
        )
    })?;
    check_cancelled(boundary)?;
    let challenge = match read_logon_challenge_response(transport).map_err(|error| {
        protocol_failure(
            error,
            "login authentication",
            RecoveryAction::CheckReferenceRealm,
        )
    })? {
        LoginChallengeResponse::Accepted(challenge) => challenge,
        LoginChallengeResponse::Rejected { .. } => return Err(authentication_rejected()),
    };
    check_cancelled(boundary)?;
    check_deadline(clock, deadline)?;

    let mut private_ephemeral = Zeroizing::new([0_u8; 32]);
    entropy
        .fill(private_ephemeral.as_mut())
        .map_err(|()| entropy_failure())?;
    check_cancelled(boundary)?;
    let proof = calculate_srp_client_proof(
        credentials.account(),
        credentials.password(),
        &challenge,
        *private_ephemeral,
    )
    .map_err(|error| {
        protocol_failure(
            error,
            "login authentication",
            RecoveryAction::CheckReferenceRealm,
        )
    })?;
    let proof_request = Zeroizing::new(encode_logon_proof(&proof));
    transport
        .write_all(proof_request.as_ref())
        .map_err(|error| {
            io_failure(
                error.kind(),
                "login authentication",
                RecoveryAction::CheckReferenceRealm,
            )
        })?;
    check_cancelled(boundary)?;
    let server_proof = match read_logon_proof_response(transport).map_err(|error| {
        protocol_failure(
            error,
            "login authentication",
            RecoveryAction::CheckReferenceRealm,
        )
    })? {
        LoginProofResponse::Accepted { server_proof } => server_proof,
        LoginProofResponse::Rejected { .. } => return Err(authentication_rejected()),
    };
    check_cancelled(boundary)?;
    if !proof.verify_server_proof(&server_proof) {
        return Err(DiscoveryError::Failure(ClientFailure::new(
            FailureCategory::Authentication,
            "login authentication",
            "login server proof did not authenticate the session",
            RecoveryAction::CheckCredentials,
        )));
    }
    check_deadline(clock, deadline)?;
    Ok(Zeroizing::new(*proof.session_key().as_bytes()))
}

fn select_realm<T, C>(
    config: &ClientConfig,
    boundary: &mut WorkerBoundary,
    machine: &mut EntryMachine,
    transport: &mut T,
    clock: &mut C,
    deadline: Duration,
) -> Result<DiscoveredRealm, DiscoveryError>
where
    T: LoginTransport,
    C: MonotonicClock,
{
    check_cancelled(boundary)?;
    transition(
        boundary,
        machine.selecting_realm(),
        EntryStage::RealmSelection,
    )?;
    transport.write_all(&REALM_LIST_REQUEST).map_err(|error| {
        io_failure(
            error.kind(),
            "realm selection",
            RecoveryAction::CheckReferenceRealm,
        )
    })?;
    check_cancelled(boundary)?;
    let realms = read_realm_list_response(transport).map_err(|error| {
        protocol_failure(
            error,
            "realm selection",
            RecoveryAction::CheckReferenceRealm,
        )
    })?;
    check_cancelled(boundary)?;
    check_deadline(clock, deadline)?;
    let realm = realms
        .into_iter()
        .find(|realm| realm.name() == config.identity().realm_name())
        .ok_or_else(realm_mismatch)?;
    let endpoint = realm.address().parse().map_err(|_| realm_mismatch())?;
    if realm.is_locked()
        || u32::from(realm.id()) != config.identity().realm_id()
        || realm
            .build()
            .is_some_and(|build| build != config.identity().client_build())
        || endpoint != config.world_endpoint()
    {
        return Err(realm_mismatch());
    }
    DiscoveredRealm::new(
        u32::from(realm.id()),
        realm.name(),
        realm.build().unwrap_or(config.identity().client_build()),
        endpoint,
    )
    .map_err(|_| realm_mismatch())
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn select_character<T, C, E>(
    config: &ClientConfig,
    credentials: &CredentialMaterial,
    boundary: &mut WorkerBoundary,
    machine: &mut EntryMachine,
    transport: &mut T,
    clock: &mut C,
    entropy: &mut E,
    deadline: Duration,
    session_key: &[u8; 40],
    target: WorkerTarget,
) -> Result<EntryAttemptOutcome, DiscoveryError>
where
    T: LoginTransport,
    C: MonotonicClock,
    E: EntropySource,
{
    check_cancelled(boundary)?;
    check_deadline(clock, deadline)?;
    let challenge_frame = read_plain_world_server_frame(transport).map_err(|error| {
        world_protocol_failure(
            error,
            "world authentication",
            RecoveryAction::CheckReferenceRealm,
        )
    })?;
    let challenge = decode_world_auth_challenge(&challenge_frame).map_err(|error| {
        world_protocol_failure(
            error,
            "world authentication",
            RecoveryAction::CheckReferenceRealm,
        )
    })?;

    let mut client_seed = Zeroizing::new([0_u8; 4]);
    entropy
        .fill(client_seed.as_mut())
        .map_err(|()| entropy_failure())?;
    check_cancelled(boundary)?;
    let client_seed = u32::from_le_bytes(*client_seed);
    let auth_frame = encode_world_auth_session_frame(
        credentials.account(),
        config.identity().realm_id(),
        client_seed,
        challenge.server_seed(),
        session_key,
    )
    .map_err(|error| {
        world_protocol_failure(
            error,
            "world authentication",
            RecoveryAction::FixConfiguration,
        )
    })?;
    transport.write_all(auth_frame.as_ref()).map_err(|error| {
        world_io_failure(
            error.kind(),
            "world authentication",
            RecoveryAction::CheckReferenceRealm,
        )
    })?;
    check_cancelled(boundary)?;

    let mut server_stream = WorldServerStream::new(session_key);
    let mut client_stream = WorldClientStream::new(session_key);
    let auth_response = server_stream.read_frame(transport).map_err(|error| {
        world_protocol_failure(
            error,
            "world authentication",
            RecoveryAction::CheckReferenceRealm,
        )
    })?;
    if auth_response.opcode() != SMSG_AUTH_RESPONSE {
        return Err(world_protocol_drift("world authentication"));
    }
    match decode_world_auth_response(auth_response.payload()).map_err(|error| {
        world_protocol_failure(
            error,
            "world authentication",
            RecoveryAction::CheckReferenceRealm,
        )
    })? {
        WorldAuthResponse::Accepted => {}
        WorldAuthResponse::Rejected => return Err(world_authentication_rejected()),
    }

    transition(
        boundary,
        machine.selecting_character(),
        EntryStage::CharacterSelection,
    )?;
    let request = client_stream
        .encode_frame(CMSG_CHAR_ENUM, &[])
        .map_err(|error| {
            world_protocol_failure(
                error,
                "character selection",
                RecoveryAction::CheckReferenceRealm,
            )
        })?;
    transport.write_all(&request).map_err(|error| {
        world_io_failure(
            error.kind(),
            "character selection",
            RecoveryAction::CheckReferenceRealm,
        )
    })?;

    let selected = read_selected_character(
        config,
        boundary,
        transport,
        clock,
        deadline,
        &mut server_stream,
    )?;
    if target.stops_after_character() {
        machine
            .complete()
            .map_err(|_| internal_transition_failure())?;
        return Ok(EntryAttemptOutcome::CharacterSelected(selected));
    }
    if !boundary.selected(selected.clone()) {
        return Err(DiscoveryError::Boundary);
    }
    transition(boundary, machine.bootstrapping(), EntryStage::Bootstrap)?;
    let request = client_stream
        .encode_frame(CMSG_PLAYER_LOGIN, &encode_player_login(selected.guid()))
        .map_err(|error| {
            world_protocol_failure(
                error,
                "world bootstrap",
                RecoveryAction::CheckReferenceRealm,
            )
        })?;
    transport.write_all(&request).map_err(|error| {
        world_io_failure(
            error.kind(),
            "world bootstrap",
            RecoveryAction::CheckReferenceRealm,
        )
    })?;
    reach_movement_ready(
        boundary,
        machine,
        transport,
        clock,
        deadline,
        &mut server_stream,
        &mut client_stream,
        &selected,
    )?;
    Ok(EntryAttemptOutcome::MovementReady)
}

fn read_selected_character<T, C>(
    config: &ClientConfig,
    boundary: &WorkerBoundary,
    transport: &mut T,
    clock: &mut C,
    deadline: Duration,
    server_stream: &mut WorldServerStream,
) -> Result<SelectedCharacter, DiscoveryError>
where
    T: LoginTransport,
    C: MonotonicClock,
{
    for _ in 0..64 {
        check_cancelled(boundary)?;
        check_deadline(clock, deadline)?;
        let frame = server_stream.read_frame(transport).map_err(|error| {
            world_protocol_failure(
                error,
                "character selection",
                RecoveryAction::CheckReferenceRealm,
            )
        })?;
        if frame.opcode() != SMSG_CHAR_ENUM {
            continue;
        }
        let characters = decode_character_enumeration(frame.payload()).map_err(|error| {
            world_protocol_failure(
                error,
                "character selection",
                RecoveryAction::CheckReferenceRealm,
            )
        })?;
        let mut matches = characters
            .into_iter()
            .filter(|character| character.name() == config.identity().character_name());
        let selected = matches.next().ok_or_else(character_absent)?;
        if matches.next().is_some() {
            return Err(character_duplicate());
        }
        return SelectedCharacter::new(SelectedCharacterFields {
            guid: selected.guid(),
            name: selected.name().to_owned(),
            race: selected.race(),
            class: selected.class(),
            gender: selected.gender(),
            level: selected.level(),
            area_id: selected.area_id(),
            map_id: selected.map_id(),
            position: selected.position(),
        })
        .map_err(|_| world_protocol_drift("character selection"));
    }

    Err(world_protocol_drift("character selection"))
}

struct BootstrapProgress {
    location: Option<WorldEntryLocation>,
    movement: Option<AcoreMovementInfo>,
    run_speed: Option<f32>,
    time_synchronized: bool,
    no_flight_acknowledged: bool,
    synchronizing: bool,
}

impl BootstrapProgress {
    const fn new() -> Self {
        Self {
            location: None,
            movement: None,
            run_speed: None,
            time_synchronized: false,
            no_flight_acknowledged: false,
            synchronizing: false,
        }
    }

    const fn ready(&self) -> bool {
        self.synchronizing
            && self.run_speed.is_some()
            && self.time_synchronized
            && self.no_flight_acknowledged
    }

    const fn stage(&self) -> &'static str {
        if self.synchronizing {
            "control synchronization"
        } else {
            "world bootstrap"
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn reach_movement_ready<T, C>(
    boundary: &mut WorkerBoundary,
    machine: &mut EntryMachine,
    transport: &mut T,
    clock: &mut C,
    deadline: Duration,
    server_stream: &mut WorldServerStream,
    client_stream: &mut WorldClientStream,
    selected: &SelectedCharacter,
) -> Result<(), DiscoveryError>
where
    T: LoginTransport,
    C: MonotonicClock,
{
    let mut progress = BootstrapProgress::new();
    let mut pending_run_speeds = Vec::new();
    let mut pending_no_flight = Vec::new();

    for _ in 0..1024 {
        check_cancelled(boundary)?;
        check_deadline(clock, deadline)?;
        let frame = server_stream.read_frame(transport).map_err(|error| {
            world_protocol_failure(error, progress.stage(), RecoveryAction::CheckReferenceRealm)
        })?;
        match frame.opcode() {
            SMSG_LOGIN_VERIFY_WORLD => {
                if progress.location.is_some() {
                    return Err(entry_invariant_failure());
                }
                let location = decode_login_verify_world(frame.payload()).map_err(|error| {
                    world_protocol_failure(
                        error,
                        "world bootstrap",
                        RecoveryAction::CheckReferenceRealm,
                    )
                })?;
                validate_selected_location(selected, location)?;
                if let Some(movement) = progress.movement {
                    validate_self_location(movement, location)?;
                }
                let pose = world_pose(location);
                if !boundary.observe_entry_anchor(pose) {
                    return Err(DiscoveryError::Boundary);
                }
                progress.location = Some(location);
            }
            SMSG_UPDATE_OBJECT | SMSG_COMPRESSED_UPDATE_OBJECT => {
                if let Some(self_state) = decode_authoritative_self_update(
                    frame.opcode(),
                    frame.payload(),
                    selected.guid(),
                )
                .map_err(|error| {
                    world_protocol_failure(
                        error,
                        "world bootstrap",
                        RecoveryAction::CheckReferenceRealm,
                    )
                })? {
                    if progress.movement.is_some() || self_state.guid() != selected.guid() {
                        return Err(entry_invariant_failure());
                    }
                    if let Some(location) = progress.location {
                        validate_self_location(self_state.movement(), location)?;
                    }
                    progress.movement = Some(self_state.movement());
                    progress.run_speed = Some(self_state.speeds().run());

                    for speed in pending_run_speeds.drain(..) {
                        acknowledge_run_speed(
                            transport,
                            client_stream,
                            clock,
                            &mut progress,
                            speed,
                            selected.guid(),
                        )?;
                    }
                    for no_flight in pending_no_flight.drain(..) {
                        acknowledge_no_flight(
                            transport,
                            client_stream,
                            clock,
                            &mut progress,
                            no_flight,
                            selected.guid(),
                        )?;
                    }
                }
            }
            SMSG_FORCE_RUN_SPEED_CHANGE => {
                let speed = decode_force_run_speed_change(frame.payload()).map_err(|error| {
                    world_protocol_failure(
                        error,
                        progress.stage(),
                        RecoveryAction::CheckReferenceRealm,
                    )
                })?;
                if speed.guid() != selected.guid() {
                    return Err(entry_invariant_failure());
                }
                if progress.movement.is_some() {
                    acknowledge_run_speed(
                        transport,
                        client_stream,
                        clock,
                        &mut progress,
                        speed,
                        selected.guid(),
                    )?;
                } else if pending_run_speeds.len() < 8 {
                    pending_run_speeds.push(speed);
                } else {
                    return Err(world_protocol_drift("world bootstrap"));
                }
            }
            SMSG_TIME_SYNC_REQ => {
                let counter = decode_time_sync_request(frame.payload()).map_err(|error| {
                    world_protocol_failure(
                        error,
                        progress.stage(),
                        RecoveryAction::CheckReferenceRealm,
                    )
                })?;
                let payload = encode_time_sync_response(counter, client_time_ms(clock));
                write_world_frame(
                    transport,
                    client_stream,
                    CMSG_TIME_SYNC_RESP,
                    &payload,
                    progress.stage(),
                )?;
                progress.time_synchronized = true;
            }
            SMSG_MOVE_UNSET_CAN_FLY => {
                let no_flight = decode_unset_can_fly(frame.payload()).map_err(|error| {
                    world_protocol_failure(
                        error,
                        progress.stage(),
                        RecoveryAction::CheckReferenceRealm,
                    )
                })?;
                if no_flight.guid() != selected.guid() {
                    return Err(entry_invariant_failure());
                }
                if progress.movement.is_some() {
                    acknowledge_no_flight(
                        transport,
                        client_stream,
                        clock,
                        &mut progress,
                        no_flight,
                        selected.guid(),
                    )?;
                } else if pending_no_flight.len() < 8 {
                    pending_no_flight.push(no_flight);
                } else {
                    return Err(world_protocol_drift("world bootstrap"));
                }
            }
            opcode => {
                let controlled_guid = decode_unsupported_self_control_guid(opcode, frame.payload())
                    .map_err(|error| {
                        world_protocol_failure(
                            error,
                            progress.stage(),
                            RecoveryAction::CheckReferenceRealm,
                        )
                    })?;
                if controlled_guid == Some(selected.guid()) {
                    return Err(unsupported_self_control_failure());
                }
            }
        }

        if !progress.synchronizing && progress.location.is_some() && progress.movement.is_some() {
            transition(
                boundary,
                machine.synchronizing(),
                EntryStage::ControlSynchronization,
            )?;
            progress.synchronizing = true;
        }
        if progress.ready() {
            machine
                .movement_ready()
                .map_err(|_| internal_transition_failure())?;
            let run_speed = progress.run_speed.ok_or_else(entry_invariant_failure)?;
            if !boundary.movement_ready(run_speed) {
                return Err(DiscoveryError::Boundary);
            }
            return Ok(());
        }
    }

    Err(world_protocol_drift(progress.stage()))
}

fn acknowledge_run_speed<T, C>(
    transport: &mut T,
    client_stream: &mut WorldClientStream,
    clock: &mut C,
    progress: &mut BootstrapProgress,
    speed: client_protocol::ForceRunSpeedChange,
    selected_guid: u64,
) -> Result<(), DiscoveryError>
where
    T: LoginTransport,
    C: MonotonicClock,
{
    if speed.guid() != selected_guid {
        return Err(entry_invariant_failure());
    }
    let movement = progress
        .movement
        .ok_or_else(entry_invariant_failure)?
        .with_timestamp(client_time_ms(clock));
    let payload = encode_force_run_speed_change_ack(speed, movement).map_err(|error| {
        world_protocol_failure(error, progress.stage(), RecoveryAction::CheckReferenceRealm)
    })?;
    write_world_frame(
        transport,
        client_stream,
        CMSG_FORCE_RUN_SPEED_CHANGE_ACK,
        &payload,
        progress.stage(),
    )?;
    progress.movement = Some(movement);
    progress.run_speed = Some(speed.run_speed());
    Ok(())
}

fn acknowledge_no_flight<T, C>(
    transport: &mut T,
    client_stream: &mut WorldClientStream,
    clock: &mut C,
    progress: &mut BootstrapProgress,
    no_flight: client_protocol::UnsetCanFly,
    selected_guid: u64,
) -> Result<(), DiscoveryError>
where
    T: LoginTransport,
    C: MonotonicClock,
{
    if no_flight.guid() != selected_guid {
        return Err(entry_invariant_failure());
    }
    let movement = progress
        .movement
        .ok_or_else(entry_invariant_failure)?
        .with_timestamp(client_time_ms(clock));
    let payload = encode_move_set_can_fly_ack(no_flight, movement).map_err(|error| {
        world_protocol_failure(error, progress.stage(), RecoveryAction::CheckReferenceRealm)
    })?;
    write_world_frame(
        transport,
        client_stream,
        CMSG_MOVE_SET_CAN_FLY_ACK,
        &payload,
        progress.stage(),
    )?;
    progress.movement = Some(movement);
    progress.no_flight_acknowledged = true;
    Ok(())
}

fn write_world_frame<T: LoginTransport>(
    transport: &mut T,
    client_stream: &mut WorldClientStream,
    opcode: u32,
    payload: &[u8],
    stage: &'static str,
) -> Result<(), DiscoveryError> {
    let frame = client_stream
        .encode_frame(opcode, payload)
        .map_err(|error| {
            world_protocol_failure(error, stage, RecoveryAction::CheckReferenceRealm)
        })?;
    transport
        .write_all(&frame)
        .map_err(|error| world_io_failure(error.kind(), stage, RecoveryAction::CheckReferenceRealm))
}

fn validate_selected_location(
    selected: &SelectedCharacter,
    location: WorldEntryLocation,
) -> Result<(), DiscoveryError> {
    if selected.map_id() != location.map_id()
        || squared_distance(selected.position(), location.position()) > 0.25_f32.powi(2)
    {
        return Err(entry_invariant_failure());
    }
    Ok(())
}

fn validate_self_location(
    movement: AcoreMovementInfo,
    location: WorldEntryLocation,
) -> Result<(), DiscoveryError> {
    if squared_distance(movement.position(), location.position()) > 0.25_f32.powi(2)
        || angular_distance(movement.orientation(), location.orientation()) > 0.01
    {
        return Err(entry_invariant_failure());
    }
    Ok(())
}

fn squared_distance(left: [f32; 3], right: [f32; 3]) -> f32 {
    left.into_iter()
        .zip(right)
        .map(|(left, right)| (left - right).powi(2))
        .sum()
}

fn angular_distance(left: f32, right: f32) -> f32 {
    let full_turn = 2.0 * std::f32::consts::PI;
    let distance = (left - right).abs() % full_turn;
    distance.min(full_turn - distance)
}

fn world_pose(location: WorldEntryLocation) -> WorldPose {
    let [east, north, elevation] = location.position();
    WorldPose {
        map_id: location.map_id(),
        east,
        north,
        elevation,
        orientation: location.orientation(),
    }
}

fn client_time_ms(clock: &mut impl MonotonicClock) -> u32 {
    let modulus = u128::from(u32::MAX) + 1;
    u32::try_from(clock.now().as_millis() % modulus).expect("client time modulo always fits in u32")
}

fn transition(
    boundary: &mut WorkerBoundary,
    transition: Result<EntryStage, crate::machine::InvalidTransition>,
    expected: EntryStage,
) -> Result<(), DiscoveryError> {
    let stage = transition.map_err(|_| internal_transition_failure())?;
    if stage != expected || !boundary.transition(ClientPhase::Entering(stage)) {
        return Err(DiscoveryError::Boundary);
    }
    Ok(())
}

fn check_deadline(
    clock: &mut impl MonotonicClock,
    deadline: Duration,
) -> Result<(), DiscoveryError> {
    if clock.now() > deadline {
        return Err(timeout_failure());
    }
    Ok(())
}

enum DiscoveryError {
    Failure(ClientFailure),
    Cancelled,
    Boundary,
}

fn check_cancelled(boundary: &WorkerBoundary) -> Result<(), DiscoveryError> {
    if boundary.is_shutdown() {
        Err(DiscoveryError::Cancelled)
    } else {
        Ok(())
    }
}

fn authentication_rejected() -> DiscoveryError {
    DiscoveryError::Failure(ClientFailure::new(
        FailureCategory::Authentication,
        "login authentication",
        "fixture account authentication was rejected",
        RecoveryAction::CheckCredentials,
    ))
}

fn realm_mismatch() -> DiscoveryError {
    DiscoveryError::Failure(ClientFailure::new(
        FailureCategory::ProtocolIncompatibility,
        "realm selection",
        "authenticated realm identity build or endpoint did not match configuration",
        RecoveryAction::CheckReferenceRealm,
    ))
}

fn world_authentication_rejected() -> DiscoveryError {
    DiscoveryError::Failure(ClientFailure::new(
        FailureCategory::Authentication,
        "world authentication",
        "world session authentication was rejected",
        RecoveryAction::CheckCredentials,
    ))
}

fn character_absent() -> DiscoveryError {
    DiscoveryError::Failure(ClientFailure::new(
        FailureCategory::Configuration,
        "character selection",
        "configured character was absent from the authenticated realm",
        RecoveryAction::FixConfiguration,
    ))
}

fn character_duplicate() -> DiscoveryError {
    DiscoveryError::Failure(ClientFailure::new(
        FailureCategory::ProtocolIncompatibility,
        "character selection",
        "authenticated realm returned duplicate configured characters",
        RecoveryAction::CheckReferenceRealm,
    ))
}

fn entry_invariant_failure() -> DiscoveryError {
    DiscoveryError::Failure(ClientFailure::new(
        FailureCategory::ProtocolIncompatibility,
        "world bootstrap",
        "authoritative entry state did not corroborate the selected character",
        RecoveryAction::CheckReferenceRealm,
    ))
}

fn unsupported_self_control_failure() -> DiscoveryError {
    DiscoveryError::Failure(ClientFailure::new(
        FailureCategory::UnsupportedSelfControl,
        "world bootstrap",
        "selected character entered an unsupported movement or control state",
        RecoveryAction::CheckReferenceRealm,
    ))
}

fn world_protocol_drift(stage: &'static str) -> DiscoveryError {
    DiscoveryError::Failure(ClientFailure::new(
        FailureCategory::ProtocolIncompatibility,
        stage,
        "Reference Realm world protocol response was incompatible",
        RecoveryAction::CheckReferenceRealm,
    ))
}

fn timeout_failure() -> DiscoveryError {
    DiscoveryError::Failure(ClientFailure::new(
        FailureCategory::Timeout,
        "realm discovery",
        "realm discovery exceeded its configured deadline",
        RecoveryAction::RetryExplicitly,
    ))
}

fn entropy_failure() -> DiscoveryError {
    DiscoveryError::Failure(ClientFailure::new(
        FailureCategory::Configuration,
        "login authentication",
        "secure client entropy was unavailable",
        RecoveryAction::RestartClient,
    ))
}

fn internal_transition_failure() -> DiscoveryError {
    DiscoveryError::Failure(ClientFailure::new(
        FailureCategory::ProtocolIncompatibility,
        "realm discovery",
        "ordered login state transition failed closed",
        RecoveryAction::RestartClient,
    ))
}

fn io_failure(
    kind: io::ErrorKind,
    stage: &'static str,
    recovery: RecoveryAction,
) -> DiscoveryError {
    transport_failure(TransportKind::Login, kind, stage, recovery)
}

fn world_io_failure(
    kind: io::ErrorKind,
    stage: &'static str,
    recovery: RecoveryAction,
) -> DiscoveryError {
    transport_failure(TransportKind::World, kind, stage, recovery)
}

#[derive(Clone, Copy)]
enum TransportKind {
    Login,
    World,
}

fn transport_failure(
    transport: TransportKind,
    kind: io::ErrorKind,
    stage: &'static str,
    recovery: RecoveryAction,
) -> DiscoveryError {
    let (timed_out, failed) = match transport {
        TransportKind::Login => (
            "login transport operation timed out",
            "login transport operation failed",
        ),
        TransportKind::World => (
            "world transport operation timed out",
            "world transport operation failed",
        ),
    };
    let (category, context, recovery) =
        if matches!(kind, io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock) {
            (
                FailureCategory::Timeout,
                timed_out,
                RecoveryAction::RetryExplicitly,
            )
        } else {
            (FailureCategory::Transport, failed, recovery)
        };
    DiscoveryError::Failure(ClientFailure::new(category, stage, context, recovery))
}

fn protocol_failure(
    error: ProtocolError,
    stage: &'static str,
    recovery: RecoveryAction,
) -> DiscoveryError {
    map_protocol_failure(TransportKind::Login, error, stage, recovery)
}

fn world_protocol_failure(
    error: ProtocolError,
    stage: &'static str,
    recovery: RecoveryAction,
) -> DiscoveryError {
    map_protocol_failure(TransportKind::World, error, stage, recovery)
}

fn map_protocol_failure(
    transport: TransportKind,
    error: ProtocolError,
    stage: &'static str,
    recovery: RecoveryAction,
) -> DiscoveryError {
    match error {
        ProtocolError::Io(kind) => transport_failure(transport, kind, stage, recovery),
        ProtocolError::InvalidCredentialEncoding => DiscoveryError::Failure(ClientFailure::new(
            FailureCategory::Configuration,
            stage,
            match transport {
                TransportKind::Login => {
                    "credential encoding is incompatible with the login protocol"
                }
                TransportKind::World => {
                    "credential encoding is incompatible with the world protocol"
                }
            },
            RecoveryAction::FixConfiguration,
        )),
        ProtocolError::UnsupportedMovementState => unsupported_self_control_failure(),
        ProtocolError::MalformedWorldEntry { .. } => DiscoveryError::Failure(ClientFailure::new(
            FailureCategory::ProtocolIncompatibility,
            stage,
            "Reference Realm world-entry packet was malformed",
            recovery,
        )),
        ProtocolError::MalformedFrame
        | ProtocolError::UnsupportedSecurity
        | ProtocolError::InvalidSrpParameters => DiscoveryError::Failure(ClientFailure::new(
            FailureCategory::ProtocolIncompatibility,
            stage,
            match transport {
                TransportKind::Login => "Reference Realm login protocol response was incompatible",
                TransportKind::World => "Reference Realm world protocol response was incompatible",
            },
            recovery,
        )),
    }
}

struct TcpTransportFactory;

impl TransportFactory for TcpTransportFactory {
    type Transport = TcpLoginTransport;

    fn connect_login(&mut self, config: &ClientConfig) -> io::Result<Self::Transport> {
        connect_tcp(config.login_endpoint(), config)
    }

    fn connect_world(&mut self, config: &ClientConfig) -> io::Result<Self::Transport> {
        connect_tcp(config.world_endpoint(), config)
    }
}

fn connect_tcp(
    endpoint: std::net::SocketAddr,
    config: &ClientConfig,
) -> io::Result<TcpLoginTransport> {
    let stream = TcpStream::connect_timeout(&endpoint, config.connect_timeout())?;
    stream.set_read_timeout(Some(config.io_timeout()))?;
    stream.set_write_timeout(Some(config.io_timeout()))?;
    stream.set_nodelay(true)?;
    Ok(TcpLoginTransport(stream))
}

struct TcpLoginTransport(TcpStream);

impl Read for TcpLoginTransport {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.0.read(buffer)
    }
}

impl Write for TcpLoginTransport {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.0.write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl LoginTransport for TcpLoginTransport {
    fn close(&mut self) -> io::Result<()> {
        self.0.shutdown(Shutdown::Both)
    }
}

struct SystemClock {
    epoch: Instant,
}

impl SystemClock {
    fn new() -> Self {
        Self {
            epoch: Instant::now(),
        }
    }
}

impl MonotonicClock for SystemClock {
    fn now(&mut self) -> Duration {
        self.epoch.elapsed()
    }
}

struct SystemEntropy;

impl EntropySource for SystemEntropy {
    fn fill(&mut self, destination: &mut [u8]) -> Result<(), ()> {
        getrandom::fill(destination).map_err(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        io::{self, Cursor, Read, Write},
        net::{IpAddr, Ipv4Addr, SocketAddr},
        path::PathBuf,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        thread,
        time::Duration,
    };

    use crate::{
        ClientConfigSpec, ClientEvent, ClientEventKind, ClientPhase, ControlCommand,
        CredentialPaths, FailureCategory, SanitizedIdentity, boundary::new_boundary,
        config::CredentialMaterial,
    };
    use client_protocol::{
        CMSG_CHAR_ENUM, CMSG_FORCE_RUN_SPEED_CHANGE_ACK, CMSG_MOVE_SET_CAN_FLY_ACK,
        CMSG_PLAYER_LOGIN, CMSG_TIME_SYNC_RESP, HeaderCipher, HeaderDirection,
        LoginChallengeResponse, SMSG_AUTH_RESPONSE, SMSG_CHAR_ENUM, SMSG_COMPRESSED_UPDATE_OBJECT,
        SMSG_FORCE_RUN_SPEED_CHANGE, SMSG_LOGIN_VERIFY_WORLD, SMSG_MOVE_UNSET_CAN_FLY,
        SMSG_TIME_SYNC_REQ, SMSG_UPDATE_OBJECT, calculate_srp_client_proof,
        read_logon_challenge_response,
    };

    use super::{
        DiscoveryError, EntropySource, EntryAttemptOutcome, LoginTransport, MonotonicClock,
        TransportFactory, WorkerTarget, run_entry_attempt, run_worker_loop, run_worker_loop_for,
    };

    const PRIVATE_EPHEMERAL: [u8; 32] = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
        0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e,
        0x1f, 0x20,
    ];

    #[test]
    fn scripted_fragmented_success_reaches_the_final_boundary_in_order() {
        let script = success_script();
        let expected_writes = [
            fixture("login-challenge-request.hex"),
            fixture("login-proof-request.hex"),
            fixture("realm-request.hex"),
        ]
        .concat();
        let run = run_scripted(
            ControlCommand::StartEntry,
            script,
            1,
            None,
            FixedClock::default(),
            FixedEntropy::success(PRIVATE_EPHEMERAL),
            b"LEARNER",
            b"ONLYFORVECTOR",
        );

        let realm = run.snapshot.discovered_realm.as_ref().unwrap();
        assert_eq!(realm.realm_id(), 1);
        assert_eq!(realm.realm_name(), "Miazcore Reference Realm");
        assert_eq!(realm.client_build(), 12_340);
        assert_eq!(realm.endpoint(), "127.0.0.1:8085".parse().unwrap());
        assert_eq!(run.snapshot.phase, ClientPhase::Offline);
        assert_eq!(*run.state.writes.lock().unwrap(), expected_writes);
        assert!(run.state.closed.load(Ordering::Acquire));
        assert_eq!(run.state.connections.load(Ordering::Acquire), 1);

        let phases = run
            .events
            .iter()
            .filter_map(|event| match &event.kind {
                ClientEventKind::PhaseChanged { phase } => Some(phase.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            phases,
            [
                ClientPhase::Offline,
                ClientPhase::Entering(crate::EntryStage::LoginConnection),
                ClientPhase::Entering(crate::EntryStage::LoginAuthentication),
                ClientPhase::Entering(crate::EntryStage::RealmSelection),
                ClientPhase::Offline,
            ]
        );
        assert!(matches!(
            run.events.last().map(|event| &event.kind),
            Some(ClientEventKind::Disconnected)
        ));
    }

    #[test]
    fn fragmented_world_success_selects_exact_character_and_stops_before_player_login() {
        let run = run_character_scripted(
            world_success_script(),
            1,
            None,
            FixedClock::default(),
            FixedEntropy::success(PRIVATE_EPHEMERAL),
            config(),
        );

        let character = run.snapshot.selected_character.as_ref().unwrap();
        assert_eq!(character.guid(), 0x1234);
        assert_eq!(character.name(), "Miaztest");
        assert_eq!(character.level(), 80);
        for (actual, expected) in character.position().into_iter().zip([1.25, -2.5, 3.75]) {
            assert!((actual - expected).abs() < f32::EPSILON);
        }
        assert_eq!(run.snapshot.phase, ClientPhase::Offline);
        assert!(run.login_state.closed.load(Ordering::Acquire));
        assert!(run.world_state.closed.load(Ordering::Acquire));
        assert_eq!(run.login_state.connections.load(Ordering::Acquire), 1);
        assert_eq!(run.world_state.connections.load(Ordering::Acquire), 1);

        let phases = run
            .events
            .iter()
            .filter_map(|event| match &event.kind {
                ClientEventKind::PhaseChanged { phase } => Some(phase.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            phases,
            [
                ClientPhase::Offline,
                ClientPhase::Entering(crate::EntryStage::LoginConnection),
                ClientPhase::Entering(crate::EntryStage::LoginAuthentication),
                ClientPhase::Entering(crate::EntryStage::RealmSelection),
                ClientPhase::Entering(crate::EntryStage::WorldAuthentication),
                ClientPhase::Entering(crate::EntryStage::CharacterSelection),
                ClientPhase::Offline,
            ]
        );
        assert!(run.events.iter().any(|event| matches!(
            &event.kind,
            ClientEventKind::CharacterSelected { character } if character.name() == "Miaztest"
        )));
        let formatted = format!("{:?} {:?}", run.snapshot, run.events);
        assert!(!formatted.contains("ONLYFORVECTOR"));
        assert!(!formatted.contains("session_key"));

        let world_writes = run.world_state.writes.lock().unwrap();
        assert_eq!(world_writes.len(), 74 + 6);
        assert_eq!(
            u32::from_le_bytes(world_writes[2..6].try_into().unwrap()),
            0x01ed
        );
    }

    #[test]
    fn movement_ready_requires_self_pose_speed_and_both_control_acknowledgements() {
        let run = run_movement_scripted(movement_ready_success_script(), &config());

        assert_eq!(run.snapshot.phase, ClientPhase::Offline);
        assert_eq!(run.snapshot.entry_anchor.unwrap().map_id, 0);
        assert_f32(run.snapshot.entry_anchor.unwrap().east, 1.25);
        assert_f32(run.snapshot.run_speed.unwrap(), 8.5);
        assert_eq!(run.snapshot.queue_counters.movement_revision, 1);
        assert!(run.login_states[0].closed.load(Ordering::Acquire));
        assert!(run.world_states[0].closed.load(Ordering::Acquire));

        let phases = run
            .events
            .iter()
            .filter_map(|event| match &event.kind {
                ClientEventKind::PhaseChanged { phase } => Some(phase.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            phases,
            [
                ClientPhase::Offline,
                ClientPhase::Entering(crate::EntryStage::LoginConnection),
                ClientPhase::Entering(crate::EntryStage::LoginAuthentication),
                ClientPhase::Entering(crate::EntryStage::RealmSelection),
                ClientPhase::Entering(crate::EntryStage::WorldAuthentication),
                ClientPhase::Entering(crate::EntryStage::CharacterSelection),
                ClientPhase::Entering(crate::EntryStage::Bootstrap),
                ClientPhase::Entering(crate::EntryStage::ControlSynchronization),
                ClientPhase::MovementReady,
                ClientPhase::Offline,
            ]
        );
        assert!(run.events.iter().any(|event| matches!(
            event.kind,
            ClientEventKind::PoseObserved {
                source: crate::PoseSource::EntryObservation,
                ..
            }
        )));

        let frames = decode_client_frames(&run.world_states[0].writes.lock().unwrap()[74..]);
        assert_eq!(
            frames.iter().map(|(opcode, _)| *opcode).collect::<Vec<_>>(),
            [
                CMSG_CHAR_ENUM,
                CMSG_PLAYER_LOGIN,
                CMSG_FORCE_RUN_SPEED_CHANGE_ACK,
                CMSG_TIME_SYNC_RESP,
                CMSG_MOVE_SET_CAN_FLY_ACK,
            ]
        );
        assert_eq!(frames[1].1, fixture("world-entry-player-login-body.hex"));

        let mut expected_speed = fixture("world-entry-force-run-ack-body.hex");
        expected_speed[13..17].fill(0);
        assert_eq!(frames[2].1, expected_speed);
        let mut expected_time = fixture("world-entry-time-sync-response-body.hex");
        expected_time[4..].fill(0);
        assert_eq!(frames[3].1, expected_time);
        let mut expected_flight = fixture("world-entry-unset-can-fly-ack-body.hex");
        expected_flight[18..22].fill(0);
        assert_eq!(frames[4].1, expected_flight);
    }

    #[test]
    fn movement_ready_fails_closed_at_each_authoritative_or_sync_boundary() {
        let mut bad_pose = fixture("world-entry-login-verify-body.hex");
        bad_pose[4..8].copy_from_slice(&99.0_f32.to_le_bytes());
        let pose = run_movement_scripted(
            movement_entry_script(&[(SMSG_LOGIN_VERIFY_WORLD, bad_pose)]),
            &config(),
        );
        assert_failed_movement(
            &pose,
            ExpectedMovementFailure {
                category: FailureCategory::ProtocolIncompatibility,
                stage: "world bootstrap",
                context: "authoritative entry state did not corroborate the selected character",
                recovery: crate::RecoveryAction::CheckReferenceRealm,
            },
        );

        let mut malformed = fixture("world-entry-self-update-compressed-body.hex");
        malformed.pop();
        let compressed = run_movement_scripted(
            movement_entry_script(&[
                (
                    SMSG_LOGIN_VERIFY_WORLD,
                    fixture("world-entry-login-verify-body.hex"),
                ),
                (SMSG_COMPRESSED_UPDATE_OBJECT, malformed),
            ]),
            &config(),
        );
        assert_failed_movement(
            &compressed,
            ExpectedMovementFailure::malformed("world bootstrap"),
        );

        let unsupported = run_movement_scripted(
            movement_entry_script(&[
                (
                    SMSG_LOGIN_VERIFY_WORLD,
                    fixture("world-entry-login-verify-body.hex"),
                ),
                (
                    SMSG_UPDATE_OBJECT,
                    fixture("world-entry-self-update-body.hex"),
                ),
                (0x00e8, fixture("world-entry-unset-can-fly-body.hex")),
            ]),
            &config(),
        );
        assert_failed_movement(
            &unsupported,
            ExpectedMovementFailure {
                category: FailureCategory::UnsupportedSelfControl,
                stage: "world bootstrap",
                context: "selected character entered an unsupported movement or control state",
                recovery: crate::RecoveryAction::CheckReferenceRealm,
            },
        );

        let missing_flight = run_movement_scripted(
            movement_entry_script(&[
                (
                    SMSG_LOGIN_VERIFY_WORLD,
                    fixture("world-entry-login-verify-body.hex"),
                ),
                (
                    SMSG_UPDATE_OBJECT,
                    fixture("world-entry-self-update-body.hex"),
                ),
                (
                    SMSG_TIME_SYNC_REQ,
                    fixture("world-entry-time-sync-request-body.hex"),
                ),
            ]),
            &config(),
        );
        assert_failed_movement(
            &missing_flight,
            ExpectedMovementFailure {
                category: FailureCategory::Transport,
                stage: "control synchronization",
                context: "world transport operation failed",
                recovery: crate::RecoveryAction::CheckReferenceRealm,
            },
        );
    }

    #[test]
    fn malformed_identity_speed_time_and_flight_boundaries_are_stable_and_redacted() {
        let mut truncated_location = fixture("world-entry-login-verify-body.hex");
        truncated_location.pop();
        let location = run_movement_scripted(
            movement_entry_script(&[(SMSG_LOGIN_VERIFY_WORLD, truncated_location)]),
            &config(),
        );
        assert_failed_movement(
            &location,
            ExpectedMovementFailure::malformed("world bootstrap"),
        );

        let mut mismatched_self = fixture("world-entry-self-update-body.hex");
        mismatched_self[6] = 0x35;
        let identity = run_movement_scripted(
            movement_entry_script(&[
                (
                    SMSG_LOGIN_VERIFY_WORLD,
                    fixture("world-entry-login-verify-body.hex"),
                ),
                (SMSG_UPDATE_OBJECT, mismatched_self),
            ]),
            &config(),
        );
        assert_failed_movement(
            &identity,
            ExpectedMovementFailure::malformed("world bootstrap"),
        );

        let absent_self = run_movement_scripted(
            movement_entry_script(&[
                (
                    SMSG_LOGIN_VERIFY_WORLD,
                    fixture("world-entry-login-verify-body.hex"),
                ),
                (SMSG_UPDATE_OBJECT, 0_u32.to_le_bytes().to_vec()),
            ]),
            &config(),
        );
        assert_failed_movement(
            &absent_self,
            ExpectedMovementFailure {
                category: FailureCategory::Transport,
                stage: "world bootstrap",
                context: "world transport operation failed",
                recovery: crate::RecoveryAction::CheckReferenceRealm,
            },
        );

        let mut malformed_speed = fixture("world-entry-force-run-body.hex");
        malformed_speed[7] = 1;
        let speed = run_movement_scripted(
            synchronized_entry_script(&[(SMSG_FORCE_RUN_SPEED_CHANGE, malformed_speed)]),
            &config(),
        );
        assert_failed_movement(
            &speed,
            ExpectedMovementFailure::malformed("control synchronization"),
        );

        let time = run_movement_scripted(
            synchronized_entry_script(&[(SMSG_TIME_SYNC_REQ, vec![1, 2, 3])]),
            &config(),
        );
        assert_failed_movement(
            &time,
            ExpectedMovementFailure::malformed("control synchronization"),
        );

        let flight = run_movement_scripted(
            synchronized_entry_script(&[(SMSG_MOVE_UNSET_CAN_FLY, vec![1, 2, 3])]),
            &config(),
        );
        assert_failed_movement(
            &flight,
            ExpectedMovementFailure::malformed("control synchronization"),
        );
    }

    #[test]
    fn synchronization_timeout_and_every_control_write_fault_fail_closed() {
        let timeout = run_faulted_movement(
            synchronized_entry_script(&[]),
            Some(io::ErrorKind::TimedOut),
            None,
        );
        assert_failed_movement(
            &timeout,
            ExpectedMovementFailure {
                category: FailureCategory::Timeout,
                stage: "control synchronization",
                context: "world transport operation timed out",
                recovery: crate::RecoveryAction::RetryExplicitly,
            },
        );

        for frames in [
            vec![(
                SMSG_FORCE_RUN_SPEED_CHANGE,
                fixture("world-entry-force-run-body.hex"),
            )],
            vec![(
                SMSG_TIME_SYNC_REQ,
                fixture("world-entry-time-sync-request-body.hex"),
            )],
            vec![(
                SMSG_MOVE_UNSET_CAN_FLY,
                fixture("world-entry-unset-can-fly-body.hex"),
            )],
        ] {
            let write = run_faulted_movement(synchronized_entry_script(&frames), None, Some(4));
            assert_failed_movement(
                &write,
                ExpectedMovementFailure {
                    category: FailureCategory::Transport,
                    stage: "control synchronization",
                    context: "world transport operation failed",
                    recovery: crate::RecoveryAction::CheckReferenceRealm,
                },
            );
        }
    }

    #[test]
    fn explicit_retry_recreates_login_world_cipher_time_and_entropy_state() {
        let first = movement_entry_script(&[(SMSG_LOGIN_VERIFY_WORLD, {
            let mut value = fixture("world-entry-login-verify-body.hex");
            value[4..8].copy_from_slice(&99.0_f32.to_le_bytes());
            value
        })]);
        let second = movement_ready_success_script();
        let config = config();
        let credentials = CredentialMaterial::synthetic(b"LEARNER", b"ONLYFORVECTOR");
        let (client, mut boundary) = new_boundary(config.identity().clone()).unwrap();
        client.send_control(ControlCommand::StartEntry).unwrap();
        client.send_control(ControlCommand::RetryEntry).unwrap();
        client.publish_movement_intent(crate::MovementIntent::planar(1.0, 0.0).unwrap());
        let mut factory = RetryScriptFactory::new([first, second]);
        let entropy_calls = Arc::new(AtomicUsize::new(0));
        let mut entropy = CountingEntropy {
            calls: Arc::clone(&entropy_calls),
        };
        let mut clock = FixedClock::sequence((0..256).map(Duration::from_millis));

        run_worker_loop_for(
            &config,
            &credentials,
            &mut boundary,
            &mut factory,
            &mut clock,
            &mut entropy,
            WorkerTarget::MovementReady,
        );
        boundary.mark_stopped();

        let snapshot = client.snapshot();
        let events = client.drain_events();
        assert_eq!(snapshot.phase, ClientPhase::Offline);
        assert!(snapshot.latest_failure.is_none());
        assert_f32(snapshot.run_speed.unwrap(), 8.5);
        assert_eq!(entropy_calls.load(Ordering::Acquire), 4);
        assert_eq!(factory.login_states.len(), 2);
        assert_eq!(factory.world_states.len(), 2);
        assert!(
            factory
                .login_states
                .iter()
                .all(|state| state.closed.load(Ordering::Acquire))
        );
        assert!(
            factory
                .world_states
                .iter()
                .all(|state| state.closed.load(Ordering::Acquire))
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(
                    event.kind,
                    ClientEventKind::PhaseChanged {
                        phase: ClientPhase::Entering(crate::EntryStage::LoginConnection)
                    }
                ))
                .count(),
            2
        );
        assert!(events.iter().any(|event| matches!(
            event.kind,
            ClientEventKind::CommandRejected {
                command: crate::CommandKind::StartEntry,
                ..
            }
        )));
        assert!(!events.iter().any(|event| matches!(
            event.kind,
            ClientEventKind::CommandRejected {
                command: crate::CommandKind::RetryEntry,
                ..
            }
        )));
    }

    #[test]
    fn shutdown_during_bootstrap_or_control_sync_sends_no_movement_and_closes_both_sockets() {
        let before_entry = world_script(&[
            (SMSG_AUTH_RESPONSE, accepted_world_auth()),
            (SMSG_CHAR_ENUM, fixture("character-enum-body.hex")),
        ])
        .len();
        let through_self = movement_entry_script(&[
            (
                SMSG_LOGIN_VERIFY_WORLD,
                fixture("world-entry-login-verify-body.hex"),
            ),
            (
                SMSG_UPDATE_OBJECT,
                fixture("world-entry-self-update-body.hex"),
            ),
        ])
        .len();

        for (cancel_after, expected_stage) in [
            (before_entry, crate::EntryStage::Bootstrap),
            (through_self, crate::EntryStage::ControlSynchronization),
        ] {
            let config = config();
            let credentials = CredentialMaterial::synthetic(b"LEARNER", b"ONLYFORVECTOR");
            let (client, mut boundary) = new_boundary(config.identity().clone()).unwrap();
            let client = Arc::new(client);
            client.send_control(ControlCommand::StartEntry).unwrap();
            client.publish_movement_intent(crate::MovementIntent::planar(1.0, 0.0).unwrap());
            let login_state = ScriptState::default();
            let world_state = ScriptState::default();
            let mut factory = CancellingEntryFactory {
                client: Arc::clone(&client),
                world_reads: Some(movement_ready_success_script()),
                cancel_after,
                login_state: login_state.clone(),
                world_state: world_state.clone(),
            };

            run_worker_loop_for(
                &config,
                &credentials,
                &mut boundary,
                &mut factory,
                &mut FixedClock::default(),
                &mut FixedEntropy::success(PRIVATE_EPHEMERAL),
                WorkerTarget::MovementReady,
            );
            boundary.mark_stopped();

            assert_eq!(client.snapshot().phase, ClientPhase::Offline);
            assert_eq!(client.snapshot().queue_counters.movement_revision, 1);
            assert!(login_state.closed.load(Ordering::Acquire));
            assert!(world_state.closed.load(Ordering::Acquire));
            let events = client.drain_events();
            assert!(events.iter().any(|event| matches!(
                event.kind,
                ClientEventKind::PhaseChanged {
                    phase: ClientPhase::Entering(stage)
                } if stage == expected_stage
            )));
            assert!(!events.iter().any(|event| matches!(
                event.kind,
                ClientEventKind::MovementSubmitted { .. }
                    | ClientEventKind::PhaseChanged {
                        phase: ClientPhase::MovementReady
                    }
            )));
            assert!(matches!(
                events.last().map(|event| &event.kind),
                Some(ClientEventKind::Disconnected)
            ));
        }
    }

    #[test]
    fn world_auth_rejection_and_character_identity_drift_are_stable() {
        let rejected = run_character_scripted(
            world_script(&[(SMSG_AUTH_RESPONSE, vec![0x0d])]),
            usize::MAX,
            None,
            FixedClock::default(),
            FixedEntropy::success(PRIVATE_EPHEMERAL),
            config(),
        );
        assert_failed_character(&rejected, FailureCategory::Authentication);
        assert_eq!(
            rejected.snapshot.latest_failure.as_ref().unwrap().context(),
            "world session authentication was rejected"
        );

        let absent = run_character_scripted(
            world_script(&[
                (SMSG_AUTH_RESPONSE, accepted_world_auth()),
                (SMSG_CHAR_ENUM, vec![0]),
            ]),
            usize::MAX,
            None,
            FixedClock::default(),
            FixedEntropy::success(PRIVATE_EPHEMERAL),
            config(),
        );
        assert_failed_character(&absent, FailureCategory::Configuration);
        assert_eq!(
            absent.snapshot.latest_failure.as_ref().unwrap().context(),
            "configured character was absent from the authenticated realm"
        );

        let record = fixture("character-enum-body.hex");
        let mut duplicate = Vec::with_capacity(record.len() * 2);
        duplicate.push(2);
        duplicate.extend_from_slice(&record[1..]);
        duplicate.extend_from_slice(&record[1..]);
        let duplicate = run_character_scripted(
            world_script(&[
                (SMSG_AUTH_RESPONSE, accepted_world_auth()),
                (SMSG_CHAR_ENUM, duplicate),
            ]),
            usize::MAX,
            None,
            FixedClock::default(),
            FixedEntropy::success(PRIVATE_EPHEMERAL),
            config(),
        );
        assert_failed_character(&duplicate, FailureCategory::ProtocolIncompatibility);
    }

    #[test]
    fn world_cipher_drift_malformed_eof_and_timeout_fail_closed() {
        let drifted = run_character_scripted(
            world_script_with_invalid_encrypted_header(),
            usize::MAX,
            None,
            FixedClock::default(),
            FixedEntropy::success(PRIVATE_EPHEMERAL),
            config(),
        );
        assert_failed_character(&drifted, FailureCategory::ProtocolIncompatibility);

        let mut malformed_challenge = fixture("world-auth-challenge-frame.hex");
        malformed_challenge[4..8].fill(0);
        let malformed = run_character_scripted(
            malformed_challenge,
            usize::MAX,
            None,
            FixedClock::default(),
            FixedEntropy::success(PRIVATE_EPHEMERAL),
            config(),
        );
        assert_failed_character(&malformed, FailureCategory::ProtocolIncompatibility);

        let mut eof = world_success_script();
        eof.pop();
        let eof = run_character_scripted(
            eof,
            usize::MAX,
            None,
            FixedClock::default(),
            FixedEntropy::success(PRIVATE_EPHEMERAL),
            config(),
        );
        assert_failed_character(&eof, FailureCategory::Transport);

        let timeout = run_character_scripted(
            Vec::new(),
            usize::MAX,
            Some(io::ErrorKind::TimedOut),
            FixedClock::default(),
            FixedEntropy::success(PRIVATE_EPHEMERAL),
            config(),
        );
        assert_failed_character(&timeout, FailureCategory::Timeout);
    }

    #[test]
    fn shutdown_during_world_authentication_closes_both_sessions_before_character_request() {
        let config = config();
        let credentials = CredentialMaterial::synthetic(b"LEARNER", b"ONLYFORVECTOR");
        let (client, mut boundary) = new_boundary(config.identity().clone()).unwrap();
        let client = Arc::new(client);
        client.send_control(ControlCommand::StartEntry).unwrap();
        let login_state = ScriptState::default();
        let world_state = ScriptState::default();
        let mut factory = CharacterScriptFactory {
            login_reads: Some(success_script()),
            world_reads: Some(world_success_script()),
            max_read: usize::MAX,
            world_connect_error: None,
            login_state: login_state.clone(),
            world_state: world_state.clone(),
        };
        let cancelling_client = Arc::clone(&client);
        let worker = thread::spawn(move || {
            run_worker_loop_for(
                &config,
                &credentials,
                &mut boundary,
                &mut factory,
                &mut FixedClock::default(),
                &mut CancellingWorldEntropy {
                    client: cancelling_client,
                    calls: 0,
                },
                WorkerTarget::CharacterSelection,
            );
            boundary.mark_stopped();
        });
        worker.join().unwrap();

        assert!(login_state.closed.load(Ordering::Acquire));
        assert!(world_state.closed.load(Ordering::Acquire));
        assert_eq!(world_state.writes.lock().unwrap().len(), 0);
        assert_eq!(client.snapshot().phase, ClientPhase::Offline);
        assert_eq!(client.snapshot().queue_counters.control_queued, 0);
        assert!(matches!(
            client.drain_events().last().map(|event| &event.kind),
            Some(ClientEventKind::Disconnected)
        ));
    }

    #[test]
    fn authentication_rejection_is_redacted_and_closes_the_transport() {
        let mut script = fixture("login-challenge-response.hex");
        script.extend_from_slice(&[0x01, 0x04]);
        let run = run_scripted(
            ControlCommand::StartEntry,
            script,
            usize::MAX,
            None,
            FixedClock::default(),
            FixedEntropy::success(PRIVATE_EPHEMERAL),
            b"LEARNER",
            b"ONLYFORVECTOR",
        );

        assert_failed(&run, FailureCategory::Authentication);
        assert_eq!(
            run.snapshot.latest_failure.as_ref().unwrap().context(),
            "fixture account authentication was rejected"
        );
        assert!(run.state.closed.load(Ordering::Acquire));
    }

    #[test]
    fn accepted_login_build_allows_omitted_tuple_but_rejects_an_explicit_mismatch() {
        let mut omitted = fixture("realm-response.hex");
        omitted[11] = 0;
        let tuple_start = omitted.len() - 7;
        omitted.drain(tuple_start..tuple_start + 5);
        let payload_len = u16::try_from(omitted.len() - 3).unwrap();
        omitted[1..3].copy_from_slice(&payload_len.to_le_bytes());
        let run = run_scripted(
            ControlCommand::StartEntry,
            [
                fixture("login-challenge-response.hex"),
                fixture("login-proof-response.hex"),
                omitted,
            ]
            .concat(),
            usize::MAX,
            None,
            FixedClock::default(),
            FixedEntropy::success(PRIVATE_EPHEMERAL),
            b"LEARNER",
            b"ONLYFORVECTOR",
        );
        assert_eq!(
            run.snapshot
                .discovered_realm
                .as_ref()
                .unwrap()
                .client_build(),
            12_340
        );

        let mut mismatched = fixture("realm-response.hex");
        let build_offset = mismatched.len() - 4;
        mismatched[build_offset..build_offset + 2].copy_from_slice(&12_341_u16.to_le_bytes());
        let run = run_scripted(
            ControlCommand::StartEntry,
            [
                fixture("login-challenge-response.hex"),
                fixture("login-proof-response.hex"),
                mismatched,
            ]
            .concat(),
            usize::MAX,
            None,
            FixedClock::default(),
            FixedEntropy::success(PRIVATE_EPHEMERAL),
            b"LEARNER",
            b"ONLYFORVECTOR",
        );
        assert_failed(&run, FailureCategory::ProtocolIncompatibility);
    }

    #[test]
    fn transport_and_clock_timeouts_are_deterministic_without_sleeping() {
        let connect_timeout = run_scripted(
            ControlCommand::StartEntry,
            Vec::new(),
            usize::MAX,
            Some(io::ErrorKind::TimedOut),
            FixedClock::default(),
            FixedEntropy::success(PRIVATE_EPHEMERAL),
            b"LEARNER",
            b"ONLYFORVECTOR",
        );
        assert_failed(&connect_timeout, FailureCategory::Timeout);
        assert_eq!(connect_timeout.state.connections.load(Ordering::Acquire), 1);

        let deadline = run_scripted(
            ControlCommand::StartEntry,
            Vec::new(),
            usize::MAX,
            None,
            FixedClock::sequence([Duration::ZERO, Duration::from_secs(30)]),
            FixedEntropy::success(PRIVATE_EPHEMERAL),
            b"LEARNER",
            b"ONLYFORVECTOR",
        );
        assert_failed(&deadline, FailureCategory::Timeout);
        assert!(deadline.state.closed.load(Ordering::Acquire));
    }

    #[test]
    fn malformed_frames_and_entropy_failure_close_and_stop_the_worker() {
        let malformed = run_scripted(
            ControlCommand::StartEntry,
            vec![0xff, 0, 0],
            usize::MAX,
            None,
            FixedClock::default(),
            FixedEntropy::success(PRIVATE_EPHEMERAL),
            b"LEARNER",
            b"ONLYFORVECTOR",
        );
        assert_failed(&malformed, FailureCategory::ProtocolIncompatibility);
        assert!(malformed.state.closed.load(Ordering::Acquire));

        let entropy = run_scripted(
            ControlCommand::StartEntry,
            fixture("login-challenge-response.hex"),
            usize::MAX,
            None,
            FixedClock::default(),
            FixedEntropy::failure(),
            b"LEARNER",
            b"ONLYFORVECTOR",
        );
        assert_failed(&entropy, FailureCategory::Configuration);
        assert!(entropy.state.closed.load(Ordering::Acquire));
    }

    #[test]
    fn disconnect_before_start_is_orderly_and_opens_no_transport() {
        let run = run_scripted(
            ControlCommand::Disconnect,
            Vec::new(),
            usize::MAX,
            None,
            FixedClock::default(),
            FixedEntropy::success(PRIVATE_EPHEMERAL),
            b"LEARNER",
            b"ONLYFORVECTOR",
        );
        assert_eq!(run.snapshot.phase, ClientPhase::Offline);
        assert_eq!(run.state.connections.load(Ordering::Acquire), 0);
        assert!(matches!(
            run.events.last().map(|event| &event.kind),
            Some(ClientEventKind::Disconnected)
        ));
    }

    #[test]
    fn shutdown_during_authentication_stops_before_proof_and_closes_transport() {
        let config = config();
        let credentials = CredentialMaterial::synthetic(b"LEARNER", b"ONLYFORVECTOR");
        let (client, mut boundary) = new_boundary(config.identity().clone()).unwrap();
        let client = Arc::new(client);
        client.send_control(ControlCommand::StartEntry).unwrap();
        let state = ScriptState::default();
        let mut factory = ScriptFactory {
            reads: Some(success_script()),
            max_read: usize::MAX,
            connect_error: None,
            state: state.clone(),
        };
        let cancelling_client = Arc::clone(&client);
        let worker = thread::spawn(move || {
            run_worker_loop(
                &config,
                &credentials,
                &mut boundary,
                &mut factory,
                &mut FixedClock::default(),
                &mut CancellingEntropy {
                    client: cancelling_client,
                    value: PRIVATE_EPHEMERAL,
                },
            );
            boundary.mark_stopped();
        });
        worker.join().unwrap();

        assert_eq!(
            *state.writes.lock().unwrap(),
            fixture("login-challenge-request.hex")
        );
        assert!(state.closed.load(Ordering::Acquire));
        assert_eq!(client.snapshot().phase, ClientPhase::Offline);
        assert_eq!(client.snapshot().queue_counters.control_queued, 0);
        assert!(matches!(
            client.drain_events().last().map(|event| &event.kind),
            Some(ClientEventKind::Disconnected)
        ));
    }

    #[test]
    fn public_failure_and_evidence_formats_cannot_reveal_credentials_or_session_material() {
        let account = b"NEVERPRINTACCOUNT";
        let password = b"NEVERPRINTPASSWORD";
        let run = run_scripted(
            ControlCommand::StartEntry,
            fixture("login-challenge-response.hex"),
            usize::MAX,
            None,
            FixedClock::default(),
            FixedEntropy::success(PRIVATE_EPHEMERAL),
            account,
            password,
        );
        let formatted = format!("{:?} {:?}", run.snapshot, run.events);
        assert!(!formatted.contains(std::str::from_utf8(account).unwrap()));
        assert!(!formatted.contains(std::str::from_utf8(password).unwrap()));
        assert!(!formatted.contains("session_key"));
    }

    fn assert_failed(run: &ScriptedRun, category: FailureCategory) {
        assert!(matches!(run.snapshot.phase, ClientPhase::Failed(_)));
        assert_eq!(
            run.snapshot.latest_failure.as_ref().unwrap().category(),
            category
        );
        assert!(
            run.events
                .iter()
                .any(|event| matches!(event.kind, ClientEventKind::CommandRejected { .. }))
        );
        assert!(matches!(
            run.events.last().map(|event| &event.kind),
            Some(ClientEventKind::Disconnected)
        ));
    }

    fn success_script() -> Vec<u8> {
        [
            fixture("login-challenge-response.hex"),
            fixture("login-proof-response.hex"),
            fixture("realm-response.hex"),
        ]
        .concat()
    }

    fn accepted_world_auth() -> Vec<u8> {
        vec![0x0c, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    }

    fn world_success_script() -> Vec<u8> {
        world_script(&[
            (SMSG_AUTH_RESPONSE, accepted_world_auth()),
            (0x0222, vec![0xaa, 0xbb, 0xcc]),
            (SMSG_CHAR_ENUM, fixture("character-enum-body.hex")),
        ])
    }

    fn movement_ready_success_script() -> Vec<u8> {
        movement_entry_script(&[
            (
                SMSG_LOGIN_VERIFY_WORLD,
                fixture("world-entry-login-verify-body.hex"),
            ),
            (
                SMSG_UPDATE_OBJECT,
                fixture("world-entry-self-update-body.hex"),
            ),
            (
                SMSG_FORCE_RUN_SPEED_CHANGE,
                fixture("world-entry-force-run-body.hex"),
            ),
            (
                SMSG_TIME_SYNC_REQ,
                fixture("world-entry-time-sync-request-body.hex"),
            ),
            (
                SMSG_MOVE_UNSET_CAN_FLY,
                fixture("world-entry-unset-can-fly-body.hex"),
            ),
        ])
    }

    fn movement_entry_script(entry_frames: &[(u16, Vec<u8>)]) -> Vec<u8> {
        let mut frames = vec![
            (SMSG_AUTH_RESPONSE, accepted_world_auth()),
            (SMSG_CHAR_ENUM, fixture("character-enum-body.hex")),
        ];
        frames.extend_from_slice(entry_frames);
        world_script(&frames)
    }

    fn synchronized_entry_script(sync_frames: &[(u16, Vec<u8>)]) -> Vec<u8> {
        let mut frames = vec![
            (
                SMSG_LOGIN_VERIFY_WORLD,
                fixture("world-entry-login-verify-body.hex"),
            ),
            (
                SMSG_UPDATE_OBJECT,
                fixture("world-entry-self-update-body.hex"),
            ),
        ];
        frames.extend_from_slice(sync_frames);
        movement_entry_script(&frames)
    }

    fn world_script(frames: &[(u16, Vec<u8>)]) -> Vec<u8> {
        let session_key = login_session_key();
        let mut cipher = HeaderCipher::new(HeaderDirection::ServerToClient, &session_key);
        let mut script = fixture("world-auth-challenge-frame.hex");
        for (opcode, payload) in frames {
            let size = u16::try_from(payload.len() + 2).unwrap();
            let mut header = [0_u8; 4];
            header[..2].copy_from_slice(&size.to_be_bytes());
            header[2..].copy_from_slice(&opcode.to_le_bytes());
            cipher.apply(&mut header);
            script.extend_from_slice(&header);
            script.extend_from_slice(payload);
        }
        script
    }

    fn world_script_with_invalid_encrypted_header() -> Vec<u8> {
        let session_key = login_session_key();
        let mut cipher = HeaderCipher::new(HeaderDirection::ServerToClient, &session_key);
        let mut invalid_header = [0_u8, 1, 0xee, 0x01];
        cipher.apply(&mut invalid_header);
        [
            fixture("world-auth-challenge-frame.hex"),
            invalid_header.to_vec(),
        ]
        .concat()
    }

    fn login_session_key() -> [u8; 40] {
        let challenge = match read_logon_challenge_response(&mut Cursor::new(fixture(
            "login-challenge-response.hex",
        )))
        .unwrap()
        {
            LoginChallengeResponse::Accepted(challenge) => challenge,
            LoginChallengeResponse::Rejected { .. } => panic!("synthetic challenge rejected"),
        };
        let proof =
            calculate_srp_client_proof(b"LEARNER", b"ONLYFORVECTOR", &challenge, PRIVATE_EPHEMERAL)
                .unwrap();
        *proof.session_key().as_bytes()
    }

    fn decode_client_frames(bytes: &[u8]) -> Vec<(u32, Vec<u8>)> {
        let mut cipher = HeaderCipher::new(HeaderDirection::ClientToServer, &login_session_key());
        let mut cursor = Cursor::new(bytes);
        let mut frames = Vec::new();
        while usize::try_from(cursor.position()).unwrap() < bytes.len() {
            let mut header = [0_u8; 6];
            cursor.read_exact(&mut header).unwrap();
            cipher.apply(&mut header);
            let size = usize::from(u16::from_be_bytes(header[..2].try_into().unwrap()));
            assert!(size >= 4);
            let opcode = u32::from_le_bytes(header[2..].try_into().unwrap());
            let mut payload = vec![0_u8; size - 4];
            cursor.read_exact(&mut payload).unwrap();
            frames.push((opcode, payload));
        }
        frames
    }

    fn fixture(name: &str) -> Vec<u8> {
        let value = match name {
            "login-challenge-request.hex" => {
                include_str!("../../client_protocol/tests/fixtures/v1/login-challenge-request.hex")
            }
            "login-challenge-response.hex" => {
                include_str!("../../client_protocol/tests/fixtures/v1/login-challenge-response.hex")
            }
            "login-proof-request.hex" => {
                include_str!("../../client_protocol/tests/fixtures/v1/login-proof-request.hex")
            }
            "login-proof-response.hex" => {
                include_str!("../../client_protocol/tests/fixtures/v1/login-proof-response.hex")
            }
            "realm-request.hex" => {
                include_str!("../../client_protocol/tests/fixtures/v1/realm-request.hex")
            }
            "realm-response.hex" => {
                include_str!("../../client_protocol/tests/fixtures/v1/realm-response.hex")
            }
            "world-auth-challenge-frame.hex" => include_str!(
                "../../client_protocol/tests/fixtures/v1/world-auth-challenge-frame.hex"
            ),
            "character-enum-body.hex" => {
                include_str!("../../client_protocol/tests/fixtures/v1/character-enum-body.hex")
            }
            "world-entry-login-verify-body.hex" => include_str!(
                "../../client_protocol/tests/fixtures/v1/world-entry-login-verify-body.hex"
            ),
            "world-entry-player-login-body.hex" => include_str!(
                "../../client_protocol/tests/fixtures/v1/world-entry-player-login-body.hex"
            ),
            "world-entry-self-update-body.hex" => include_str!(
                "../../client_protocol/tests/fixtures/v1/world-entry-self-update-body.hex"
            ),
            "world-entry-self-update-compressed-body.hex" => include_str!(
                "../../client_protocol/tests/fixtures/v1/world-entry-self-update-compressed-body.hex"
            ),
            "world-entry-force-run-body.hex" => include_str!(
                "../../client_protocol/tests/fixtures/v1/world-entry-force-run-body.hex"
            ),
            "world-entry-force-run-ack-body.hex" => include_str!(
                "../../client_protocol/tests/fixtures/v1/world-entry-force-run-ack-body.hex"
            ),
            "world-entry-unset-can-fly-body.hex" => include_str!(
                "../../client_protocol/tests/fixtures/v1/world-entry-unset-can-fly-body.hex"
            ),
            "world-entry-unset-can-fly-ack-body.hex" => include_str!(
                "../../client_protocol/tests/fixtures/v1/world-entry-unset-can-fly-ack-body.hex"
            ),
            "world-entry-time-sync-request-body.hex" => include_str!(
                "../../client_protocol/tests/fixtures/v1/world-entry-time-sync-request-body.hex"
            ),
            "world-entry-time-sync-response-body.hex" => include_str!(
                "../../client_protocol/tests/fixtures/v1/world-entry-time-sync-response-body.hex"
            ),
            _ => panic!("unknown fixture"),
        };
        value
            .split_whitespace()
            .collect::<String>()
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    struct ScriptedRun {
        snapshot: crate::ClientSnapshot,
        events: Vec<ClientEvent>,
        state: ScriptState,
    }

    struct CharacterScriptedRun {
        snapshot: crate::ClientSnapshot,
        events: Vec<ClientEvent>,
        login_state: ScriptState,
        world_state: ScriptState,
    }

    struct MovementScriptedRun {
        snapshot: crate::ClientSnapshot,
        events: Vec<ClientEvent>,
        login_states: Vec<ScriptState>,
        world_states: Vec<ScriptState>,
    }

    fn assert_f32(actual: f32, expected: f32) {
        assert_eq!(actual.to_bits(), expected.to_bits());
    }

    #[derive(Clone, Copy)]
    struct ExpectedMovementFailure {
        category: FailureCategory,
        stage: &'static str,
        context: &'static str,
        recovery: crate::RecoveryAction,
    }

    impl ExpectedMovementFailure {
        const fn malformed(stage: &'static str) -> Self {
            Self {
                category: FailureCategory::ProtocolIncompatibility,
                stage,
                context: "Reference Realm world-entry packet was malformed",
                recovery: crate::RecoveryAction::CheckReferenceRealm,
            }
        }
    }

    fn assert_failed_movement(run: &MovementScriptedRun, expected: ExpectedMovementFailure) {
        assert!(matches!(run.snapshot.phase, ClientPhase::Failed(_)));
        let failure = run.snapshot.latest_failure.as_ref().unwrap();
        assert_eq!(
            (
                failure.category(),
                failure.stage(),
                failure.context(),
                failure.recommended_recovery()
            ),
            (
                expected.category,
                expected.stage,
                expected.context,
                expected.recovery
            )
        );
        assert!(run.snapshot.run_speed.is_none());
        assert_eq!(run.snapshot.queue_counters.movement_revision, 1);
        assert!(
            run.snapshot
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message() == expected.context)
        );
        assert_eq!(
            run.events
                .iter()
                .filter(|event| matches!(
                    event.kind,
                    ClientEventKind::CommandRejected {
                        command: crate::CommandKind::StartEntry,
                        ..
                    }
                ))
                .count(),
            1
        );
        assert_eq!(
            run.events
                .iter()
                .filter(|event| matches!(
                    event.kind,
                    ClientEventKind::PhaseChanged {
                        phase: ClientPhase::Entering(crate::EntryStage::LoginConnection)
                    }
                ))
                .count(),
            1
        );
        assert!(!run.events.iter().any(|event| matches!(
            event.kind,
            ClientEventKind::MovementSubmitted { .. }
                | ClientEventKind::PhaseChanged {
                    phase: ClientPhase::MovementReady
                }
        )));
        assert!(run.login_states[0].closed.load(Ordering::Acquire));
        assert!(run.world_states[0].closed.load(Ordering::Acquire));
        assert_eq!(run.login_states.len(), 1);
        assert_eq!(run.world_states.len(), 1);
        let formatted = format!("{:?} {:?}", run.snapshot, run.events);
        assert!(!formatted.contains("ONLYFORVECTOR"));
        assert!(!formatted.contains("session_key"));
        assert!(matches!(
            run.events.last().map(|event| &event.kind),
            Some(ClientEventKind::Disconnected)
        ));
    }

    fn run_movement_scripted(
        world_reads: Vec<u8>,
        config: &crate::ClientConfig,
    ) -> MovementScriptedRun {
        let credentials = CredentialMaterial::synthetic(b"LEARNER", b"ONLYFORVECTOR");
        let (client, mut boundary) = new_boundary(config.identity().clone()).unwrap();
        client.publish_movement_intent(crate::MovementIntent::planar(1.0, 0.0).unwrap());
        let mut factory = RetryScriptFactory::new([world_reads]);
        match run_entry_attempt(
            config,
            &credentials,
            &mut boundary,
            &mut factory,
            &mut FixedClock::default(),
            &mut FixedEntropy::success(PRIVATE_EPHEMERAL),
            WorkerTarget::MovementReady,
        ) {
            Ok(EntryAttemptOutcome::MovementReady) => {
                boundary.disconnect();
            }
            Ok(
                EntryAttemptOutcome::RealmDiscovered | EntryAttemptOutcome::CharacterSelected(_),
            ) => {
                panic!("scripted movement attempt stopped at the wrong capability boundary");
            }
            Err(DiscoveryError::Failure(failure)) => {
                boundary.fail(crate::CommandKind::StartEntry, failure);
            }
            Err(DiscoveryError::Cancelled | DiscoveryError::Boundary) => {
                panic!("scripted movement attempt ended without semantic evidence");
            }
        }
        boundary.mark_stopped();
        MovementScriptedRun {
            snapshot: client.snapshot(),
            events: client.drain_events(),
            login_states: factory.login_states,
            world_states: factory.world_states,
        }
    }

    fn run_faulted_movement(
        world_reads: Vec<u8>,
        world_read_error_at_eof: Option<io::ErrorKind>,
        fail_world_write_call: Option<usize>,
    ) -> MovementScriptedRun {
        let config = config();
        let credentials = CredentialMaterial::synthetic(b"LEARNER", b"ONLYFORVECTOR");
        let (client, mut boundary) = new_boundary(config.identity().clone()).unwrap();
        client.publish_movement_intent(crate::MovementIntent::planar(1.0, 0.0).unwrap());
        let login_state = ScriptState::default();
        let world_state = ScriptState::default();
        let mut factory = FaultingEntryFactory {
            world_reads: Some(world_reads),
            world_read_error_at_eof,
            fail_world_write_call,
            login_state: login_state.clone(),
            world_state: world_state.clone(),
        };
        match run_entry_attempt(
            &config,
            &credentials,
            &mut boundary,
            &mut factory,
            &mut FixedClock::default(),
            &mut FixedEntropy::success(PRIVATE_EPHEMERAL),
            WorkerTarget::MovementReady,
        ) {
            Err(DiscoveryError::Failure(failure)) => {
                boundary.fail(crate::CommandKind::StartEntry, failure);
            }
            Ok(_) | Err(DiscoveryError::Cancelled | DiscoveryError::Boundary) => {
                panic!("faulted movement attempt did not fail semantically");
            }
        }
        boundary.mark_stopped();
        MovementScriptedRun {
            snapshot: client.snapshot(),
            events: client.drain_events(),
            login_states: vec![login_state],
            world_states: vec![world_state],
        }
    }

    fn assert_failed_character(run: &CharacterScriptedRun, category: FailureCategory) {
        assert!(matches!(run.snapshot.phase, ClientPhase::Failed(_)));
        assert_eq!(
            run.snapshot.latest_failure.as_ref().unwrap().category(),
            category
        );
        assert!(run.login_state.closed.load(Ordering::Acquire));
        if run.world_state.connections.load(Ordering::Acquire) > 0
            && category != FailureCategory::Timeout
        {
            assert!(run.world_state.closed.load(Ordering::Acquire));
        }
        assert!(run.snapshot.selected_character.is_none());
        assert_eq!(
            run.events
                .iter()
                .filter(|event| matches!(
                    event.kind,
                    ClientEventKind::PhaseChanged {
                        phase: ClientPhase::Entering(crate::EntryStage::LoginConnection),
                    }
                ))
                .count(),
            1
        );
        assert!(matches!(
            run.events.last().map(|event| &event.kind),
            Some(ClientEventKind::Disconnected)
        ));
    }

    fn run_character_scripted<E>(
        world_reads: Vec<u8>,
        max_read: usize,
        world_connect_error: Option<io::ErrorKind>,
        mut clock: FixedClock,
        mut entropy: E,
        config: crate::ClientConfig,
    ) -> CharacterScriptedRun
    where
        E: EntropySource + 'static,
    {
        let credentials = CredentialMaterial::synthetic(b"LEARNER", b"ONLYFORVECTOR");
        let (client, mut boundary) = new_boundary(config.identity().clone()).unwrap();
        client.send_control(ControlCommand::StartEntry).unwrap();
        let login_state = ScriptState::default();
        let world_state = ScriptState::default();
        let mut factory = CharacterScriptFactory {
            login_reads: Some(success_script()),
            world_reads: Some(world_reads),
            max_read,
            world_connect_error,
            login_state: login_state.clone(),
            world_state: world_state.clone(),
        };
        let worker = thread::spawn(move || {
            run_worker_loop_for(
                &config,
                &credentials,
                &mut boundary,
                &mut factory,
                &mut clock,
                &mut entropy,
                WorkerTarget::CharacterSelection,
            );
            boundary.mark_stopped();
        });
        worker.join().unwrap();
        CharacterScriptedRun {
            snapshot: client.snapshot(),
            events: client.drain_events(),
            login_state,
            world_state,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn run_scripted(
        command: ControlCommand,
        reads: Vec<u8>,
        max_read: usize,
        connect_error: Option<io::ErrorKind>,
        mut clock: FixedClock,
        mut entropy: FixedEntropy,
        account: &[u8],
        password: &[u8],
    ) -> ScriptedRun {
        let config = config();
        let credentials = CredentialMaterial::synthetic(account, password);
        let (client, mut boundary) = new_boundary(config.identity().clone()).unwrap();
        client.send_control(command).unwrap();
        let state = ScriptState::default();
        let mut factory = ScriptFactory {
            reads: Some(reads),
            max_read,
            connect_error,
            state: state.clone(),
        };
        let worker = thread::spawn(move || {
            run_worker_loop(
                &config,
                &credentials,
                &mut boundary,
                &mut factory,
                &mut clock,
                &mut entropy,
            );
            boundary.mark_stopped();
        });
        worker.join().unwrap();
        ScriptedRun {
            snapshot: client.snapshot(),
            events: client.drain_events(),
            state,
        }
    }

    fn config() -> crate::ClientConfig {
        crate::ClientConfig::new(ClientConfigSpec {
            realm_id: 1,
            realm_name: "Miazcore Reference Realm".to_owned(),
            character_name: "Miaztest".to_owned(),
            client_build: 12_340,
            login_endpoint: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3724),
            world_endpoint: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8085),
            connect_timeout: Duration::from_secs(1),
            io_timeout: Duration::from_secs(1),
            credentials: CredentialPaths::new(
                PathBuf::from("synthetic-account"),
                PathBuf::from("synthetic-password"),
            ),
        })
        .unwrap()
    }

    #[derive(Clone, Default)]
    struct ScriptState {
        writes: Arc<Mutex<Vec<u8>>>,
        closed: Arc<AtomicBool>,
        connections: Arc<AtomicUsize>,
    }

    struct ScriptFactory {
        reads: Option<Vec<u8>>,
        max_read: usize,
        connect_error: Option<io::ErrorKind>,
        state: ScriptState,
    }

    impl TransportFactory for ScriptFactory {
        type Transport = ScriptTransport;

        fn connect_login(&mut self, _config: &crate::ClientConfig) -> io::Result<Self::Transport> {
            self.state.connections.fetch_add(1, Ordering::AcqRel);
            if let Some(kind) = self.connect_error {
                return Err(io::Error::from(kind));
            }
            Ok(ScriptTransport {
                reads: Cursor::new(self.reads.take().unwrap_or_default()),
                max_read: self.max_read,
                state: self.state.clone(),
            })
        }
    }

    struct CharacterScriptFactory {
        login_reads: Option<Vec<u8>>,
        world_reads: Option<Vec<u8>>,
        max_read: usize,
        world_connect_error: Option<io::ErrorKind>,
        login_state: ScriptState,
        world_state: ScriptState,
    }

    impl TransportFactory for CharacterScriptFactory {
        type Transport = ScriptTransport;

        fn connect_login(&mut self, _config: &crate::ClientConfig) -> io::Result<Self::Transport> {
            self.login_state.connections.fetch_add(1, Ordering::AcqRel);
            Ok(ScriptTransport {
                reads: Cursor::new(self.login_reads.take().unwrap_or_default()),
                max_read: self.max_read,
                state: self.login_state.clone(),
            })
        }

        fn connect_world(&mut self, _config: &crate::ClientConfig) -> io::Result<Self::Transport> {
            self.world_state.connections.fetch_add(1, Ordering::AcqRel);
            if let Some(kind) = self.world_connect_error {
                return Err(io::Error::from(kind));
            }
            Ok(ScriptTransport {
                reads: Cursor::new(self.world_reads.take().unwrap_or_default()),
                max_read: self.max_read,
                state: self.world_state.clone(),
            })
        }
    }

    struct RetryScriptFactory {
        world_reads: VecDeque<Vec<u8>>,
        login_states: Vec<ScriptState>,
        world_states: Vec<ScriptState>,
    }

    impl RetryScriptFactory {
        fn new(world_reads: impl IntoIterator<Item = Vec<u8>>) -> Self {
            Self {
                world_reads: world_reads.into_iter().collect(),
                login_states: Vec::new(),
                world_states: Vec::new(),
            }
        }
    }

    impl TransportFactory for RetryScriptFactory {
        type Transport = ScriptTransport;

        fn connect_login(&mut self, _config: &crate::ClientConfig) -> io::Result<Self::Transport> {
            let state = ScriptState::default();
            state.connections.fetch_add(1, Ordering::AcqRel);
            self.login_states.push(state.clone());
            Ok(ScriptTransport {
                reads: Cursor::new(success_script()),
                max_read: usize::MAX,
                state,
            })
        }

        fn connect_world(&mut self, _config: &crate::ClientConfig) -> io::Result<Self::Transport> {
            let state = ScriptState::default();
            state.connections.fetch_add(1, Ordering::AcqRel);
            self.world_states.push(state.clone());
            Ok(ScriptTransport {
                reads: Cursor::new(self.world_reads.pop_front().unwrap_or_default()),
                max_read: usize::MAX,
                state,
            })
        }
    }

    struct CancellingEntryFactory {
        client: Arc<crate::boundary::SessionClient>,
        world_reads: Option<Vec<u8>>,
        cancel_after: usize,
        login_state: ScriptState,
        world_state: ScriptState,
    }

    impl TransportFactory for CancellingEntryFactory {
        type Transport = CancellingScriptTransport;

        fn connect_login(&mut self, _config: &crate::ClientConfig) -> io::Result<Self::Transport> {
            self.login_state.connections.fetch_add(1, Ordering::AcqRel);
            Ok(CancellingScriptTransport {
                reads: Cursor::new(success_script()),
                cancel_after: usize::MAX,
                cancelled: false,
                client: Arc::clone(&self.client),
                state: self.login_state.clone(),
            })
        }

        fn connect_world(&mut self, _config: &crate::ClientConfig) -> io::Result<Self::Transport> {
            self.world_state.connections.fetch_add(1, Ordering::AcqRel);
            Ok(CancellingScriptTransport {
                reads: Cursor::new(self.world_reads.take().unwrap_or_default()),
                cancel_after: self.cancel_after,
                cancelled: false,
                client: Arc::clone(&self.client),
                state: self.world_state.clone(),
            })
        }
    }

    struct CancellingScriptTransport {
        reads: Cursor<Vec<u8>>,
        cancel_after: usize,
        cancelled: bool,
        client: Arc<crate::boundary::SessionClient>,
        state: ScriptState,
    }

    impl Read for CancellingScriptTransport {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let count = self.reads.read(buffer)?;
            if !self.cancelled
                && usize::try_from(self.reads.position()).unwrap() >= self.cancel_after
            {
                self.cancelled = true;
                self.client.request_shutdown();
            }
            Ok(count)
        }
    }

    impl Write for CancellingScriptTransport {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            self.state.writes.lock().unwrap().extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl LoginTransport for CancellingScriptTransport {
        fn close(&mut self) -> io::Result<()> {
            self.state.closed.store(true, Ordering::Release);
            Ok(())
        }
    }

    struct FaultingEntryFactory {
        world_reads: Option<Vec<u8>>,
        world_read_error_at_eof: Option<io::ErrorKind>,
        fail_world_write_call: Option<usize>,
        login_state: ScriptState,
        world_state: ScriptState,
    }

    impl TransportFactory for FaultingEntryFactory {
        type Transport = FaultingScriptTransport;

        fn connect_login(&mut self, _config: &crate::ClientConfig) -> io::Result<Self::Transport> {
            self.login_state.connections.fetch_add(1, Ordering::AcqRel);
            Ok(FaultingScriptTransport {
                reads: Cursor::new(success_script()),
                read_error_at_eof: None,
                fail_write_call: None,
                write_calls: 0,
                state: self.login_state.clone(),
            })
        }

        fn connect_world(&mut self, _config: &crate::ClientConfig) -> io::Result<Self::Transport> {
            self.world_state.connections.fetch_add(1, Ordering::AcqRel);
            Ok(FaultingScriptTransport {
                reads: Cursor::new(self.world_reads.take().unwrap_or_default()),
                read_error_at_eof: self.world_read_error_at_eof,
                fail_write_call: self.fail_world_write_call,
                write_calls: 0,
                state: self.world_state.clone(),
            })
        }
    }

    struct FaultingScriptTransport {
        reads: Cursor<Vec<u8>>,
        read_error_at_eof: Option<io::ErrorKind>,
        fail_write_call: Option<usize>,
        write_calls: usize,
        state: ScriptState,
    }

    impl Read for FaultingScriptTransport {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let count = self.reads.read(buffer)?;
            if count == 0
                && let Some(kind) = self.read_error_at_eof
            {
                return Err(io::Error::from(kind));
            }
            Ok(count)
        }
    }

    impl Write for FaultingScriptTransport {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            self.write_calls = self.write_calls.saturating_add(1);
            if self.fail_write_call == Some(self.write_calls) {
                return Err(io::Error::from(io::ErrorKind::BrokenPipe));
            }
            self.state.writes.lock().unwrap().extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl LoginTransport for FaultingScriptTransport {
        fn close(&mut self) -> io::Result<()> {
            self.state.closed.store(true, Ordering::Release);
            Ok(())
        }
    }

    struct ScriptTransport {
        reads: Cursor<Vec<u8>>,
        max_read: usize,
        state: ScriptState,
    }

    impl Read for ScriptTransport {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let read_len = buffer.len().min(self.max_read);
            self.reads.read(&mut buffer[..read_len])
        }
    }

    impl Write for ScriptTransport {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            self.state.writes.lock().unwrap().extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl LoginTransport for ScriptTransport {
        fn close(&mut self) -> io::Result<()> {
            self.state.closed.store(true, Ordering::Release);
            Ok(())
        }
    }

    #[derive(Default)]
    struct FixedClock {
        values: Vec<Duration>,
        index: usize,
    }

    impl FixedClock {
        fn sequence(values: impl IntoIterator<Item = Duration>) -> Self {
            Self {
                values: values.into_iter().collect(),
                index: 0,
            }
        }
    }

    impl MonotonicClock for FixedClock {
        fn now(&mut self) -> Duration {
            let value = self
                .values
                .get(self.index)
                .copied()
                .or_else(|| self.values.last().copied())
                .unwrap_or(Duration::ZERO);
            self.index = self.index.saturating_add(1);
            value
        }
    }

    struct FixedEntropy {
        value: [u8; 32],
        fails: bool,
    }

    struct CountingEntropy {
        calls: Arc<AtomicUsize>,
    }

    impl EntropySource for CountingEntropy {
        fn fill(&mut self, destination: &mut [u8]) -> Result<(), ()> {
            self.calls.fetch_add(1, Ordering::AcqRel);
            destination.copy_from_slice(&PRIVATE_EPHEMERAL[..destination.len()]);
            Ok(())
        }
    }

    impl FixedEntropy {
        const fn success(value: [u8; 32]) -> Self {
            Self {
                value,
                fails: false,
            }
        }

        const fn failure() -> Self {
            Self {
                value: [0; 32],
                fails: true,
            }
        }
    }

    impl EntropySource for FixedEntropy {
        fn fill(&mut self, destination: &mut [u8]) -> Result<(), ()> {
            if self.fails {
                return Err(());
            }
            destination.copy_from_slice(&self.value[..destination.len()]);
            Ok(())
        }
    }

    struct CancellingEntropy {
        client: Arc<crate::boundary::SessionClient>,
        value: [u8; 32],
    }

    struct CancellingWorldEntropy {
        client: Arc<crate::boundary::SessionClient>,
        calls: usize,
    }

    impl EntropySource for CancellingWorldEntropy {
        fn fill(&mut self, destination: &mut [u8]) -> Result<(), ()> {
            self.calls = self.calls.saturating_add(1);
            destination.copy_from_slice(&PRIVATE_EPHEMERAL[..destination.len()]);
            if self.calls == 2 {
                self.client.request_shutdown();
            }
            Ok(())
        }
    }

    impl EntropySource for CancellingEntropy {
        fn fill(&mut self, destination: &mut [u8]) -> Result<(), ()> {
            self.client.request_shutdown();
            destination.copy_from_slice(&self.value);
            Ok(())
        }
    }

    #[test]
    fn synthetic_identity_is_not_credential_derived() {
        let identity =
            SanitizedIdentity::new(1, "Miazcore Reference Realm", "Miaztest", 12_340).unwrap();
        assert_ne!(identity.realm_name(), "LEARNER");
    }
}
