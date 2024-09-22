use crate::db::write::Entry;
use crate::errors::Error;
use core::str;
use std::fmt;
use std::str::FromStr;

const CHAR_TABLE: &[u8; 64] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-+";

pub type Inner = i64;
const ID_LENGTH: usize = 11;

/// Represents a 64-bit integer either numerically or mapped to a 11 character string.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Id {
    n: Inner,
}

impl Id {
    /// Return the value itself.
    pub fn as_inner(self) -> Inner {
        self.n
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
        #[allow(clippy::cast_sign_loss)]
        let buf: [u8; ID_LENGTH] = [
            CHAR_TABLE[((self.n >> 58) & 0x3f) as usize],
            CHAR_TABLE[((self.n >> 52) & 0x3f) as usize],
            CHAR_TABLE[((self.n >> 46) & 0x3f) as usize],
            CHAR_TABLE[((self.n >> 40) & 0x3f) as usize],
            CHAR_TABLE[((self.n >> 34) & 0x3f) as usize],
            CHAR_TABLE[((self.n >> 28) & 0x3f) as usize],
            CHAR_TABLE[((self.n >> 22) & 0x3f) as usize],
            CHAR_TABLE[((self.n >> 16) & 0x3f) as usize],
            CHAR_TABLE[((self.n >> 10) & 0x3f) as usize],
            CHAR_TABLE[((self.n >> 4) & 0x3f) as usize],
            CHAR_TABLE[(self.n & 0xf) as usize],
        ];

        let str = str::from_utf8(&buf).expect("characters are valid UTF-8");
        debug_assert!(str.len() == ID_LENGTH);

        write!(f, "{str}")
    }
}

impl FromStr for Id {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        /* Support ID generated in the old 32-bit format */
        if value.len() == 6 {
            let mut n: Inner = 0;

            for (pos, char) in value.as_bytes().iter().enumerate() {
                let bits: Option<Inner> = CHAR_TABLE.iter().enumerate().find_map(|(bits, c)| {
                    (char == c).then(|| bits.try_into().expect("bits not 64 bits"))
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

            return Ok(Self { n });
        }

        if value.len() != ID_LENGTH {
            return Err(Error::WrongSize);
        }

        let mut n: Inner = 0;

        for (pos, char) in value.as_bytes().iter().enumerate() {
            let bits: Option<Inner> = CHAR_TABLE.iter().enumerate().find_map(|(bits, c)| {
                (char == c).then(|| bits.try_into().expect("bits not 64 bits"))
            });

            match bits {
                None => return Err(Error::IllegalCharacters),
                Some(bits) => {
                    if pos < ID_LENGTH - 1 {
                        n = (n << 6) | bits;
                    } else {
                        n = (n << 4) | bits;
                    }
                }
            }
        }

        Ok(Self { n })
    }
}

impl From<Inner> for Id {
    fn from(n: Inner) -> Self {
        Self { n }
    }
}

#[cfg(test)]
mod tests {
    use std::i64;

    use super::*;

    #[test]
    fn convert_inner_to_id_and_back() {
        let id = Id::from(0);
        assert_eq!(id.to_string(), "aaaaaaaaaaa");
        assert_eq!(id.as_inner(), 0);
        assert_eq!(Id::from_str(&id.to_string()).unwrap(), id);

        let id = Id::from(-1);
        assert_eq!(id.to_string(), "++++++++++p");
        assert_eq!(id.as_inner(), -1);
        assert_eq!(Id::from_str(&id.to_string()).unwrap(), id);

        let id = Id::from(0xffffffff);
        assert_eq!(id.to_string(), "aaaaap++++p");
        assert_eq!(id.as_inner(), 0xffffffff);
        assert_eq!(Id::from_str(&id.to_string()).unwrap(), id);

        let id = Id::from(i64::MAX);
        assert_eq!(id.to_string(), "F+++++++++p");
        assert_eq!(id.as_inner(), i64::MAX);
        assert_eq!(Id::from_str(&id.to_string()).unwrap(), id);

        let id = Id::from(i64::MIN);
        assert_eq!(id.to_string(), "Gaaaaaaaaaa");
        assert_eq!(id.as_inner(), i64::MIN);
        assert_eq!(Id::from_str(&id.to_string()).unwrap(), id);
    }

    #[test]
    fn convert_id_from_string() {
        /* Support ID generated in the old 32-bit format */
        //assert!(Id::from_str("abDE+-").is_ok());
        assert!(Id::from_str("#bDE+-").is_err());
        assert!(Id::from_str("abDE+-1").is_err());
        assert!(Id::from_str("abDE+").is_err());

        /* New 64-bit format */
        assert_eq!(
            Id::from_str("abDE+-12345").unwrap(),
            Id::from(6578377758007225)
        );
        assert!(Id::from_str("#bDE+-12345").is_err());
        assert!(Id::from_str("abDE+-123456").is_err());
        assert!(Id::from_str("abDE+12345").is_err());
    }
}
