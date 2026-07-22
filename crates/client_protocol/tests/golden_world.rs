use std::{
    fs,
    io::{self, Cursor, Read},
    path::{Path, PathBuf},
};

use client_protocol::{
    CMSG_CHAR_ENUM, ProtocolError, SMSG_AUTH_RESPONSE, SMSG_CHAR_ENUM, WorldAuthResponse,
    WorldClientStream, WorldServerStream, decode_character_enumeration,
    decode_world_auth_challenge, decode_world_auth_response, encode_world_auth_session_frame,
    read_plain_world_server_frame,
};

const ACCOUNT: &[u8] = b"LEARNER";
const SESSION_KEY: [u8; 40] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f,
    0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27,
];

#[test]
fn golden_world_challenge_and_auth_session_match_independent_frames() {
    let frame =
        read_plain_world_server_frame(&mut Cursor::new(fixture("world-auth-challenge-frame.hex")))
            .unwrap();
    let challenge = decode_world_auth_challenge(&frame).unwrap();
    assert_eq!(challenge.server_seed(), 0x1122_3344);

    let encoded = encode_world_auth_session_frame(
        ACCOUNT,
        1,
        0x5566_7788,
        challenge.server_seed(),
        &SESSION_KEY,
    )
    .unwrap();
    assert_eq!(encoded.as_slice(), fixture("world-auth-session-frame.hex"));
}

#[test]
fn encrypted_server_stream_stays_aligned_across_fragmented_complete_frames() {
    let bytes = fixture("world-server-encrypted-stream.hex");
    let mut reader = OneByteAtATime::new(bytes.clone());
    let mut stream = WorldServerStream::new(&SESSION_KEY);

    let auth = stream.read_frame(&mut reader).unwrap();
    assert_eq!(auth.opcode(), SMSG_AUTH_RESPONSE);
    assert_eq!(
        decode_world_auth_response(auth.payload()).unwrap(),
        WorldAuthResponse::Accepted
    );

    let unknown = stream.read_frame(&mut reader).unwrap();
    assert_eq!(unknown.opcode(), 0x0222);
    assert_eq!(unknown.payload(), [0xaa, 0xbb, 0xcc]);

    let characters = stream.read_frame(&mut reader).unwrap();
    assert_eq!(characters.opcode(), SMSG_CHAR_ENUM);
    let characters = decode_character_enumeration(characters.payload()).unwrap();
    assert_eq!(characters.len(), 1);
    assert_character(&characters[0]);
    assert_eq!(reader.position(), u64::try_from(bytes.len()).unwrap());
}

#[test]
fn encrypted_streams_stay_aligned_across_coalesced_packets_in_both_directions() {
    let server_bytes = fixture("world-server-encrypted-stream.hex");
    let mut reader = Cursor::new(server_bytes.clone());
    let mut server = WorldServerStream::new(&SESSION_KEY);
    assert_eq!(
        server.read_frame(&mut reader).unwrap().opcode(),
        SMSG_AUTH_RESPONSE
    );
    assert_eq!(server.read_frame(&mut reader).unwrap().opcode(), 0x0222);
    assert_eq!(
        server.read_frame(&mut reader).unwrap().opcode(),
        SMSG_CHAR_ENUM
    );
    assert_eq!(
        reader.position(),
        u64::try_from(server_bytes.len()).unwrap()
    );

    let mut client = WorldClientStream::new(&SESSION_KEY);
    let actual = [
        client.encode_frame(CMSG_CHAR_ENUM, &[]).unwrap(),
        client.encode_frame(0x0012_3456, &[1, 2, 3]).unwrap(),
        client.encode_frame(CMSG_CHAR_ENUM, &[]).unwrap(),
    ]
    .concat();
    assert_eq!(actual, fixture("world-client-encrypted-stream.hex"));
}

#[test]
fn malformed_headers_cipher_drift_and_incomplete_character_records_fail_closed() {
    assert!(matches!(
        read_plain_world_server_frame(&mut Cursor::new([0x00, 0x01, 0xec, 0x01])),
        Err(ProtocolError::MalformedFrame)
    ));

    let mut drifted = fixture("world-server-encrypted-stream.hex");
    drifted.remove(0);
    let mut stream = WorldServerStream::new(&SESSION_KEY);
    assert!(stream.read_frame(&mut Cursor::new(drifted)).is_err());

    let mut truncated = fixture("character-enum-body.hex");
    truncated.pop();
    assert!(decode_character_enumeration(&truncated).is_err());

    let mut trailing = fixture("character-enum-body.hex");
    trailing.push(0xff);
    assert!(matches!(
        decode_character_enumeration(&trailing),
        Err(ProtocolError::MalformedFrame)
    ));
}

fn assert_character(character: &client_protocol::WorldCharacter) {
    assert_eq!(character.guid(), 0x1234);
    assert_eq!(character.name(), "Miaztest");
    assert_eq!(character.race(), 1);
    assert_eq!(character.class(), 1);
    assert_eq!(character.gender(), 0);
    assert_eq!(character.level(), 80);
    assert_eq!(character.area_id(), 1);
    assert_eq!(character.map_id(), 0);
    for (actual, expected) in character.position().into_iter().zip([1.25, -2.5, 3.75]) {
        assert!((actual - expected).abs() < f32::EPSILON);
    }
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

struct OneByteAtATime {
    inner: Cursor<Vec<u8>>,
}

impl OneByteAtATime {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            inner: Cursor::new(bytes),
        }
    }

    fn position(&self) -> u64 {
        self.inner.position()
    }
}

impl Read for OneByteAtATime {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let read_len = buffer.len().min(1);
        self.inner.read(&mut buffer[..read_len])
    }
}
