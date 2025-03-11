use crate::env;
use crate::errors::Error;
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use std::sync::LazyLock;
use tokio::task::spawn_blocking;

static CONFIG: LazyLock<argon2::Config> = LazyLock::new(|| argon2::Config {
    variant: argon2::Variant::Argon2i,
    version: argon2::Version::Version13,
    mem_cost: 65536,
    time_cost: 10,
    lanes: 4,
    secret: &[],
    ad: &[],
    hash_length: 32,
});

static SALT: LazyLock<String> = LazyLock::new(env::password_hash_salt);

/// Encrypted data item.
pub(crate) struct Encrypted {
    /// Encrypted ciphertext.
    pub ciphertext: Vec<u8>,
    /// Nonce used for encryption.
    pub nonce: Vec<u8>,
}

#[derive(Clone)]
pub(crate) struct Password(Vec<u8>);

/// Plaintext bytes to be encrypted.
pub(crate) struct Plaintext(Vec<u8>);

impl From<Vec<u8>> for Password {
    fn from(value: Vec<u8>) -> Self {
        Self(value)
    }
}

impl From<Vec<u8>> for Plaintext {
    fn from(value: Vec<u8>) -> Self {
        Self(value)
    }
}

fn cipher_from(password: &[u8]) -> Result<XChaCha20Poly1305, Error> {
    let key = argon2::hash_raw(password, SALT.as_bytes(), &CONFIG)?;
    let key = Key::from_slice(&key);
    Ok(XChaCha20Poly1305::new(key))
}

impl Plaintext {
    /// Consume and encrypt plaintext into [`Encrypted`] using `password`.
    pub async fn encrypt(self, password: Password) -> Result<Encrypted, Error> {
        spawn_blocking(move || {
            let cipher = cipher_from(&password.0)?;
            let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
            let ciphertext = cipher
                .encrypt(&nonce, self.0.as_ref())
                .map_err(|_| Error::ChaCha20Poly1305Encrypt)?;

            Ok(Encrypted::new(ciphertext, nonce.to_vec()))
        })
        .await?
    }
}

impl Encrypted {
    /// Create new [`Encrypted`] item from `ciphertext` and `nonce`.
    pub fn new(ciphertext: Vec<u8>, nonce: Vec<u8>) -> Self {
        Self { ciphertext, nonce }
    }

    /// Decrypt into bytes using `password`.
    pub async fn decrypt(self, password: Password) -> Result<Vec<u8>, Error> {
        spawn_blocking(move || {
            let cipher = cipher_from(&password.0)?;
            let nonce = XNonce::from_slice(&self.nonce);
            let plaintext = cipher
                .decrypt(nonce, self.ciphertext.as_ref())
                .map_err(|_| Error::ChaCha20Poly1305Decrypt)?;
            Ok(plaintext)
        })
        .await?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn roundtrip() {
        let password = "secret".to_string();
        let plaintext = "encrypt me".to_string();
        let encrypted = Plaintext::from(plaintext.as_bytes().to_vec())
            .encrypt(Password::from(password.as_bytes().to_vec()))
            .await
            .unwrap();
        let decrypted = encrypted
            .decrypt(Password::from(password.as_bytes().to_vec()))
            .await
            .unwrap();
        assert_eq!(decrypted, plaintext.as_bytes());
    }
}
