//! Envelope encryption primitive used by the desktop control plane.
//!
//! The master key is deliberately supplied by the host. The Tauri layer keeps
//! that key in Keychain/Credential Manager (or a passphrase-derived key); this
//! crate only handles authenticated encryption of database values.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::{rngs::OsRng, RngCore};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("encrypted secret is malformed")]
    Malformed,
    #[error("encrypted secret could not be authenticated")]
    Authentication,
}

pub fn encrypt(master_key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>, SecretError> {
    let cipher = Aes256Gcm::new_from_slice(master_key).map_err(|_| SecretError::Malformed)?;
    let mut nonce = [0_u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext)
        .map_err(|_| SecretError::Authentication)?;
    let mut envelope = Vec::with_capacity(12 + ciphertext.len());
    envelope.extend_from_slice(&nonce);
    envelope.extend_from_slice(&ciphertext);
    Ok(envelope)
}

pub fn decrypt(master_key: &[u8; 32], envelope: &[u8]) -> Result<Vec<u8>, SecretError> {
    if envelope.len() < 12 {
        return Err(SecretError::Malformed);
    }
    let cipher = Aes256Gcm::new_from_slice(master_key).map_err(|_| SecretError::Malformed)?;
    cipher
        .decrypt(Nonce::from_slice(&envelope[..12]), &envelope[12..])
        .map_err(|_| SecretError::Authentication)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_round_trips_and_rejects_wrong_key() {
        let key = [7_u8; 32];
        let wrong = [8_u8; 32];
        let value = encrypt(&key, b"secret").unwrap();
        assert_eq!(decrypt(&key, &value).unwrap(), b"secret");
        assert!(decrypt(&wrong, &value).is_err());
    }
}
