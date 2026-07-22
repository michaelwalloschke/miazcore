use std::{error::Error, fmt, io};

/// Stable failure surface for build-12340 protocol framing and validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolError {
    Io(io::ErrorKind),
    InvalidCredentialEncoding,
    MalformedFrame,
    MalformedWorldEntry { opcode: u16, byte_offset: usize },
    UnsupportedMovementState,
    UnsupportedSecurity,
    InvalidSrpParameters,
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(_) => formatter.write_str("protocol transport read or write failed"),
            Self::InvalidCredentialEncoding => {
                formatter.write_str("login credential encoding is unsupported")
            }
            Self::MalformedFrame => formatter.write_str("protocol frame is malformed"),
            Self::MalformedWorldEntry {
                opcode,
                byte_offset,
            } => write!(
                formatter,
                "world-entry opcode {opcode:#06x} is malformed at byte offset {byte_offset}"
            ),
            Self::UnsupportedMovementState => {
                formatter.write_str("world movement state is outside the controlled capability")
            }
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
