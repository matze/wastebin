use std::sync::LazyLock;

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use rand::RngExt;
use tokio::task::spawn_blocking;

use crate::env;

/// Derivation used for entries sealed before each carried a salt of its own.
///
/// Those entries are already in databases, and their key depends on every value here, so this
/// cannot change without making them unreadable.
static LEGACY_CONFIG: LazyLock<argon2::Config> = LazyLock::new(|| argon2::Config {
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

/// Derivation for entries that carry their own salt.
///
/// Argon2id rather than Argon2i: RFC 9106 §4 recommends it, and its hybrid addressing is what
/// resists the tradeoff attacks that Argon2i's data-independent addressing invites.
static CONFIG: LazyLock<argon2::Config> = LazyLock::new(|| argon2::Config {
    variant: argon2::Variant::Argon2id,
    version: argon2::Version::Version13,
    mem_cost: 65536,
    time_cost: 10,
    lanes: 4,
    thread_mode: argon2::ThreadMode::Parallel,
    secret: &[],
    ad: &[],
    hash_length: 32,
});

/// Process-wide salt, used only for entries sealed before each carried its own.
static SALT: LazyLock<String> = LazyLock::new(env::password_hash_salt);

/// Bytes of salt minted for a new entry.
const ENTRY_SALT_LEN: usize = 16;

/// Encryption or decryption errors.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to hash with argon2: {0}")]
    Argon2(#[from] argon2::Error),
    #[error("failed to encrypt")]
    ChaCha20Poly1305Encrypt,
    #[error("failed to decrypt")]
    ChaCha20Poly1305Decrypt,
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

/// How an entry's key is derived.
///
/// One salt shared by every entry made a single argon2 evaluation per candidate password testable
/// against the whole database at once, and gave two pastes with the same password the same key.
/// New entries carry their own; the ones already stored keep the derivation they were sealed with.
#[derive(Clone, Debug)]
pub(crate) enum Derivation {
    /// An entry from before per-entry salts: the process-wide salt and Argon2i.
    Legacy,
    /// An entry carrying a salt of its own, derived with Argon2id.
    PerEntry(Vec<u8>),
}

/// Mint a fresh salt for a new entry.
pub(crate) fn new_salt() -> Vec<u8> {
    rand::rng().random::<[u8; ENTRY_SALT_LEN]>().to_vec()
}

impl Derivation {
    fn config(&self) -> &'static argon2::Config<'static> {
        match self {
            Self::Legacy => &LEGACY_CONFIG,
            Self::PerEntry(_) => &CONFIG,
        }
    }

    fn salt(&self) -> &[u8] {
        match self {
            Self::Legacy => SALT.as_bytes(),
            Self::PerEntry(salt) => salt,
        }
    }
}

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

fn cipher_from(
    password: &[u8],
    salt: &[u8],
    config: &argon2::Config,
) -> Result<XChaCha20Poly1305, Error> {
    let key = argon2::hash_raw(password, salt, config)?;
    let key = Key::try_from(key.as_slice()).map_err(|_| Error::ChaCha20Poly1305Encrypt)?;
    Ok(XChaCha20Poly1305::new(&key))
}

impl Plaintext {
    /// Consume and encrypt plaintext into [`Encrypted`] using `password`.
    pub async fn encrypt(
        self,
        password: Password,
        derivation: Derivation,
    ) -> Result<Encrypted, Error> {
        spawn_blocking(move || {
            let cipher = cipher_from(&password.0, derivation.salt(), derivation.config())?;
            let nonce = XNonce::from(rand::rng().random::<[u8; 24]>());
            let ciphertext = cipher
                .encrypt(&nonce, self.0.as_ref())
                .map_err(|_| Error::ChaCha20Poly1305Encrypt)?;

            Ok(Encrypted::new(ciphertext, nonce))
        })
        .await?
    }
}

impl Encrypted {
    /// Create new [`Encrypted`] item from `ciphertext` and `nonce`.
    pub fn new(ciphertext: Vec<u8>, nonce: XNonce) -> Self {
        Self { ciphertext, nonce }
    }

    /// Decrypt into bytes using `password`.
    pub async fn decrypt(
        self,
        password: Password,
        derivation: Derivation,
    ) -> Result<Vec<u8>, Error> {
        spawn_blocking(move || {
            let cipher = cipher_from(&password.0, derivation.salt(), derivation.config())?;
            let nonce = XNonce::try_from(self.nonce.as_slice())
                .map_err(|_| Error::ChaCha20Poly1305Decrypt)?;
            let plaintext = cipher
                .decrypt(&nonce, self.ciphertext.as_ref())
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
            .encrypt(
                Password::from(password.as_bytes().to_vec()),
                Derivation::Legacy,
            )
            .await
            .unwrap();
        let decrypted = encrypted
            .decrypt(
                Password::from(password.as_bytes().to_vec()),
                Derivation::Legacy,
            )
            .await
            .unwrap();
        assert_eq!(decrypted, plaintext.as_bytes());
    }

    /// Captured from the scheme that shipped before per-entry salts: the default process-wide
    /// salt and Argon2i. Every paste encrypted by a running instance is sealed exactly like this,
    /// so the legacy derivation has to keep reproducing the same key byte for byte.
    #[test]
    fn a_paste_sealed_before_per_entry_salts_still_opens() {
        fn unhex(s: &str) -> Vec<u8> {
            (0..s.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
                .collect()
        }

        let ciphertext = unhex("099efe0be8d9b175835af41b5fe908b3ebc34efdebd11047299413bf40");
        let nonce = unhex("2514a266bf4d6ddf4ac3d02edb795b493a9756b43e81aabf");
        let nonce = XNonce::try_from(nonce.as_slice()).unwrap();

        // The salt is spelled out rather than read through `SALT`, which follows the environment.
        let cipher = cipher_from(b"hunter2", b"somesalt", Derivation::Legacy.config()).unwrap();
        let decrypted = cipher.decrypt(&nonce, ciphertext.as_ref()).unwrap();

        assert_eq!(decrypted, b"legacy secret");
    }

    /// The salt exists to make one derivation testable against one entry. Sharing it across every
    /// paste meant a single argon2 evaluation per candidate password could be checked against all
    /// of them at once, and two pastes with the same password shared a key.
    #[tokio::test]
    async fn two_pastes_with_one_password_do_not_share_a_key() {
        let first_salt = new_salt();
        let second_salt = new_salt();
        assert_ne!(first_salt, second_salt);

        let first = Plaintext::from(b"encrypt me".to_vec())
            .encrypt(
                Password::from(b"secret".to_vec()),
                Derivation::PerEntry(first_salt),
            )
            .await
            .unwrap();

        // The key derived for one entry must not open another, even with the right password.
        assert!(
            first
                .decrypt(
                    Password::from(b"secret".to_vec()),
                    Derivation::PerEntry(second_salt),
                )
                .await
                .is_err()
        );
    }
}
