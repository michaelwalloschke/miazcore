use std::{
    fs,
    path::{Path, PathBuf},
};

use client_protocol::{
    CMSG_PLAYER_LOGIN, ProtocolError, SMSG_COMPRESSED_UPDATE_OBJECT, SMSG_UPDATE_OBJECT,
    decode_authoritative_self_update, decode_force_run_speed_change, decode_login_verify_world,
    decode_time_sync_request, decode_unset_can_fly, encode_force_run_speed_change_ack,
    encode_move_set_can_fly_ack, encode_player_login, encode_time_sync_response,
};
use flate2::{Compression, write::ZlibEncoder};
use std::io::Write as _;

const SELECTED_GUID: u64 = 0x1234;

#[test]
fn golden_world_entry_transcript_matches_selective_production_codecs() {
    assert_eq!(CMSG_PLAYER_LOGIN, 0x003d);
    assert_eq!(
        encode_player_login(SELECTED_GUID),
        fixture("world-entry-player-login-body.hex").as_slice()
    );

    let location = decode_login_verify_world(&fixture("world-entry-login-verify-body.hex"))
        .expect("independent login location is valid");
    assert_eq!(location.map_id(), 0);
    assert_f32_array(location.position(), [1.25, -2.5, 3.75]);
    assert_f32(location.orientation(), 0.5);

    let plain = decode_authoritative_self_update(
        SMSG_UPDATE_OBJECT,
        &fixture("world-entry-self-update-body.hex"),
        SELECTED_GUID,
    )
    .unwrap()
    .expect("plain update contains selected self");
    let compressed = decode_authoritative_self_update(
        SMSG_COMPRESSED_UPDATE_OBJECT,
        &fixture("world-entry-self-update-compressed-body.hex"),
        SELECTED_GUID,
    )
    .unwrap()
    .expect("compressed update contains selected self");
    assert_eq!(compressed, plain);
    assert_eq!(plain.guid(), SELECTED_GUID);
    assert_f32_array(plain.movement().position(), location.position());
    assert_f32(plain.movement().orientation(), location.orientation());
    assert_eq!(plain.movement().fall_time_ms(), 0x7fc0_0001);
    assert_eq!(plain.speeds().values().len(), 9);
    assert_f32(plain.speeds().run(), 7.0);

    let speed = decode_force_run_speed_change(&fixture("world-entry-force-run-body.hex")).unwrap();
    assert_eq!(speed.guid(), SELECTED_GUID);
    assert_eq!(speed.counter(), 7);
    assert_f32(speed.run_speed(), 8.5);
    assert_eq!(
        encode_force_run_speed_change_ack(speed, plain.movement()).unwrap(),
        fixture("world-entry-force-run-ack-body.hex")
    );

    let no_flight = decode_unset_can_fly(&fixture("world-entry-unset-can-fly-body.hex")).unwrap();
    assert_eq!(no_flight.guid(), SELECTED_GUID);
    assert_eq!(no_flight.counter(), 9);
    assert_eq!(
        encode_move_set_can_fly_ack(no_flight, plain.movement()).unwrap(),
        fixture("world-entry-unset-can-fly-ack-body.hex")
    );

    let counter =
        decode_time_sync_request(&fixture("world-entry-time-sync-request-body.hex")).unwrap();
    assert_eq!(counter, 11);
    assert_eq!(
        encode_time_sync_response(counter, 0x5566_7788),
        fixture("world-entry-time-sync-response-body.hex").as_slice()
    );
}

#[test]
fn sanitized_live_self_projection_preserves_observed_pose_and_all_nine_speeds() {
    let state = decode_authoritative_self_update(
        SMSG_UPDATE_OBJECT,
        &fixture("world-entry-live-self-projection-body.hex"),
        SELECTED_GUID,
    )
    .unwrap()
    .expect("sanitized live projection contains selected self");
    assert_eq!(state.movement().flags(), 0);
    assert_eq!(state.movement().flags2(), 0);
    assert_eq!(state.movement().timestamp(), 0);
    assert_eq!(state.movement().fall_time_ms(), 0);
    assert_f32_array(state.movement().position(), [-8949.95, -132.493, 83.5312]);
    assert_f32(state.movement().orientation(), 0.0);
    for (actual, expected) in state.speeds().values().into_iter().zip([
        2.5,
        7.0,
        4.5,
        4.722_222,
        2.5,
        7.0,
        4.5,
        3.141_594,
        f32::from_bits(0x4048_f5c3),
    ]) {
        assert_f32(actual, expected);
    }
}

#[test]
fn malformed_or_ambiguous_self_updates_fail_closed() {
    let valid = fixture("world-entry-self-update-body.hex");

    assert!(
        decode_authoritative_self_update(SMSG_UPDATE_OBJECT, &0_u32.to_le_bytes(), SELECTED_GUID)
            .unwrap()
            .is_none()
    );

    let mut truncated = valid.clone();
    truncated.pop();
    assert_malformed(SMSG_UPDATE_OBJECT, &truncated);

    let mut trailing = valid.clone();
    trailing.push(0xff);
    assert_malformed(SMSG_UPDATE_OBJECT, &trailing);

    let mut bad_mask_count = valid.clone();
    bad_mask_count[77] = u8::MAX;
    assert_malformed(SMSG_UPDATE_OBJECT, &bad_mask_count);

    let mut unknown_type = valid.clone();
    unknown_type[4] = 0xff;
    assert_malformed(SMSG_UPDATE_OBJECT, &unknown_type);

    let mut wrong_flags = valid.clone();
    wrong_flags[9..11].copy_from_slice(&0x0060_u16.to_le_bytes());
    assert_malformed(SMSG_UPDATE_OBJECT, &wrong_flags);

    let mut mismatched_self = valid.clone();
    mismatched_self[6] = 0x35;
    assert_malformed(SMSG_UPDATE_OBJECT, &mismatched_self);

    let mut non_finite = valid.clone();
    non_finite[21..25].copy_from_slice(&f32::NAN.to_le_bytes());
    assert_malformed(SMSG_UPDATE_OBJECT, &non_finite);

    let block = &valid[4..];
    let duplicate = [2_u32.to_le_bytes().as_slice(), block, block].concat();
    assert_malformed(SMSG_UPDATE_OBJECT, &duplicate);

    let mut unsupported = valid;
    unsupported[11..15].copy_from_slice(&0x0000_0800_u32.to_le_bytes());
    assert!(matches!(
        decode_authoritative_self_update(SMSG_UPDATE_OBJECT, &unsupported, SELECTED_GUID),
        Err(ProtocolError::UnsupportedMovementState)
    ));
}

#[test]
fn compressed_updates_enforce_declared_size_stream_end_and_single_layer() {
    let valid = fixture("world-entry-self-update-compressed-body.hex");

    let mut oversized = valid.clone();
    oversized[..4].copy_from_slice(&(1024_u32 * 1024 + 1).to_le_bytes());
    assert_malformed(SMSG_COMPRESSED_UPDATE_OBJECT, &oversized);

    let mut truncated = valid.clone();
    truncated.pop();
    assert_malformed(SMSG_COMPRESSED_UPDATE_OBJECT, &truncated);

    let mut trailing = valid.clone();
    trailing.push(0);
    assert_malformed(SMSG_COMPRESSED_UPDATE_OBJECT, &trailing);

    let mut wrong_size = valid.clone();
    wrong_size[..4].copy_from_slice(&89_u32.to_le_bytes());
    assert_malformed(SMSG_COMPRESSED_UPDATE_OBJECT, &wrong_size);

    let mut underfilled = valid.clone();
    underfilled[..4].copy_from_slice(&91_u32.to_le_bytes());
    assert_malformed(SMSG_COMPRESSED_UPDATE_OBJECT, &underfilled);

    let nested_body = valid;
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&nested_body).unwrap();
    let nested_stream = encoder.finish().unwrap();
    let nested = [
        u32::try_from(nested_body.len())
            .unwrap()
            .to_le_bytes()
            .as_slice(),
        nested_stream.as_slice(),
    ]
    .concat();
    assert_malformed(SMSG_COMPRESSED_UPDATE_OBJECT, &nested);
}

fn assert_malformed(opcode: u16, payload: &[u8]) {
    match decode_authoritative_self_update(opcode, payload, SELECTED_GUID) {
        Err(ProtocolError::MalformedWorldEntry {
            opcode: actual_opcode,
            byte_offset,
        }) => {
            assert_eq!(actual_opcode, opcode);
            assert!(byte_offset <= 1024 * 1024);
        }
        result => panic!("expected offset-bearing malformed update, got {result:?}"),
    }
}

fn assert_f32(actual: f32, expected: f32) {
    assert_eq!(actual.to_bits(), expected.to_bits());
}

fn assert_f32_array(actual: [f32; 3], expected: [f32; 3]) {
    for (actual, expected) in actual.into_iter().zip(expected) {
        assert_f32(actual, expected);
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
