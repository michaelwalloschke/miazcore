use std::{fmt, io::Read};

use sha1::{Digest, Sha1};
use zeroize::{Zeroize, Zeroizing};

use crate::{HeaderCipher, HeaderDirection, ProtocolError, TARGET_CLIENT_BUILD};

const MAX_WORLD_FRAME_SIZE: usize = 1024 * 1024;
const MAX_CHARACTERS: usize = 50;
const CHARACTER_EQUIPMENT_BYTES: usize = 23 * 9;
const AUTH_OK: u8 = 0x0c;

pub const SMSG_AUTH_CHALLENGE: u16 = 0x01ec;
pub const CMSG_AUTH_SESSION: u32 = 0x01ed;
pub const SMSG_AUTH_RESPONSE: u16 = 0x01ee;
pub const CMSG_CHAR_ENUM: u32 = 0x0037;
pub const SMSG_CHAR_ENUM: u16 = 0x003b;

/// One complete world-server packet with its decrypted header and plaintext payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorldServerFrame {
    opcode: u16,
    payload: Vec<u8>,
}

impl WorldServerFrame {
    #[must_use]
    pub const fn opcode(&self) -> u16 {
        self.opcode
    }

    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

/// Stateful server-to-client header stream.
pub struct WorldServerStream {
    cipher: HeaderCipher,
}

impl WorldServerStream {
    #[must_use]
    pub fn new(session_key: &[u8; 40]) -> Self {
        Self {
            cipher: HeaderCipher::new(HeaderDirection::ServerToClient, session_key),
        }
    }

    /// Read exactly one complete packet before returning it to the caller.
    ///
    /// # Errors
    ///
    /// Returns an error for truncated, malformed, or over-limit headers and payloads.
    pub fn read_frame(
        &mut self,
        reader: &mut impl Read,
    ) -> Result<WorldServerFrame, ProtocolError> {
        read_server_frame(reader, Some(&mut self.cipher))
    }
}

impl fmt::Debug for WorldServerStream {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("WorldServerStream([REDACTED])")
    }
}

/// Stateful client-to-server header stream.
pub struct WorldClientStream {
    cipher: HeaderCipher,
}

impl WorldClientStream {
    #[must_use]
    pub fn new(session_key: &[u8; 40]) -> Self {
        Self {
            cipher: HeaderCipher::new(HeaderDirection::ClientToServer, session_key),
        }
    }

    /// Encode one packet while advancing the directional cipher exactly once.
    ///
    /// # Errors
    ///
    /// Returns an error when the complete client packet cannot use a normal Wrath header.
    pub fn encode_frame(&mut self, opcode: u32, payload: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        encode_client_frame(opcode, payload, Some(&mut self.cipher))
    }
}

impl fmt::Debug for WorldClientStream {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("WorldClientStream([REDACTED])")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorldAuthChallenge {
    server_seed: u32,
}

impl WorldAuthChallenge {
    #[must_use]
    pub const fn server_seed(self) -> u32 {
        self.server_seed
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorldAuthResponse {
    Accepted,
    Rejected,
}

/// Project-owned character-enumeration record.
#[derive(Clone, Debug, PartialEq)]
pub struct WorldCharacter {
    guid: u64,
    name: String,
    race: u8,
    class: u8,
    gender: u8,
    level: u8,
    area_id: u32,
    map_id: u32,
    position: [f32; 3],
    flags: u32,
}

impl WorldCharacter {
    #[must_use]
    pub const fn guid(&self) -> u64 {
        self.guid
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn race(&self) -> u8 {
        self.race
    }

    #[must_use]
    pub const fn class(&self) -> u8 {
        self.class
    }

    #[must_use]
    pub const fn gender(&self) -> u8 {
        self.gender
    }

    #[must_use]
    pub const fn level(&self) -> u8 {
        self.level
    }

    #[must_use]
    pub const fn area_id(&self) -> u32 {
        self.area_id
    }

    #[must_use]
    pub const fn map_id(&self) -> u32 {
        self.map_id
    }

    #[must_use]
    pub const fn position(&self) -> [f32; 3] {
        self.position
    }

    #[must_use]
    pub const fn flags(&self) -> u32 {
        self.flags
    }
}

/// Read one unencrypted world-server frame.
///
/// # Errors
///
/// Returns an error for a truncated, malformed, or over-limit frame.
pub fn read_plain_world_server_frame(
    reader: &mut impl Read,
) -> Result<WorldServerFrame, ProtocolError> {
    read_server_frame(reader, None)
}

/// Decode the fixed build-12340 world challenge.
///
/// # Errors
///
/// Returns an error for a wrong opcode, wrong body size, or unexpected marker.
pub fn decode_world_auth_challenge(
    frame: &WorldServerFrame,
) -> Result<WorldAuthChallenge, ProtocolError> {
    if frame.opcode != SMSG_AUTH_CHALLENGE || frame.payload.len() != 40 {
        return Err(ProtocolError::MalformedFrame);
    }
    let marker = u32::from_le_bytes(
        frame.payload[0..4]
            .try_into()
            .map_err(|_| ProtocolError::MalformedFrame)?,
    );
    if marker != 1 {
        return Err(ProtocolError::MalformedFrame);
    }
    let server_seed = u32::from_le_bytes(
        frame.payload[4..8]
            .try_into()
            .map_err(|_| ProtocolError::MalformedFrame)?,
    );
    Ok(WorldAuthChallenge { server_seed })
}

/// Encode the complete unencrypted `CMSG_AUTH_SESSION` frame.
///
/// # Errors
///
/// Returns an error for an invalid account encoding or over-limit frame.
pub fn encode_world_auth_session_frame(
    account: &[u8],
    realm_id: u32,
    client_seed: u32,
    server_seed: u32,
    session_key: &[u8; 40],
) -> Result<Zeroizing<Vec<u8>>, ProtocolError> {
    if account.is_empty()
        || account.len() > 64
        || !account.iter().all(u8::is_ascii_graphic)
        || account.contains(&0)
    {
        return Err(ProtocolError::InvalidCredentialEncoding);
    }

    let mut digest = Sha1::new();
    digest.update(account);
    digest.update(0_u32.to_le_bytes());
    digest.update(client_seed.to_le_bytes());
    digest.update(server_seed.to_le_bytes());
    digest.update(session_key);
    let mut proof: [u8; 20] = digest.finalize().into();

    let mut body = Zeroizing::new(Vec::with_capacity(61 + account.len()));
    body.extend_from_slice(&u32::from(TARGET_CLIENT_BUILD).to_le_bytes());
    body.extend_from_slice(&0_u32.to_le_bytes());
    body.extend_from_slice(account);
    body.push(0);
    body.extend_from_slice(&0_u32.to_le_bytes());
    body.extend_from_slice(&client_seed.to_le_bytes());
    body.extend_from_slice(&0_u32.to_le_bytes());
    body.extend_from_slice(&0_u32.to_le_bytes());
    body.extend_from_slice(&realm_id.to_le_bytes());
    body.extend_from_slice(&0_u64.to_le_bytes());
    body.extend_from_slice(&proof);
    body.extend_from_slice(&0_u32.to_le_bytes());
    proof.zeroize();

    let frame = encode_client_frame(CMSG_AUTH_SESSION, &body, None)?;
    Ok(Zeroizing::new(frame))
}

/// Decode a complete `SMSG_AUTH_RESPONSE` payload.
///
/// # Errors
///
/// Returns an error for an empty or malformed accepted response.
pub fn decode_world_auth_response(payload: &[u8]) -> Result<WorldAuthResponse, ProtocolError> {
    let result = *payload.first().ok_or(ProtocolError::MalformedFrame)?;
    if result == AUTH_OK {
        if payload.len() != 11 {
            return Err(ProtocolError::MalformedFrame);
        }
        Ok(WorldAuthResponse::Accepted)
    } else {
        Ok(WorldAuthResponse::Rejected)
    }
}

/// Decode all complete Wrath character-enumeration records.
///
/// # Errors
///
/// Returns an error for invalid counts, strings, scalar values, truncation, or trailing bytes.
pub fn decode_character_enumeration(payload: &[u8]) -> Result<Vec<WorldCharacter>, ProtocolError> {
    let mut cursor = SliceCursor::new(payload);
    let count = usize::from(cursor.u8()?);
    if count > MAX_CHARACTERS {
        return Err(ProtocolError::MalformedFrame);
    }

    let mut characters = Vec::with_capacity(count);
    for _ in 0..count {
        let guid = cursor.u64()?;
        let name = cursor.cstring()?;
        let race = cursor.u8()?;
        let class = cursor.u8()?;
        let gender = cursor.u8()?;
        cursor.skip(5)?;
        let level = cursor.u8()?;
        let area_id = cursor.u32()?;
        let map_id = cursor.u32()?;
        let position = [cursor.f32()?, cursor.f32()?, cursor.f32()?];
        if !position.iter().all(|coordinate| coordinate.is_finite()) {
            return Err(ProtocolError::MalformedFrame);
        }
        cursor.skip(4)?;
        let flags = cursor.u32()?;
        cursor.skip(4)?;
        if cursor.u8()? > 1 {
            return Err(ProtocolError::MalformedFrame);
        }
        cursor.skip(12)?;
        cursor.skip(CHARACTER_EQUIPMENT_BYTES)?;
        characters.push(WorldCharacter {
            guid,
            name,
            race,
            class,
            gender,
            level,
            area_id,
            map_id,
            position,
            flags,
        });
    }
    if cursor.remaining() != 0 {
        return Err(ProtocolError::MalformedFrame);
    }
    Ok(characters)
}

fn read_server_frame(
    reader: &mut impl Read,
    mut cipher: Option<&mut HeaderCipher>,
) -> Result<WorldServerFrame, ProtocolError> {
    let mut first = read_array::<1>(reader)?;
    apply_optional_cipher(&mut cipher, &mut first);
    let large = first[0] & 0x80 != 0;
    let (size, opcode) = if large {
        let mut remainder = read_array::<4>(reader)?;
        apply_optional_cipher(&mut cipher, &mut remainder);
        let size = (usize::from(first[0] & 0x7f) << 16)
            | (usize::from(remainder[0]) << 8)
            | usize::from(remainder[1]);
        let opcode = u16::from_le_bytes([remainder[2], remainder[3]]);
        (size, opcode)
    } else {
        let mut remainder = read_array::<3>(reader)?;
        apply_optional_cipher(&mut cipher, &mut remainder);
        let size = (usize::from(first[0]) << 8) | usize::from(remainder[0]);
        let opcode = u16::from_le_bytes([remainder[1], remainder[2]]);
        (size, opcode)
    };
    if !(2..=MAX_WORLD_FRAME_SIZE).contains(&size) {
        return Err(ProtocolError::MalformedFrame);
    }
    let payload_len = size - 2;
    let mut payload = vec![0_u8; payload_len];
    reader.read_exact(&mut payload)?;
    Ok(WorldServerFrame { opcode, payload })
}

fn apply_optional_cipher(cipher: &mut Option<&mut HeaderCipher>, bytes: &mut [u8]) {
    if let Some(cipher) = cipher.as_deref_mut() {
        cipher.apply(bytes);
    }
}

fn encode_client_frame(
    opcode: u32,
    payload: &[u8],
    mut cipher: Option<&mut HeaderCipher>,
) -> Result<Vec<u8>, ProtocolError> {
    let size = payload
        .len()
        .checked_add(4)
        .and_then(|size| u16::try_from(size).ok())
        .ok_or(ProtocolError::MalformedFrame)?;
    let mut header = [0_u8; 6];
    header[..2].copy_from_slice(&size.to_be_bytes());
    header[2..].copy_from_slice(&opcode.to_le_bytes());
    apply_optional_cipher(&mut cipher, &mut header);
    let mut frame = Vec::with_capacity(header.len() + payload.len());
    frame.extend_from_slice(&header);
    frame.extend_from_slice(payload);
    Ok(frame)
}

fn read_array<const N: usize>(reader: &mut impl Read) -> Result<[u8; N], ProtocolError> {
    let mut bytes = [0_u8; N];
    reader.read_exact(&mut bytes)?;
    Ok(bytes)
}

struct SliceCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> SliceCursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    fn skip(&mut self, count: usize) -> Result<(), ProtocolError> {
        let _ = self.take(count)?;
        Ok(())
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], ProtocolError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(ProtocolError::MalformedFrame)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(ProtocolError::MalformedFrame)?;
        self.offset = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], ProtocolError> {
        self.take(N)?
            .try_into()
            .map_err(|_| ProtocolError::MalformedFrame)
    }

    fn u8(&mut self) -> Result<u8, ProtocolError> {
        Ok(self.array::<1>()?[0])
    }

    fn u32(&mut self) -> Result<u32, ProtocolError> {
        Ok(u32::from_le_bytes(self.array::<4>()?))
    }

    fn u64(&mut self) -> Result<u64, ProtocolError> {
        Ok(u64::from_le_bytes(self.array::<8>()?))
    }

    fn f32(&mut self) -> Result<f32, ProtocolError> {
        Ok(f32::from_le_bytes(self.array::<4>()?))
    }

    fn cstring(&mut self) -> Result<String, ProtocolError> {
        let remainder = self
            .bytes
            .get(self.offset..)
            .ok_or(ProtocolError::MalformedFrame)?;
        let length = remainder
            .iter()
            .position(|byte| *byte == 0)
            .ok_or(ProtocolError::MalformedFrame)?;
        if !(1..=64).contains(&length) {
            return Err(ProtocolError::MalformedFrame);
        }
        let bytes = self.take(length + 1)?;
        let value = &bytes[..length];
        if !value.iter().all(|byte| (0x20..=0x7e).contains(byte)) {
            return Err(ProtocolError::MalformedFrame);
        }
        String::from_utf8(value.to_vec()).map_err(|_| ProtocolError::MalformedFrame)
    }
}
