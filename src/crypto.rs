use crate::env;
use crate::errors::Error;
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use std::convert::From;
use std::sync::OnceLock;
use tokio::task::spawn_blocking;

fn config() -> &'static argon2::Config<'static> {
    static DATA: OnceLock<argon2::Config> = OnceLock::new();

    DATA.get_or_init(|| argon2::Config {
        variant: argon2::Variant::Argon2i,
        version: argon2::Version::Version13,
        mem_cost: 65536,
        time_cost: 10,
        lanes: 4,
        thread_mode: argon2::ThreadMode::Parallel,
        secret: &[],
        ad: &[],
        hash_length: 32,
    })
}

fn salt() -> &'static String {
    static SALT: OnceLock<String> = OnceLock::new();

    SALT.get_or_init(env::password_hash_salt)
}

pub struct Encrypted {
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
}

#[derive(Clone)]
pub struct Password(Vec<u8>);

pub struct Plaintext(Vec<u8>);

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
    let key = argon2::hash_raw(password, salt().as_bytes(), config())?;
    let key = Key::from_slice(&key);
    Ok(XChaCha20Poly1305::new(key))
}

impl Encrypted {
    pub fn new(ciphertext: Vec<u8>, nonce: Vec<u8>) -> Self {
        Self { ciphertext, nonce }
    }

    pub async fn encrypt(password: Password, plaintext: Plaintext) -> Result<Self, Error> {
        spawn_blocking(move || {
            let cipher = cipher_from(&password.0)?;
            let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
            let ciphertext = cipher
                .encrypt(&nonce, plaintext.0.as_ref())
                .map_err(|_| Error::ChaCha20Poly1305)?;

            Ok(Self {
                ciphertext,
                nonce: nonce.to_vec(),
            })
        })
        .await?
    }

    pub async fn decrypt(self, password: Password) -> Result<Vec<u8>, Error> {
        spawn_blocking(move || {
            let cipher = cipher_from(&password.0)?;
            let nonce = XNonce::from_slice(&self.nonce);
            let plaintext = cipher
                .decrypt(nonce, self.ciphertext.as_ref())
                .map_err(|_| Error::ChaCha20Poly1305)?;
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
        let encrypted = Encrypted::encrypt(
            Password::from(password.as_bytes().to_vec()),
            Plaintext::from(plaintext.as_bytes().to_vec()),
        )
        .await
        .unwrap();
        let decrypted = encrypted
            .decrypt(Password::from(password.as_bytes().to_vec()))
            .await
            .unwrap();
        assert_eq!(decrypted, plaintext.as_bytes());
    }
}
