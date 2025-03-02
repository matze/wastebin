use crate::db::write::Entry;
use crate::errors::Error;
use rand::Rng;
use std::fmt;
use std::str::FromStr;

const CHAR_TABLE: &[char; 64] = &[
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's',
    't', 'u', 'v', 'w', 'x', 'y', 'z', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L',
    'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', '0', '1', '2', '3', '4',
    '5', '6', '7', '8', '9', '-', '+',
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Id {
    /// Six-character identifiers.
    Id32(u32),
    /// Eleven-character identifiers.
    Id64(i64),
}

impl Id {
    /// Generate a new random [`Id`]. According to the [`rand::rng()`] documentation this should be
    /// fast and not require additional an `spawn_blocking()` call.
    pub fn new() -> Self {
        Self::Id64(rand::rng().random::<i64>())
    }

    /// Return i64 representation for database storage purposes.
    pub fn to_i64(self) -> i64 {
        match self {
            Self::Id32(n) => n.into(),
            Self::Id64(n) => n,
        }
    }

    /// Generate a URL path from the string representation and `entry`'s extension.
    pub fn to_url_path(self, entry: &Entry) -> String {
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
        }
    }
}

impl FromStr for Id {
    type Err = Error;

    #[expect(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() == 6 {
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
        } else if value.len() == 11 {
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
        } else {
            return Err(Error::WrongSize);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
