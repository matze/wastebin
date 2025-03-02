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

/// Represents a 32-bit integer either numerically or mapped to a 6 character string.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Id {
    n: u32,
}

impl Id {
    /// Generate a new random [`Id`]. According to the [`rand::rng()`] documentation this should be
    /// fast and not require additional an `spawn_blocking()` call.
    pub fn new() -> Self {
        let n = rand::rng().random::<u32>();
        Self { n }
    }

    /// Return i64 representation for database storage purposes.
    pub fn to_i64(self) -> i64 {
        self.n.into()
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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut s = String::with_capacity(6);

        s.push(CHAR_TABLE[((self.n >> 26) & 0x3f) as usize]);
        s.push(CHAR_TABLE[((self.n >> 20) & 0x3f) as usize]);
        s.push(CHAR_TABLE[((self.n >> 14) & 0x3f) as usize]);
        s.push(CHAR_TABLE[((self.n >> 8) & 0x3f) as usize]);
        s.push(CHAR_TABLE[((self.n >> 2) & 0x3f) as usize]);
        s.push(CHAR_TABLE[(self.n & 0x3) as usize]);

        write!(f, "{s}")
    }
}

impl FromStr for Id {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != 6 {
            return Err(Error::WrongSize);
        }

        let mut n: u32 = 0;

        for (pos, char) in value.chars().enumerate() {
            let bits: Option<u32> = CHAR_TABLE.iter().enumerate().find_map(|(bits, c)| {
                (char == *c).then(|| bits.try_into().expect("bits not 32 bits"))
            });

            match bits {
                None => return Err(Error::IllegalCharacters),
                Some(bits) => {
                    if pos < 5 {
                        n = (n << 6) | bits;
                    } else {
                        n = (n << 2) | bits;
                    }
                }
            }
        }

        Ok(Self { n })
    }
}

impl From<u32> for Id {
    fn from(n: u32) -> Self {
        Self { n }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_i64_to_id_and_back() {
        let id = Id::from(0);
        assert_eq!(id.to_string(), "aaaaaa");
        assert_eq!(id.to_i64(), 0);

        let id = Id::from(0xffffffff);
        assert_eq!(id.to_string(), "+++++d");
        assert_eq!(id.to_i64(), 0xffffffff);
    }

    #[test]
    fn convert_string_to_id_and_back() {
        let id = Id::from_str("bJZCna").unwrap();
        assert_eq!(id, Id { n: 104651828 });
        assert_eq!(id.to_string(), "bJZCna");
    }

    #[test]
    fn conversion_failures() {
        assert!(Id::from_str("abDE+-").is_ok());
        assert!(matches!(
            Id::from_str("#bDE+-"),
            Err(Error::IllegalCharacters)
        ));
        assert!(matches!(Id::from_str("abDE+-1"), Err(Error::WrongSize)));
        assert!(matches!(Id::from_str("abDE+"), Err(Error::WrongSize)));
    }
}
