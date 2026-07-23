//! Engine-independent protocol boundary for the Learning Client.
//!
//! This crate owns byte framing and cryptographic compatibility with the pinned
//! build-12340 Reference Realm. It deliberately owns no socket, runtime, engine,
//! or application-boundary types.

mod error;
mod header_crypto;
mod login;
mod srp;
mod world;
mod world_entry;

pub use error::ProtocolError;
pub use header_crypto::{HeaderCipher, HeaderDirection};
pub use login::{
    LoginChallenge, LoginChallengeResponse, LoginProofResponse, RealmEntry, encode_logon_challenge,
    read_logon_challenge_response, read_logon_proof_response, read_realm_list_response,
};
pub use srp::{ClientSrpProof, SessionKey, calculate_srp_client_proof};
pub use world::{
    CMSG_AUTH_SESSION, CMSG_CHAR_ENUM, IncrementalWorldServerDecoder, SMSG_AUTH_CHALLENGE,
    SMSG_AUTH_RESPONSE, SMSG_CHAR_ENUM, WorldAuthChallenge, WorldAuthResponse, WorldCharacter,
    WorldClientStream, WorldServerFrame, WorldServerStream, decode_character_enumeration,
    decode_world_auth_challenge, decode_world_auth_response, encode_world_auth_session_frame,
    read_plain_world_server_frame,
};
pub use world_entry::{
    AcoreJumpInfo, AcoreMovementInfo, AcoreTransportInfo, AuthoritativeSelfState, BootstrapSpeeds,
    CMSG_FORCE_MOVE_ROOT_ACK, CMSG_FORCE_RUN_SPEED_CHANGE_ACK, CMSG_LOGOUT_REQUEST,
    CMSG_MOVE_SET_CAN_FLY_ACK, CMSG_PLAYER_LOGIN, CMSG_TIME_SYNC_RESP, ForceMoveRoot,
    ForceRunSpeedChange, MSG_MOVE_HEARTBEAT, MSG_MOVE_START_FORWARD, MSG_MOVE_STOP,
    SMSG_COMPRESSED_UPDATE_OBJECT, SMSG_FORCE_MOVE_ROOT, SMSG_FORCE_RUN_SPEED_CHANGE,
    SMSG_LOGIN_VERIFY_WORLD, SMSG_LOGOUT_COMPLETE, SMSG_LOGOUT_RESPONSE, SMSG_MOVE_UNSET_CAN_FLY,
    SMSG_TIME_SYNC_REQ, SMSG_UPDATE_OBJECT, UnsetCanFly, WorldEntryLocation,
    decode_authoritative_self_update, decode_force_move_root, decode_force_run_speed_change,
    decode_login_verify_world, decode_time_sync_request, decode_unset_can_fly,
    decode_unsupported_self_control_guid, encode_client_movement, encode_force_move_root_ack,
    encode_force_run_speed_change_ack, encode_move_set_can_fly_ack, encode_player_login,
    encode_time_sync_response,
};

/// The only client build accepted by the World-entry Slice.
pub const TARGET_CLIENT_BUILD: u16 = 12_340;

/// Authenticated realm-list request for the build-12340 login service.
pub const REALM_LIST_REQUEST: [u8; 5] = [0x10, 0, 0, 0, 0];

/// Encode the fixed-size client proof frame.
#[must_use]
pub fn encode_logon_proof(proof: &ClientSrpProof) -> [u8; 75] {
    let mut frame = [0_u8; 75];
    frame[0] = 0x01;
    frame[1..33].copy_from_slice(proof.client_public_key());
    frame[33..53].copy_from_slice(proof.client_proof());
    // 20-byte CRC proof remains zero on the controlled Reference Realm.
    // Telemetry-key count and security-token value are also zero.
    frame
}

#[cfg(test)]
mod tests {
    use super::TARGET_CLIENT_BUILD;

    #[test]
    fn target_build_is_the_locked_wrath_client_build() {
        assert_eq!(TARGET_CLIENT_BUILD, 12_340);
    }
}
