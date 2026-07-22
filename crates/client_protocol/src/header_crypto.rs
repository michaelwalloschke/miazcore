use std::fmt;

use hmac::{Hmac, Mac};
use sha1::Sha1;
use zeroize::Zeroize;

const CLIENT_DIRECTION_KEY: [u8; 16] = [
    0xc2, 0xb3, 0x72, 0x3c, 0xc6, 0xae, 0xd9, 0xb5, 0x34, 0x3c, 0x53, 0xee, 0x2f, 0x43, 0x67, 0xce,
];
const SERVER_DIRECTION_KEY: [u8; 16] = [
    0xcc, 0x98, 0xae, 0x04, 0xe8, 0x97, 0xea, 0xca, 0x12, 0xdd, 0xc0, 0x93, 0x42, 0x91, 0x53, 0x57,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeaderDirection {
    ClientToServer,
    ServerToClient,
}

/// Stateful Wrath header cipher. World-session integration is intentionally deferred.
pub struct HeaderCipher {
    state: [u8; 256],
    i: u8,
    j: u8,
}

impl HeaderCipher {
    /// Initialize an independent directional stream and discard its first 1024 bytes.
    ///
    /// # Panics
    ///
    /// Panics only if the HMAC implementation rejects the fixed 16-byte direction key,
    /// which HMAC-SHA1 accepts by definition.
    #[must_use]
    pub fn new(direction: HeaderDirection, session_key: &[u8; 40]) -> Self {
        let direction_key = match direction {
            HeaderDirection::ClientToServer => &CLIENT_DIRECTION_KEY,
            HeaderDirection::ServerToClient => &SERVER_DIRECTION_KEY,
        };
        let mut mac = Hmac::<Sha1>::new_from_slice(direction_key)
            .expect("fixed HMAC-SHA1 direction key is valid");
        mac.update(session_key);
        let mut rc4_key: [u8; 20] = mac.finalize().into_bytes().into();
        let mut cipher = Self::from_key(&rc4_key);
        rc4_key.zeroize();
        for _ in 0..1024 {
            let _ = cipher.next_byte();
        }
        cipher
    }

    pub fn apply(&mut self, bytes: &mut [u8]) {
        for byte in bytes {
            *byte ^= self.next_byte();
        }
    }

    fn from_key(key: &[u8; 20]) -> Self {
        let mut state = core::array::from_fn(|index| u8::try_from(index).unwrap());
        let mut j = 0_u8;
        for i in 0..256 {
            j = j.wrapping_add(state[i]).wrapping_add(key[i % key.len()]);
            state.swap(i, usize::from(j));
        }
        Self { state, i: 0, j: 0 }
    }

    fn next_byte(&mut self) -> u8 {
        self.i = self.i.wrapping_add(1);
        self.j = self.j.wrapping_add(self.state[usize::from(self.i)]);
        self.state.swap(usize::from(self.i), usize::from(self.j));
        let index = self.state[usize::from(self.i)].wrapping_add(self.state[usize::from(self.j)]);
        self.state[usize::from(index)]
    }
}

impl fmt::Debug for HeaderCipher {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HeaderCipher([REDACTED])")
    }
}

impl Drop for HeaderCipher {
    fn drop(&mut self) {
        self.state.zeroize();
        self.i.zeroize();
        self.j.zeroize();
    }
}
