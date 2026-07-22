use std::{
    collections::BTreeMap,
    fmt::Write as _,
    fs,
    io::{self, Cursor, Read},
    path::{Path, PathBuf},
};

use client_protocol::{
    HeaderCipher, HeaderDirection, LoginChallengeResponse, LoginProofResponse, ProtocolError,
    REALM_LIST_REQUEST, calculate_srp_client_proof, encode_logon_challenge, encode_logon_proof,
    read_logon_challenge_response, read_logon_proof_response, read_realm_list_response,
};
use sha2::{Digest, Sha256};

const ACCOUNT: &[u8] = b"LEARNER";
const PASSWORD: &[u8] = b"ONLYFORVECTOR";
const PRIVATE_EPHEMERAL: [u8; 32] = [
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10,
    0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x20,
];

#[test]
fn every_v1_manifest_is_complete_and_matches_its_independent_payload() {
    let fixture_root = fixture_root();
    let mut manifests = fs::read_dir(&fixture_root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "manifest")
        })
        .collect::<Vec<_>>();
    manifests.sort();
    assert_eq!(manifests.len(), 23);

    for path in manifests {
        let records = parse_manifest(&path);
        assert_eq!(records.len(), 10, "unexpected manifest keys in {path:?}");
        assert_eq!(records["format"], "miazcore-wire-fixture-v1");
        assert_eq!(records["build"], "12340");
        for key in [
            "direction",
            "opcode",
            "semantics",
            "byte_length",
            "sha256",
            "provenance",
            "upstream_pin",
            "payload",
        ] {
            assert!(!records[key].is_empty(), "empty {key} in {path:?}");
        }
        let payload = fixture(&records["payload"]);
        assert_eq!(payload.len(), records["byte_length"].parse().unwrap());
        assert_eq!(hex(&Sha256::digest(&payload)), records["sha256"]);
    }
}

#[test]
fn golden_login_transcript_matches_production_codecs_and_srp() {
    assert_eq!(
        encode_logon_challenge(ACCOUNT).unwrap(),
        fixture("login-challenge-request.hex")
    );

    let challenge_bytes = fixture("login-challenge-response.hex");
    let challenge = match read_logon_challenge_response(&mut Cursor::new(challenge_bytes)).unwrap()
    {
        LoginChallengeResponse::Accepted(challenge) => challenge,
        LoginChallengeResponse::Rejected { result } => panic!("unexpected rejection {result}"),
    };
    let proof = calculate_srp_client_proof(ACCOUNT, PASSWORD, &challenge, PRIVATE_EPHEMERAL)
        .expect("synthetic SRP inputs are valid");
    assert_eq!(
        encode_logon_proof(&proof).as_slice(),
        fixture("login-proof-request.hex")
    );

    let response =
        read_logon_proof_response(&mut Cursor::new(fixture("login-proof-response.hex"))).unwrap();
    let server_proof = match response {
        LoginProofResponse::Accepted { server_proof } => server_proof,
        LoginProofResponse::Rejected { result } => panic!("unexpected rejection {result}"),
    };
    assert!(proof.verify_server_proof(&server_proof));

    assert_eq!(REALM_LIST_REQUEST.as_slice(), fixture("realm-request.hex"));
    let realms = read_realm_list_response(&mut Cursor::new(fixture("realm-response.hex"))).unwrap();
    assert_eq!(realms.len(), 1);
    let realm = &realms[0];
    assert_eq!(realm.id(), 1);
    assert_eq!(realm.name(), "Miazcore Reference Realm");
    assert_eq!(realm.address(), "127.0.0.1:8085");
    assert_eq!(realm.build(), Some(12_340));
    assert!(!realm.is_locked());
}

#[test]
fn directional_header_crypto_matches_independent_vectors() {
    let session_key: [u8; 40] = core::array::from_fn(|index| u8::try_from(index).unwrap());
    let plaintext = [0x00, 0x2a, 0x34, 0x12, 0xef, 0xbe];

    for (direction, expected) in [
        (
            HeaderDirection::ClientToServer,
            fixture("header-client-ciphertext.hex"),
        ),
        (
            HeaderDirection::ServerToClient,
            fixture("header-server-ciphertext.hex"),
        ),
    ] {
        let mut actual = plaintext;
        HeaderCipher::new(direction, &session_key).apply(&mut actual);
        assert_eq!(actual.as_slice(), expected);
    }
}

#[test]
fn fragmented_reads_are_consumed_and_malformed_frames_fail_closed() {
    let mut fragmented = OneByteAtATime::new(fixture("realm-response.hex"));
    let realms = read_realm_list_response(&mut fragmented).unwrap();
    assert_eq!(realms[0].id(), 1);

    let mut truncated = fixture("login-challenge-response.hex");
    truncated.pop();
    assert!(matches!(
        read_logon_challenge_response(&mut Cursor::new(truncated)),
        Err(ProtocolError::Io(io::ErrorKind::UnexpectedEof))
    ));

    let rejected = read_logon_proof_response(&mut Cursor::new([0x01, 0x04])).unwrap();
    assert_eq!(rejected, LoginProofResponse::Rejected { result: 0x04 });

    let mut malformed_realm = fixture("realm-response.hex");
    *malformed_realm.last_mut().unwrap() = 0xff;
    assert!(matches!(
        read_realm_list_response(&mut Cursor::new(malformed_realm)),
        Err(ProtocolError::MalformedFrame)
    ));
}

#[test]
fn incompatible_srp_groups_fail_closed() {
    let mut altered_generator = fixture("login-challenge-response.hex");
    let generator_offset = 3 + 32 + 1;
    altered_generator[generator_offset] = 5;
    assert!(matches!(
        read_logon_challenge_response(&mut Cursor::new(altered_generator)),
        Err(ProtocolError::InvalidSrpParameters)
    ));

    let mut altered_prime = fixture("login-challenge-response.hex");
    let prime_offset = generator_offset + 1 + 1;
    altered_prime[prime_offset] ^= 1;
    assert!(matches!(
        read_logon_challenge_response(&mut Cursor::new(altered_prime)),
        Err(ProtocolError::InvalidSrpParameters)
    ));
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v1")
}

fn fixture(name: &str) -> Vec<u8> {
    let encoded = fs::read_to_string(fixture_root().join(name)).unwrap();
    decode_hex(&encoded.split_whitespace().collect::<String>())
}

fn decode_hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).unwrap();
            u8::from_str_radix(pair, 16).unwrap()
        })
        .collect()
}

fn hex(value: &[u8]) -> String {
    value.iter().fold(String::new(), |mut output, byte| {
        write!(output, "{byte:02x}").unwrap();
        output
    })
}

fn parse_manifest(path: &Path) -> BTreeMap<String, String> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| {
            let (key, value) = line.split_once('=').expect("manifest record has '='");
            (key.to_owned(), value.to_owned())
        })
        .collect()
}

struct OneByteAtATime {
    inner: Cursor<Vec<u8>>,
}

impl OneByteAtATime {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            inner: Cursor::new(bytes),
        }
    }
}

impl Read for OneByteAtATime {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let read_len = buffer.len().min(1);
        self.inner.read(&mut buffer[..read_len])
    }
}
