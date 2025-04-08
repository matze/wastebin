use crate::db::write::Entry;
use cgisf_lib::{SentenceConfigBuilder, gen_sentence};
use rand::Rng;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

const CHAR_TABLE: &[char; 64] = &[
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's',
    't', 'u', 'v', 'w', 'x', 'y', 'z', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L',
    'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', '0', '1', '2', '3', '4',
    '5', '6', '7', '8', '9', '-', '+',
];

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("illegal characters")]
    IllegalCharacters,
    #[error("wrong size")]
    WrongSize,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Id {
    /// Six-character identifiers.
    Id32(u32),
    /// Eleven-character identifiers.
    Id64(i64),
    /// Human-readable identifiers, at least twelve chracters.
    /// `Arc<String>` would be cheap to clone.
    HumanReadable(Arc<String>),
}

impl Id {
    /// Generate a new random [`Id`]. According to the [`rand::rng()`] documentation this should be
    /// fast and not require additional an `spawn_blocking()` call.
    #[must_use]
    pub fn rand(human_readable: Option<bool>) -> Self {
        if human_readable.is_some_and(|b| b) {
            Self::rand_human_readable()
        } else {
            Self::Id64(rand::rng().random::<i64>())
        }
    }

    /// Generate a new random human-readable [`Id`].
    #[must_use]
    fn rand_human_readable() -> Self {
        let gen_sentence = || {
            gen_sentence(
                SentenceConfigBuilder::random()
                    .plural(false)
                    .adjectives(1)
                    .adverbs(1)
                    .structure(cgisf_lib::Structure::AdjectivesNounVerbAdverbs)
                    .build(),
            )
            .replace("The ", "")
            .trim_end_matches(".")
            .replace(" ", "-")
        };

        let mut sentence = gen_sentence();
        while sentence.len() < 12 {
            sentence = gen_sentence();
        }

        Self::HumanReadable(Arc::new(sentence))
    }

    /// Return i64 representation for database storage purposes.
    #[must_use]
    #[expect(
        clippy::cast_possible_wrap,
        reason = "wrapping is acceptable in this case"
    )]
    pub fn to_i64(&self) -> i64 {
        match self {
            Self::Id32(n) => *n as _,
            Self::Id64(n) => *n,
            Self::HumanReadable(s) => {
                // must set a fixed seed
                let random_state = ahash::RandomState::with_seed(42);
                random_state.hash_one(s) as _
            }
        }
    }

    /// Generate a URL path from the string representation and `entry`'s extension.
    #[must_use]
    pub fn to_url_path(&self, entry: &Entry) -> String {
        entry
            .extension
            .as_ref()
            .map_or_else(|| format!("{self}"), |ext| format!("{self}.{ext}"))
    }
}

impl fmt::Display for Id {
    #[expect(clippy::cast_sign_loss)]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Id32(n) => {
                let mut s = String::with_capacity(6);

                s.push(CHAR_TABLE[((n >> 26) & 0x3f) as usize]);
                s.push(CHAR_TABLE[((n >> 20) & 0x3f) as usize]);
                s.push(CHAR_TABLE[((n >> 14) & 0x3f) as usize]);
                s.push(CHAR_TABLE[((n >> 8) & 0x3f) as usize]);
                s.push(CHAR_TABLE[((n >> 2) & 0x3f) as usize]);
                s.push(CHAR_TABLE[(n & 0x3) as usize]);

                write!(f, "{s}")
            }
            Self::Id64(n) => {
                let mut s = String::with_capacity(11);

                s.push(CHAR_TABLE[((n >> 58) & 0x3f) as usize]);
                s.push(CHAR_TABLE[((n >> 52) & 0x3f) as usize]);
                s.push(CHAR_TABLE[((n >> 46) & 0x3f) as usize]);
                s.push(CHAR_TABLE[((n >> 40) & 0x3f) as usize]);
                s.push(CHAR_TABLE[((n >> 34) & 0x3f) as usize]);
                s.push(CHAR_TABLE[((n >> 28) & 0x3f) as usize]);
                s.push(CHAR_TABLE[((n >> 22) & 0x3f) as usize]);
                s.push(CHAR_TABLE[((n >> 16) & 0x3f) as usize]);
                s.push(CHAR_TABLE[((n >> 10) & 0x3f) as usize]);
                s.push(CHAR_TABLE[((n >> 4) & 0x3f) as usize]);
                s.push(CHAR_TABLE[(n & 0xf) as usize]);

                write!(f, "{s}")
            }
            Self::HumanReadable(s) => {
                write!(f, "{s}")
            }
        }
    }
}

impl FromStr for Id {
    type Err = Error;

    #[expect(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.len() {
            6 => {
                let mut n: u32 = 0;

                for (pos, char) in value.chars().enumerate() {
                    let bits: u32 = CHAR_TABLE
                        .iter()
                        .enumerate()
                        .find_map(|(bits, c)| (char == *c).then_some(bits as u32))
                        .ok_or(Error::IllegalCharacters)?;

                    if pos < 5 {
                        n = (n << 6) | bits;
                    } else {
                        n = (n << 2) | bits;
                    }
                }

                Ok(Self::Id32(n))
            }
            11 => {
                let mut n: i64 = 0;

                for (pos, char) in value.chars().enumerate() {
                    let bits: i64 = CHAR_TABLE
                        .iter()
                        .enumerate()
                        .find_map(|(bits, c)| (char == *c).then_some(bits as i64))
                        .ok_or(Error::IllegalCharacters)?;

                    if pos < 10 {
                        n = (n << 6) | bits;
                    } else {
                        n = (n << 4) | bits;
                    }
                }

                Ok(Self::Id64(n))
            }
            _ => Self::try_from(value.to_string()),
        }
    }
}

impl TryFrom<String> for Id {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.len() < 12 {
            return Err(Error::WrongSize);
        }

        Ok(Self::HumanReadable(Arc::new(value)))
    }
}

impl From<u32> for Id {
    fn from(n: u32) -> Self {
        Self::Id32(n)
    }
}

impl From<i64> for Id {
    fn from(n: i64) -> Self {
        Self::Id64(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_i64_to_id_and_back() {
        let id = Id::from(0u32);
        assert_eq!(id.to_string(), "aaaaaa");
        assert_eq!(id.to_i64(), 0);

        let id = Id::from(0i64);
        assert_eq!(id.to_string(), "aaaaaaaaaaa");
        assert_eq!(id.to_i64(), 0);

        let id = Id::from(0xffffffffu32);
        assert_eq!(id.to_string(), "+++++d");
        assert_eq!(id.to_i64(), 0xffffffff);

        let id = Id::from(0xfffffffffffffffi64);
        assert_eq!(id.to_string(), "d+++++++++p");
        assert_eq!(id.to_i64(), 0xfffffffffffffff);
    }

    #[test]
    fn construct_human_readable_strings() {
        let id = Id::from_str("rust-empowers-programmers-deeply").unwrap();
        assert_eq!(id.to_i64(), -3379340587488716302);
        assert_eq!(id.to_string(), "rust-empowers-programmers-deeply");

        let id = Id::from_str("axum-provides-great-performance").unwrap();
        assert_eq!(id.to_i64(), -7777240774086603578);
        assert_eq!(id.to_string(), "axum-provides-great-performance");
    }

    #[test]
    fn convert_string_to_id_and_back() {
        let id = Id::from_str("bJZCna").unwrap();
        assert_eq!(id.to_i64(), 104651828);
        assert_eq!(id.to_string(), "bJZCna");

        let id = Id::from_str("eVI4Z48hybf").unwrap();
        assert_eq!(id.to_i64(), 1367045688504311829);
        assert_eq!(id.to_string(), "eVI4Z48hybf");
    }

    #[test]
    fn conversion_failures() {
        assert!(Id::from_str("abDE+-").is_ok());
        assert!(Id::from_str("abDE+-12345").is_ok());
        assert!(matches!(
            Id::from_str("#bDE+-"),
            Err(Error::IllegalCharacters)
        ));
        assert!(matches!(Id::from_str("abDE+-1"), Err(Error::WrongSize)));
        assert!(matches!(Id::from_str("abDE+"), Err(Error::WrongSize)));
    }
}
