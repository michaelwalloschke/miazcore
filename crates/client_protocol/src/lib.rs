//! Engine-independent protocol boundary for the Learning Client.
//!
//! This crate owns byte framing and cryptographic compatibility with the pinned
//! build-12340 Reference Realm. It deliberately owns no socket, runtime, engine,
//! or application-boundary types.

mod header_crypto;
mod login;
mod srp;
mod world;

pub use header_crypto::{HeaderCipher, HeaderDirection};
pub use login::{
    LoginChallenge, LoginChallengeResponse, LoginProofResponse, ProtocolError, RealmEntry,
    encode_logon_challenge, read_logon_challenge_response, read_logon_proof_response,
    read_realm_list_response,
};
pub use srp::{ClientSrpProof, SessionKey, calculate_srp_client_proof};
pub use world::{
    CMSG_AUTH_SESSION, CMSG_CHAR_ENUM, SMSG_AUTH_CHALLENGE, SMSG_AUTH_RESPONSE, SMSG_CHAR_ENUM,
    WorldAuthChallenge, WorldAuthResponse, WorldCharacter, WorldClientStream, WorldServerFrame,
    WorldServerStream, decode_character_enumeration, decode_world_auth_challenge,
    decode_world_auth_response, encode_world_auth_session_frame, read_plain_world_server_frame,
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
