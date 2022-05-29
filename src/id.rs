use crate::Error;
use std::convert::{From, TryFrom};

pub struct Id {
    s: String,
    n: u32,
}

impl Id {
    pub fn as_str(&self) -> &str {
        &self.s
    }

    pub fn as_u32(&self) -> u32 {
        self.n
    }
}

static CHAR_TABLE: &'static [char; 64] = &[
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's',
    't', 'u', 'v', 'w', 'x', 'y', 'z', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L',
    'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', '0', '1', '2', '3', '4',
    '5', '6', '7', '8', '9', '-', '+',
];

impl TryFrom<&str> for Id {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value.len() != 6 {
            return Err(Error::WrongSize);
        }

        let mut n: u32 = 0;

        for (pos, char) in value.chars().enumerate() {
            let bits = CHAR_TABLE.iter().enumerate().find_map(|(bits, c)| {
                if char == *c {
                    Some(bits as u32)
                } else {
                    None
                }
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

        Ok(Self {
            s: value.to_string(),
            n,
        })
    }
}

impl From<u32> for Id {
    fn from(n: u32) -> Self {
        let mut s = String::with_capacity(6);

        s.push(CHAR_TABLE[(((n >> 26) & 0x3f) as usize)]);
        s.push(CHAR_TABLE[(((n >> 20) & 0x3f) as usize)]);
        s.push(CHAR_TABLE[(((n >> 14) & 0x3f) as usize)]);
        s.push(CHAR_TABLE[(((n >> 8) & 0x3f) as usize)]);
        s.push(CHAR_TABLE[(((n >> 2) & 0x3f) as usize)]);
        s.push(CHAR_TABLE[((n & 0x3) as usize)]);

        Self { s, n }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_u32_to_id_and_back() {
        let id = Id::from(0);
        assert_eq!(id.as_str(), "aaaaaa");
        assert_eq!(id.as_u32(), 0);

        let id = Id::from(0xffffffff);
        assert_eq!(id.as_str(), "+++++d");
        assert_eq!(id.as_u32(), 0xffffffff);
    }

    #[test]
    fn convert_id_from_string() {
        assert!(Id::try_from("abDE+-").is_ok());
        assert!(Id::try_from("#bDE+-").is_err());
        assert!(Id::try_from("abDE+-1").is_err());
        assert!(Id::try_from("abDE+").is_err());
    }
}
