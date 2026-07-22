use std::{error::Error, fmt, io, io::Read};

use crate::TARGET_CLIENT_BUILD;

const MAX_REALM_PAYLOAD: usize = u16::MAX as usize;
const MAX_REALMS: usize = 128;

/// Stable failure surface for build-12340 login framing and SRP validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolError {
    Io(io::ErrorKind),
    InvalidCredentialEncoding,
    MalformedFrame,
    UnsupportedSecurity,
    InvalidSrpParameters,
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(_) => formatter.write_str("login transport read or write failed"),
            Self::InvalidCredentialEncoding => {
                formatter.write_str("login credential encoding is unsupported")
            }
            Self::MalformedFrame => formatter.write_str("login frame is malformed"),
            Self::UnsupportedSecurity => {
                formatter.write_str("login server requested an unsupported security method")
            }
            Self::InvalidSrpParameters => formatter.write_str("SRP6 parameters are invalid"),
        }
    }
}

impl Error for ProtocolError {}

impl From<io::Error> for ProtocolError {
    fn from(error: io::Error) -> Self {
        Self::Io(error.kind())
    }
}

/// Public SRP values returned by an accepted login challenge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoginChallenge {
    pub(crate) server_public_key: [u8; 32],
    pub(crate) generator: Vec<u8>,
    pub(crate) prime: [u8; 32],
    pub(crate) salt: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoginChallengeResponse {
    Accepted(LoginChallenge),
    Rejected { result: u8 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoginProofResponse {
    Accepted { server_proof: [u8; 20] },
    Rejected { result: u8 },
}

/// One project-owned realm-list record.
#[derive(Clone, Debug, PartialEq)]
pub struct RealmEntry {
    id: u8,
    name: String,
    address: String,
    locked: bool,
    build: Option<u16>,
}

impl RealmEntry {
    #[must_use]
    pub const fn id(&self) -> u8 {
        self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn address(&self) -> &str {
        &self.address
    }

    #[must_use]
    pub const fn is_locked(&self) -> bool {
        self.locked
    }

    #[must_use]
    pub const fn build(&self) -> Option<u16> {
        self.build
    }
}

/// Encode the build-12340 login challenge for an already normalized account name.
///
/// # Errors
///
/// Returns an error if the account cannot be represented by the one-byte legacy
/// length or contains bytes outside the supported visible ASCII range.
pub fn encode_logon_challenge(account: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    let account_len =
        u8::try_from(account.len()).map_err(|_| ProtocolError::InvalidCredentialEncoding)?;
    if account.is_empty() || !account.iter().all(u8::is_ascii_graphic) {
        return Err(ProtocolError::InvalidCredentialEncoding);
    }

    let tail_len = 30_usize
        .checked_add(account.len())
        .ok_or(ProtocolError::MalformedFrame)?;
    let tail_len = u16::try_from(tail_len).map_err(|_| ProtocolError::MalformedFrame)?;
    let mut frame = Vec::with_capacity(usize::from(tail_len) + 4);
    frame.extend_from_slice(&[0x00, 0x08]);
    frame.extend_from_slice(&tail_len.to_le_bytes());
    frame.extend_from_slice(&0x0057_6f57_u32.to_le_bytes()); // "WoW\0"
    frame.extend_from_slice(&[3, 3, 5]);
    frame.extend_from_slice(&TARGET_CLIENT_BUILD.to_le_bytes());
    frame.extend_from_slice(&0x0078_3836_u32.to_le_bytes()); // "x86\0"
    frame.extend_from_slice(&0x004f_5358_u32.to_le_bytes()); // "OSX\0"
    frame.extend_from_slice(&0x656e_5553_u32.to_le_bytes()); // "enUS"
    frame.extend_from_slice(&0_i32.to_le_bytes());
    frame.extend_from_slice(&[127, 0, 0, 1]);
    frame.push(account_len);
    frame.extend_from_slice(account);
    Ok(frame)
}

/// Read one complete login challenge response, tolerating arbitrary transport fragmentation.
///
/// # Errors
///
/// Returns a stable protocol error for truncated, malformed, or unsupported frames.
pub fn read_logon_challenge_response(
    reader: &mut impl Read,
) -> Result<LoginChallengeResponse, ProtocolError> {
    let header = read_array::<3>(reader)?;
    if header[0] != 0x00 {
        return Err(ProtocolError::MalformedFrame);
    }
    if header[2] != 0 {
        return Ok(LoginChallengeResponse::Rejected { result: header[2] });
    }

    let server_public_key = read_array::<32>(reader)?;
    let generator_len = usize::from(read_array::<1>(reader)?[0]);
    if !(1..=32).contains(&generator_len) {
        return Err(ProtocolError::InvalidSrpParameters);
    }
    let mut generator = vec![0_u8; generator_len];
    reader.read_exact(&mut generator)?;
    let prime_len = usize::from(read_array::<1>(reader)?[0]);
    if prime_len != 32 {
        return Err(ProtocolError::InvalidSrpParameters);
    }
    let prime = read_array::<32>(reader)?;
    let salt = read_array::<32>(reader)?;
    let _crc_salt = read_array::<16>(reader)?;
    let security_flags = read_array::<1>(reader)?[0];
    if security_flags != 0 {
        return Err(ProtocolError::UnsupportedSecurity);
    }
    Ok(LoginChallengeResponse::Accepted(LoginChallenge {
        server_public_key,
        generator,
        prime,
        salt,
    }))
}

/// Read one complete proof response.
///
/// # Errors
///
/// Returns a stable protocol error for a wrong opcode or truncated accepted frame.
pub fn read_logon_proof_response(
    reader: &mut impl Read,
) -> Result<LoginProofResponse, ProtocolError> {
    let header = read_array::<2>(reader)?;
    if header[0] != 0x01 {
        return Err(ProtocolError::MalformedFrame);
    }
    if header[1] != 0 {
        return Ok(LoginProofResponse::Rejected { result: header[1] });
    }
    let server_proof = read_array::<20>(reader)?;
    let _account_metadata = read_array::<10>(reader)?;
    Ok(LoginProofResponse::Accepted { server_proof })
}

/// Decode a complete authenticated realm-list response and consume its entire body.
///
/// # Errors
///
/// Returns a stable protocol error for invalid lengths, strings, counts, or trailing bytes.
pub fn read_realm_list_response(reader: &mut impl Read) -> Result<Vec<RealmEntry>, ProtocolError> {
    let header = read_array::<3>(reader)?;
    if header[0] != 0x10 {
        return Err(ProtocolError::MalformedFrame);
    }
    let payload_len = usize::from(u16::from_le_bytes([header[1], header[2]]));
    if !(6..=MAX_REALM_PAYLOAD).contains(&payload_len) {
        return Err(ProtocolError::MalformedFrame);
    }
    let mut payload = vec![0_u8; payload_len];
    reader.read_exact(&mut payload)?;
    decode_realm_payload(&payload)
}

fn decode_realm_payload(payload: &[u8]) -> Result<Vec<RealmEntry>, ProtocolError> {
    let mut cursor = SliceCursor::new(payload);
    cursor.skip(4)?;
    let realm_count = usize::from(cursor.u16()?);
    if realm_count > MAX_REALMS {
        return Err(ProtocolError::MalformedFrame);
    }
    let mut realms = Vec::with_capacity(realm_count);
    for _ in 0..realm_count {
        let _realm_type = cursor.u8()?;
        let locked = cursor.u8()? != 0;
        let flags = cursor.u8()?;
        let name = cursor.cstring()?;
        let address = cursor.cstring()?;
        let population = f32::from_le_bytes(cursor.array::<4>()?);
        if !population.is_finite() {
            return Err(ProtocolError::MalformedFrame);
        }
        let _characters = cursor.u8()?;
        let _timezone = cursor.u8()?;
        let id = cursor.u8()?;
        let build = if flags & 0x04 != 0 {
            let _major = cursor.u8()?;
            let _minor = cursor.u8()?;
            let _patch = cursor.u8()?;
            Some(cursor.u16()?)
        } else {
            None
        };
        realms.push(RealmEntry {
            id,
            name,
            address,
            locked,
            build,
        });
    }

    match cursor.remaining() {
        0 => {}
        2 if cursor.array::<2>()? == [0x10, 0x00] => {}
        _ => return Err(ProtocolError::MalformedFrame),
    }
    if cursor.remaining() != 0 {
        return Err(ProtocolError::MalformedFrame);
    }
    Ok(realms)
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

    fn u16(&mut self) -> Result<u16, ProtocolError> {
        Ok(u16::from_le_bytes(self.array::<2>()?))
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
        let bytes = self.take(length + 1)?;
        let value = &bytes[..length];
        if value.is_empty() || !value.iter().all(|byte| (0x20..=0x7e).contains(byte)) {
            return Err(ProtocolError::MalformedFrame);
        }
        String::from_utf8(value.to_vec()).map_err(|_| ProtocolError::MalformedFrame)
    }
}
