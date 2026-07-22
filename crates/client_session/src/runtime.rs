use std::{
    io::{self, Read, Write},
    net::{Shutdown, TcpStream},
    sync::mpsc,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use client_protocol::{
    CMSG_CHAR_ENUM, LoginChallengeResponse, LoginProofResponse, ProtocolError, REALM_LIST_REQUEST,
    SMSG_AUTH_RESPONSE, SMSG_CHAR_ENUM, WorldAuthResponse, WorldClientStream, WorldServerStream,
    calculate_srp_client_proof, decode_character_enumeration, decode_world_auth_challenge,
    decode_world_auth_response, encode_logon_challenge, encode_logon_proof,
    encode_world_auth_session_frame, read_logon_challenge_response, read_logon_proof_response,
    read_plain_world_server_frame, read_realm_list_response,
};
use zeroize::Zeroizing;

use crate::{
    ClientConfig, ClientFailure, ClientPhase, CommandKind, ControlCommand, DiscoveredRealm,
    EntryStage, FailureCategory, LoadedClientConfig, RecoveryAction, SelectedCharacter,
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
    loop {
        match boundary.receive_control(Duration::from_millis(20)) {
            Ok(command) => {
                boundary.control_consumed();
                if command == ControlCommand::Disconnect || boundary.is_shutdown() {
                    boundary.disconnect();
                    return;
                }
                if command != ControlCommand::StartEntry {
                    let failure = ClientFailure::new(
                        FailureCategory::Configuration,
                        "realm discovery",
                        "command is outside the headless realm-discovery capability",
                        RecoveryAction::RestartClient,
                    );
                    if !boundary.reject(command.kind(), failure) {
                        return;
                    }
                    continue;
                }
                match discover(
                    config,
                    credentials,
                    boundary,
                    factory,
                    clock,
                    entropy,
                    target,
                ) {
                    Ok(character) => {
                        if character.is_none_or(|character| boundary.selected(character)) {
                            boundary.disconnect();
                        }
                    }
                    Err(DiscoveryError::Failure(failure)) => {
                        boundary.fail(CommandKind::StartEntry, failure);
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

fn discover<F, C, E>(
    config: &ClientConfig,
    credentials: &CredentialMaterial,
    boundary: &mut WorkerBoundary,
    factory: &mut F,
    clock: &mut C,
    entropy: &mut E,
    target: WorkerTarget,
) -> Result<Option<SelectedCharacter>, DiscoveryError>
where
    F: TransportFactory,
    C: MonotonicClock,
    E: EntropySource,
{
    check_cancelled(boundary)?;
    let started = clock.now();
    let connection_count = if target == WorkerTarget::CharacterSelection {
        2
    } else {
        1
    };
    let io_count = if target == WorkerTarget::CharacterSelection {
        8
    } else {
        4
    };
    let budget = config
        .connect_timeout()
        .saturating_mul(connection_count)
        .checked_add(config.io_timeout().saturating_mul(io_count))
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
    if target == WorkerTarget::RealmDiscovery {
        machine
            .complete_after_realm()
            .map_err(|_| internal_transition_failure())?;
        return Ok(None);
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

#[allow(clippy::too_many_arguments)]
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
) -> Result<Option<SelectedCharacter>, DiscoveryError>
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
    machine
        .complete()
        .map_err(|_| internal_transition_failure())?;
    Ok(Some(selected))
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
        HeaderCipher, HeaderDirection, LoginChallengeResponse, SMSG_AUTH_RESPONSE, SMSG_CHAR_ENUM,
        calculate_srp_client_proof, read_logon_challenge_response,
    };

    use super::{
        EntropySource, LoginTransport, MonotonicClock, TransportFactory, WorkerTarget,
        run_worker_loop, run_worker_loop_for,
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
