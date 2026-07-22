use std::{
    io::{self, Read, Write},
    net::{Shutdown, TcpStream},
    sync::mpsc,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use client_protocol::{
    LoginChallengeResponse, LoginProofResponse, ProtocolError, REALM_LIST_REQUEST,
    calculate_srp_client_proof, encode_logon_challenge, encode_logon_proof,
    read_logon_challenge_response, read_logon_proof_response, read_realm_list_response,
};
use zeroize::Zeroizing;

use crate::{
    ClientConfig, ClientFailure, ClientPhase, CommandKind, ControlCommand, DiscoveredRealm,
    EntryStage, FailureCategory, LoadedClientConfig, RecoveryAction,
    boundary::{BoundaryError, WorkerBoundary},
    config::CredentialMaterial,
    machine::RealmDiscoveryMachine,
};

trait LoginTransport: Read + Write + Send {
    fn close(&mut self) -> io::Result<()>;
}

trait TransportFactory: Send {
    type Transport: LoginTransport;

    fn connect(&mut self, config: &ClientConfig) -> io::Result<Self::Transport>;
}

trait MonotonicClock: Send {
    fn now(&mut self) -> Duration;
}

trait EntropySource: Send {
    fn fill(&mut self, destination: &mut [u8]) -> Result<(), ()>;
}

pub(crate) fn spawn_production_worker(
    loaded: LoadedClientConfig,
    mut boundary: WorkerBoundary,
) -> Result<JoinHandle<()>, BoundaryError> {
    thread::Builder::new()
        .name("miazcore-realm-discovery".to_owned())
        .spawn(move || {
            let (config, mut credentials) = loaded.into_parts();
            credentials.normalize_for_login();
            run_worker_loop(
                &config,
                &credentials,
                &mut boundary,
                &mut TcpTransportFactory,
                &mut SystemClock::new(),
                &mut SystemEntropy,
            );
            boundary.mark_stopped();
        })
        .map_err(|_| BoundaryError::WorkerStopped)
}

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
                match discover(config, credentials, boundary, factory, clock, entropy) {
                    Ok(realm) => {
                        if boundary.discovered(realm) {
                            boundary.disconnect();
                        }
                    }
                    Err(DiscoveryError::Failure(failure)) => {
                        boundary.fail(CommandKind::StartEntry, failure);
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
) -> Result<DiscoveredRealm, DiscoveryError>
where
    F: TransportFactory,
    C: MonotonicClock,
    E: EntropySource,
{
    let started = clock.now();
    let budget = config
        .connect_timeout()
        .checked_add(config.io_timeout().saturating_mul(4))
        .ok_or_else(timeout_failure)?;
    let deadline = started.checked_add(budget).ok_or_else(timeout_failure)?;
    let mut machine = RealmDiscoveryMachine::new();
    transition(boundary, machine.begin(), EntryStage::LoginConnection)?;

    let mut transport = factory.connect(config).map_err(|error| {
        io_failure(
            error.kind(),
            "login connection",
            RecoveryAction::CheckReferenceRealm,
        )
    })?;
    let result = exchange(
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
    machine: &mut RealmDiscoveryMachine,
    transport: &mut T,
    clock: &mut C,
    entropy: &mut E,
    deadline: Duration,
) -> Result<DiscoveredRealm, DiscoveryError>
where
    T: LoginTransport,
    C: MonotonicClock,
    E: EntropySource,
{
    authenticate(
        credentials,
        boundary,
        machine,
        transport,
        clock,
        entropy,
        deadline,
    )?;
    select_realm(config, boundary, machine, transport, clock, deadline)
}

#[allow(clippy::too_many_arguments)]
fn authenticate<T, C, E>(
    credentials: &CredentialMaterial,
    boundary: &mut WorkerBoundary,
    machine: &mut RealmDiscoveryMachine,
    transport: &mut T,
    clock: &mut C,
    entropy: &mut E,
    deadline: Duration,
) -> Result<(), DiscoveryError>
where
    T: LoginTransport,
    C: MonotonicClock,
    E: EntropySource,
{
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
    check_deadline(clock, deadline)?;

    let mut private_ephemeral = Zeroizing::new([0_u8; 32]);
    entropy
        .fill(private_ephemeral.as_mut())
        .map_err(|()| entropy_failure())?;
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
    if !proof.verify_server_proof(&server_proof) {
        return Err(DiscoveryError::Failure(ClientFailure::new(
            FailureCategory::Authentication,
            "login authentication",
            "login server proof did not authenticate the session",
            RecoveryAction::CheckCredentials,
        )));
    }
    check_deadline(clock, deadline)?;
    Ok(())
}

fn select_realm<T, C>(
    config: &ClientConfig,
    boundary: &mut WorkerBoundary,
    machine: &mut RealmDiscoveryMachine,
    transport: &mut T,
    clock: &mut C,
    deadline: Duration,
) -> Result<DiscoveredRealm, DiscoveryError>
where
    T: LoginTransport,
    C: MonotonicClock,
{
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
    let realms = read_realm_list_response(transport).map_err(|error| {
        protocol_failure(
            error,
            "realm selection",
            RecoveryAction::CheckReferenceRealm,
        )
    })?;
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
    machine
        .complete()
        .map_err(|_| internal_transition_failure())?;
    DiscoveredRealm::new(
        u32::from(realm.id()),
        realm.name(),
        realm.build().unwrap_or(config.identity().client_build()),
        endpoint,
    )
    .map_err(|_| realm_mismatch())
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
    Boundary,
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
    let (category, context, recovery) =
        if matches!(kind, io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock) {
            (
                FailureCategory::Timeout,
                "login transport operation timed out",
                RecoveryAction::RetryExplicitly,
            )
        } else {
            (
                FailureCategory::Transport,
                "login transport operation failed",
                recovery,
            )
        };
    DiscoveryError::Failure(ClientFailure::new(category, stage, context, recovery))
}

fn protocol_failure(
    error: ProtocolError,
    stage: &'static str,
    recovery: RecoveryAction,
) -> DiscoveryError {
    match error {
        ProtocolError::Io(kind) => io_failure(kind, stage, recovery),
        ProtocolError::InvalidCredentialEncoding => DiscoveryError::Failure(ClientFailure::new(
            FailureCategory::Configuration,
            stage,
            "credential encoding is incompatible with the login protocol",
            RecoveryAction::FixConfiguration,
        )),
        ProtocolError::MalformedFrame
        | ProtocolError::UnsupportedSecurity
        | ProtocolError::InvalidSrpParameters => DiscoveryError::Failure(ClientFailure::new(
            FailureCategory::ProtocolIncompatibility,
            stage,
            "Reference Realm login protocol response was incompatible",
            recovery,
        )),
    }
}

struct TcpTransportFactory;

impl TransportFactory for TcpTransportFactory {
    type Transport = TcpLoginTransport;

    fn connect(&mut self, config: &ClientConfig) -> io::Result<Self::Transport> {
        let stream =
            TcpStream::connect_timeout(&config.login_endpoint(), config.connect_timeout())?;
        stream.set_read_timeout(Some(config.io_timeout()))?;
        stream.set_write_timeout(Some(config.io_timeout()))?;
        stream.set_nodelay(true)?;
        Ok(TcpLoginTransport(stream))
    }
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

    use super::{EntropySource, LoginTransport, MonotonicClock, TransportFactory, run_worker_loop};

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
    }

    fn success_script() -> Vec<u8> {
        [
            fixture("login-challenge-response.hex"),
            fixture("login-proof-response.hex"),
            fixture("realm-response.hex"),
        ]
        .concat()
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
            _ => panic!("unknown fixture"),
        };
        value
            .trim()
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

        fn connect(&mut self, _config: &crate::ClientConfig) -> io::Result<Self::Transport> {
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
