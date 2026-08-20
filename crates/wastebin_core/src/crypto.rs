use std::sync::LazyLock;

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use rand::RngExt;
use tokio::sync::Semaphore;
use tokio::task::spawn_blocking;

use crate::env;

static CONFIG: LazyLock<argon2::Config> = LazyLock::new(|| argon2::Config {
    variant: argon2::Variant::Argon2i,
    version: argon2::Version::Version13,
    mem_cost: 65536,
    time_cost: 10,
    lanes: 4,
    thread_mode: argon2::ThreadMode::Parallel,
    secret: &[],
    ad: &[],
    hash_length: 32,
});

static SALT: LazyLock<String> = LazyLock::new(env::password_hash_salt);

/// Bounds how many Argon2 hashes run concurrently. Each hash reserves `mem_cost`
/// (64 MiB) and keeps several cores busy, and every password attempt triggers
/// one. Without a cap, a burst of attempts would be limited only by the tokio
/// blocking pool (up to 512 threads), enough to pin the machine's memory and
/// CPU. Cap concurrency at the core count instead, independent of that pool.
static ARGON2_PERMITS: LazyLock<Semaphore> = LazyLock::new(|| {
    let permits = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1);
    Semaphore::new(permits)
});

/// Encryption or decryption errors.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to hash with argon2: {0}")]
    Argon2(#[from] argon2::Error),
    #[error("failed to encrypt")]
    ChaCha20Poly1305Encrypt,
    #[error("failed to decrypt")]
    ChaCha20Poly1305Decrypt,
    #[error("failed to acquire argon2 permit")]
    Semaphore,
    #[error("join error: {0}")]
    Join(#[from] tokio::task::JoinError),
}

/// Encrypted data item.
pub(crate) struct Encrypted {
    /// Encrypted ciphertext.
    pub ciphertext: Vec<u8>,
    /// Nonce used for encryption.
    pub nonce: XNonce,
}

pub struct Password(Vec<u8>);

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
    let key = Key::try_from(key.as_slice()).map_err(|_| Error::ChaCha20Poly1305Encrypt)?;
    Ok(XChaCha20Poly1305::new(&key))
}

impl Plaintext {
    /// Consume and encrypt plaintext into [`Encrypted`] using `password`.
    pub async fn encrypt(self, password: Password) -> Result<Encrypted, Error> {
        let permit = ARGON2_PERMITS
            .acquire()
            .await
            .map_err(|_| Error::Semaphore)?;

        let result = spawn_blocking(move || {
            let cipher = cipher_from(&password.0)?;
            let nonce = XNonce::from(rand::rng().random::<[u8; 24]>());
            let ciphertext = cipher
                .encrypt(&nonce, self.0.as_ref())
                .map_err(|_| Error::ChaCha20Poly1305Encrypt)?;

            Ok(Encrypted::new(ciphertext, nonce))
        })
        .await;

        drop(permit);
        result?
    }
}

impl Encrypted {
    /// Create new [`Encrypted`] item from `ciphertext` and `nonce`.
    pub fn new(ciphertext: Vec<u8>, nonce: XNonce) -> Self {
        Self { ciphertext, nonce }
    }

    /// Decrypt into bytes using `password`.
    pub async fn decrypt(self, password: Password) -> Result<Vec<u8>, Error> {
        let permit = ARGON2_PERMITS
            .acquire()
            .await
            .map_err(|_| Error::Semaphore)?;

        let result = spawn_blocking(move || {
            let cipher = cipher_from(&password.0)?;
            let nonce = XNonce::try_from(self.nonce.as_slice())
                .map_err(|_| Error::ChaCha20Poly1305Decrypt)?;
            let plaintext = cipher
                .decrypt(&nonce, self.ciphertext.as_ref())
                .map_err(|_| Error::ChaCha20Poly1305Decrypt)?;
            Ok(plaintext)
        })
        .await;

        drop(permit);
        result?
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

    /// The permit is released after each operation, so more concurrent
    /// round-trips than there are permits still all complete (rather than
    /// deadlocking on a leaked permit).
    #[tokio::test(flavor = "multi_thread")]
    async fn concurrent_roundtrips_all_complete() {
        let handles = (0..8u8)
            .map(|i| {
                tokio::spawn(async move {
                    let plaintext = format!("secret-{i}").into_bytes();
                    let encrypted = Plaintext::from(plaintext.clone())
                        .encrypt(Password::from(b"pw".to_vec()))
                        .await
                        .unwrap();
                    let decrypted = encrypted
                        .decrypt(Password::from(b"pw".to_vec()))
                        .await
                        .unwrap();
                    assert_eq!(decrypted, plaintext);
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            handle.await.unwrap();
        }
    }
}
