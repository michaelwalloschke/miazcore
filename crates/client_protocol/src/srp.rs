use std::fmt;

use crypto_bigint::{Encoding, Odd, U256, U512, Zero, modular::MontyForm, modular::MontyParams};
use sha1::{Digest, Sha1};
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, Zeroizing};

use crate::{LoginChallenge, ProtocolError};

const SHA1_LEN: usize = 20;

/// A zeroizing SRP session key which deliberately implements neither `Debug` nor `Display`.
pub struct SessionKey(Zeroizing<[u8; 40]>);

impl SessionKey {
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 40] {
        &self.0
    }
}

/// Client SRP values needed for the login proof exchange.
pub struct ClientSrpProof {
    client_public_key: [u8; 32],
    client_proof: [u8; 20],
    expected_server_proof: Zeroizing<[u8; 20]>,
    session_key: SessionKey,
}

impl Drop for ClientSrpProof {
    fn drop(&mut self) {
        self.client_proof.zeroize();
    }
}

impl ClientSrpProof {
    #[must_use]
    pub const fn client_public_key(&self) -> &[u8; 32] {
        &self.client_public_key
    }

    #[must_use]
    pub const fn client_proof(&self) -> &[u8; 20] {
        &self.client_proof
    }

    #[must_use]
    pub const fn session_key(&self) -> &SessionKey {
        &self.session_key
    }

    #[must_use]
    pub fn verify_server_proof(&self, received: &[u8; 20]) -> bool {
        bool::from(self.expected_server_proof.ct_eq(received))
    }
}

impl fmt::Debug for ClientSrpProof {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ClientSrpProof([REDACTED])")
    }
}

/// Calculate the Wrath SRP6 client proof from fixed caller-supplied entropy.
///
/// # Errors
///
/// Returns an error for empty credentials, a zero private value, invalid public
/// values, an even/zero prime, or a zero generator.
pub fn calculate_srp_client_proof(
    account: &[u8],
    password: &[u8],
    challenge: &LoginChallenge,
    private_ephemeral: [u8; 32],
) -> Result<ClientSrpProof, ProtocolError> {
    let private_ephemeral = Zeroizing::new(private_ephemeral);
    if account.is_empty()
        || password.is_empty()
        || !account.iter().all(u8::is_ascii_graphic)
        || !password.iter().all(u8::is_ascii_graphic)
    {
        return Err(ProtocolError::InvalidCredentialEncoding);
    }

    let mut prime = U256::from_le_bytes(challenge.prime);
    let odd_prime =
        Option::<Odd<U256>>::from(Odd::new(prime)).ok_or(ProtocolError::InvalidSrpParameters)?;
    let parameters = MontyParams::new_vartime(odd_prime);
    let mut generator_bytes = [0_u8; 32];
    generator_bytes[..challenge.generator.len()].copy_from_slice(&challenge.generator);
    let mut generator = U256::from_le_bytes(generator_bytes);
    let mut private = U256::from_le_bytes(*private_ephemeral);
    if bool::from(generator.is_zero()) || bool::from(private.is_zero()) {
        prime.zeroize();
        generator.zeroize();
        private.zeroize();
        return Err(ProtocolError::InvalidSrpParameters);
    }

    let mut server_public = U256::from_le_bytes(challenge.server_public_key);
    let reduced_server = server_public.rem(odd_prime.as_nz_ref());
    if bool::from(reduced_server.is_zero()) {
        zeroize_integers(&mut [&mut prime, &mut generator, &mut private, &mut server_public]);
        return Err(ProtocolError::InvalidSrpParameters);
    }

    let generator_form = MontyForm::new(&generator, parameters);
    let mut client_public = generator_form.pow(&private).retrieve();
    let client_public_key = client_public.to_le_bytes();

    let mut identity_hash = sha1(&[account, b":", password]);
    let mut x_hash = sha1(&[&challenge.salt, &identity_hash]);
    identity_hash.zeroize();
    let mut x_bytes = [0_u8; 32];
    x_bytes[..SHA1_LEN].copy_from_slice(&x_hash);
    x_hash.zeroize();
    let mut x = U256::from_le_bytes(x_bytes);
    x_bytes.zeroize();

    let mut scramble_hash = sha1(&[&client_public_key, &challenge.server_public_key]);
    let mut scramble_bytes = [0_u8; 32];
    scramble_bytes[..SHA1_LEN].copy_from_slice(&scramble_hash);
    scramble_hash.zeroize();
    let mut scramble = U256::from_le_bytes(scramble_bytes);
    scramble_bytes.zeroize();

    let mut gx = generator_form.pow(&x);
    let multiplier = MontyForm::new(&U256::from(3_u8), parameters);
    let mut base = (MontyForm::new(&server_public, parameters) - (gx * multiplier)).retrieve();
    let mut exponent: U512 = scramble.widening_mul(&x);
    exponent = exponent.wrapping_add(&U512::from(&private));
    let mut shared_secret = MontyForm::new(&base, parameters).pow(&exponent).retrieve();
    let mut shared_secret_bytes = shared_secret.to_le_bytes();
    let session_key_bytes = wow_interleave(&shared_secret_bytes);
    shared_secret_bytes.zeroize();

    let mut prime_hash = sha1(&[&challenge.prime]);
    let mut generator_hash = sha1(&[&challenge.generator]);
    for (prime_byte, generator_byte) in prime_hash.iter_mut().zip(generator_hash.iter()) {
        *prime_byte ^= generator_byte;
    }
    generator_hash.zeroize();
    let mut account_hash = sha1(&[account]);
    let client_proof = sha1(&[
        &prime_hash,
        &account_hash,
        &challenge.salt,
        &client_public_key,
        &challenge.server_public_key,
        &session_key_bytes,
    ]);
    prime_hash.zeroize();
    account_hash.zeroize();
    let expected_server_proof = sha1(&[&client_public_key, &client_proof, &session_key_bytes]);

    prime.zeroize();
    generator.zeroize();
    private.zeroize();
    server_public.zeroize();
    client_public.zeroize();
    x.zeroize();
    scramble.zeroize();
    gx.zeroize();
    base.zeroize();
    exponent.zeroize();
    shared_secret.zeroize();

    Ok(ClientSrpProof {
        client_public_key,
        client_proof,
        expected_server_proof: Zeroizing::new(expected_server_proof),
        session_key: SessionKey(Zeroizing::new(session_key_bytes)),
    })
}

fn sha1(parts: &[&[u8]]) -> [u8; 20] {
    let mut digest = Sha1::new();
    for part in parts {
        digest.update(part);
    }
    digest.finalize().into()
}

fn wow_interleave(shared_secret: &[u8; 32]) -> [u8; 40] {
    let mut lead = shared_secret
        .iter()
        .position(|byte| *byte != 0)
        .unwrap_or(shared_secret.len());
    if lead % 2 != 0 {
        lead += 1;
    }
    let trimmed = &shared_secret[lead.min(shared_secret.len())..];
    let mut even_input = Zeroizing::new(Vec::with_capacity(trimmed.len().div_ceil(2)));
    let mut odd_input = Zeroizing::new(Vec::with_capacity(trimmed.len() / 2));
    for (index, byte) in trimmed.iter().copied().enumerate() {
        if index % 2 == 0 {
            even_input.push(byte);
        } else {
            odd_input.push(byte);
        }
    }
    let mut even_hash = sha1(&[&even_input]);
    let mut odd_hash = sha1(&[&odd_input]);
    let mut session_key = [0_u8; 40];
    for index in 0..20 {
        session_key[index * 2] = even_hash[index];
        session_key[index * 2 + 1] = odd_hash[index];
    }
    even_hash.zeroize();
    odd_hash.zeroize();
    session_key
}

fn zeroize_integers(integers: &mut [&mut U256]) {
    for integer in integers {
        integer.zeroize();
    }
}
