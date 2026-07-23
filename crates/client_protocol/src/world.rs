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

    #[must_use]
    pub fn into_incremental(self) -> IncrementalWorldServerDecoder {
        IncrementalWorldServerDecoder {
            cipher: self.cipher,
            buffered: Vec::new(),
        }
    }
}

/// Incremental encrypted World-frame decoder for retained sessions.
///
/// Cipher state advances only after a complete encrypted header has arrived,
/// so arbitrary socket fragmentation cannot desynchronize the stream.
pub struct IncrementalWorldServerDecoder {
    cipher: HeaderCipher,
    buffered: Vec<u8>,
}

impl IncrementalWorldServerDecoder {
    #[must_use]
    pub fn new(session_key: &[u8; 40]) -> Self {
        Self {
            cipher: HeaderCipher::new(HeaderDirection::ServerToClient, session_key),
            buffered: Vec::new(),
        }
    }

    /// Buffer received encrypted bytes.
    ///
    /// # Errors
    ///
    /// Returns an error when buffering would exceed the World-frame limit.
    pub fn push_bytes(&mut self, bytes: &[u8]) -> Result<(), ProtocolError> {
        if self.buffered.len().saturating_add(bytes.len()) > MAX_WORLD_FRAME_SIZE + 5 {
            return Err(ProtocolError::MalformedFrame);
        }
        self.buffered.extend_from_slice(bytes);
        Ok(())
    }

    /// Yield one complete frame, or `None` until enough bytes have arrived.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed or over-limit frame headers.
    pub fn next_frame(&mut self) -> Result<Option<WorldServerFrame>, ProtocolError> {
        let Some(&first_ciphertext) = self.buffered.first() else {
            return Ok(None);
        };
        let original_cipher = self.cipher.clone();
        let mut preview = original_cipher.clone();
        let mut first = [first_ciphertext];
        preview.apply(&mut first);
        let header_len = if first[0] & 0x80 != 0 { 5 } else { 4 };
        if self.buffered.len() < header_len {
            return Ok(None);
        }
        let mut header = self.buffered[..header_len].to_vec();
        self.cipher.apply(&mut header);
        let (size, opcode) = decode_server_header(&header)?;
        let payload_len = size - 2;
        let total_len = header_len.saturating_add(payload_len);
        if self.buffered.len() < total_len {
            // The cipher must not be committed until the full frame exists.
            // Rebuild it from the same session key state by retaining the
            // ciphertext and deferring commitment to a later call.
            self.cipher = original_cipher;
            return Ok(None);
        }
        let payload = self.buffered[header_len..total_len].to_vec();
        self.buffered.drain(..total_len);
        Ok(Some(WorldServerFrame { opcode, payload }))
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

fn decode_server_header(header: &[u8]) -> Result<(usize, u16), ProtocolError> {
    let (size, opcode) = match header {
        [first, a, b, opcode_low, opcode_high] if first & 0x80 != 0 => (
            (usize::from(first & 0x7f) << 16) | (usize::from(*a) << 8) | usize::from(*b),
            u16::from_le_bytes([*opcode_low, *opcode_high]),
        ),
        [first, size_low, opcode_low, opcode_high] if first & 0x80 == 0 => (
            (usize::from(*first) << 8) | usize::from(*size_low),
            u16::from_le_bytes([*opcode_low, *opcode_high]),
        ),
        _ => return Err(ProtocolError::MalformedFrame),
    };
    if !(2..=MAX_WORLD_FRAME_SIZE).contains(&size) {
        return Err(ProtocolError::MalformedFrame);
    }
    Ok((size, opcode))
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

#[cfg(test)]
#[allow(clippy::items_after_test_module)] // parsing helpers remain adjacent to production decoder code
mod tests {
    use super::IncrementalWorldServerDecoder;
    use crate::{HeaderCipher, HeaderDirection};

    const KEY: [u8; 40] = [7; 40];

    fn encrypted_server_frame(opcode: u16, payload: &[u8]) -> Vec<u8> {
        let size = u16::try_from(payload.len() + 2).unwrap();
        let mut header = vec![size.to_be_bytes()[0], size.to_be_bytes()[1]];
        header.extend_from_slice(&opcode.to_le_bytes());
        HeaderCipher::new(HeaderDirection::ServerToClient, &KEY).apply(&mut header);
        header.extend_from_slice(payload);
        header
    }

    fn encrypted_server_frames(frames: &[(u16, Vec<u8>)]) -> Vec<u8> {
        let mut cipher = HeaderCipher::new(HeaderDirection::ServerToClient, &KEY);
        let mut output = Vec::new();
        for (opcode, payload) in frames {
            let size = payload.len() + 2;
            let mut header = if size > 0x7fff {
                vec![
                    0x80 | u8::try_from(size >> 16).unwrap(),
                    u8::try_from(size >> 8).unwrap(),
                    u8::try_from(size & 0xff).unwrap(),
                    0,
                    0,
                ]
            } else {
                vec![
                    u8::try_from(size >> 8).unwrap(),
                    u8::try_from(size & 0xff).unwrap(),
                    0,
                    0,
                ]
            };
            let opcode_offset = header.len() - 2;
            header[opcode_offset..].copy_from_slice(&opcode.to_le_bytes());
            cipher.apply(&mut header);
            output.extend_from_slice(&header);
            output.extend_from_slice(payload);
        }
        output
    }

    #[test]
    fn incremental_decoder_keeps_cipher_aligned_across_fragmentation_and_coalescing() {
        let first = encrypted_server_frame(0x1234, &[1, 2, 3]);
        let second = encrypted_server_frame(0x5678, &[4]);
        // The second header is encrypted after the first header's cipher bytes.
        let mut cipher = HeaderCipher::new(HeaderDirection::ServerToClient, &KEY);
        let mut first_header = vec![0, 5, 0x34, 0x12];
        cipher.apply(&mut first_header);
        let mut second_header = vec![0, 3, 0x78, 0x56];
        cipher.apply(&mut second_header);
        let wire = [first_header, vec![1, 2, 3], second_header, vec![4]].concat();
        assert_ne!(wire, [first, second].concat());
        let mut decoder = IncrementalWorldServerDecoder::new(&KEY);
        for byte in &wire[..5] {
            decoder.push_bytes(&[*byte]).unwrap();
            assert!(decoder.next_frame().unwrap().is_none());
        }
        decoder.push_bytes(&wire[5..]).unwrap();
        let one = decoder.next_frame().unwrap().unwrap();
        let two = decoder.next_frame().unwrap().unwrap();
        assert_eq!((one.opcode(), one.payload()), (0x1234, &[1, 2, 3][..]));
        assert_eq!((two.opcode(), two.payload()), (0x5678, &[4][..]));
        assert!(decoder.next_frame().unwrap().is_none());
    }

    #[test]
    fn incremental_decoder_defers_partial_payload_without_losing_following_cipher_alignment() {
        let wire = encrypted_server_frames(&[(0x1111, vec![1, 2, 3]), (0x2222, vec![4, 5])]);
        let mut decoder = IncrementalWorldServerDecoder::new(&KEY);
        decoder.push_bytes(&wire[..5]).unwrap(); // complete header + one payload byte
        assert!(decoder.next_frame().unwrap().is_none());
        decoder.push_bytes(&wire[5..]).unwrap();
        let first = decoder.next_frame().unwrap().unwrap();
        let second = decoder.next_frame().unwrap().unwrap();
        assert_eq!((first.opcode(), first.payload()), (0x1111, &[1, 2, 3][..]));
        assert_eq!((second.opcode(), second.payload()), (0x2222, &[4, 5][..]));
    }

    #[test]
    fn incremental_decoder_handles_large_headers_and_rejects_overflowed_buffering() {
        let payload = vec![9; 32_766];
        let wire = encrypted_server_frames(&[(0x1234, payload.clone())]);
        let mut decoder = IncrementalWorldServerDecoder::new(&KEY);
        for byte in &wire[..5] {
            decoder.push_bytes(&[*byte]).unwrap();
            assert!(decoder.next_frame().unwrap().is_none());
        }
        decoder.push_bytes(&wire[5..]).unwrap();
        assert_eq!(
            decoder.next_frame().unwrap().unwrap().payload(),
            payload.as_slice()
        );
        assert!(decoder.push_bytes(&vec![0; 1024 * 1024 + 6]).is_err());
    }
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
