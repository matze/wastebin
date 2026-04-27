use rand::RngExt;

use std::fmt;
use std::str::FromStr;

use crate::db::write::Entry;

const CHAR_TABLE: &[char; 64] = &[
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's',
    't', 'u', 'v', 'w', 'x', 'y', 'z', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L',
    'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', '0', '1', '2', '3', '4',
    '5', '6', '7', '8', '9', '-', '+',
];

include!(concat!(env!("OUT_DIR"), "/wordlists.rs"));

/// Word-URL slot layout, MSB → LSB. Each entry is `(bit_width, word_list)`.
/// Widths must sum to 64 — verified by [`tests::slot_widths_sum_to_64`].
static SLOTS: [(u32, &[&str]); 7] = [
    (6, DETERMINERS),
    (11, ADJECTIVES),
    (10, NOUNS),
    (10, VERBS),
    (6, DETERMINERS),
    (11, ADJECTIVES),
    (10, NOUNS),
];

/// URL encoding scheme for paste IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UrlScheme {
    /// Existing 6/11-character base64-style IDs.
    Compact,
    /// Seven dash-separated words: `det-adj-noun-verb-det-adj-noun`.
    Words,
}

impl FromStr for UrlScheme {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "compact" => Ok(Self::Compact),
            "words" => Ok(Self::Words),
            _ => Err(Error::UnknownScheme),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("illegal characters")]
    IllegalCharacters,
    #[error("wrong size")]
    WrongSize,
    #[error("unknown word")]
    UnknownWord,
    #[error("unknown URL scheme")]
    UnknownScheme,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Id {
    /// Six-character identifiers.
    Id32(u32),
    /// Eleven-character identifiers.
    Id64(i64),
}

impl Id {
    /// Generate a new random [`Id`]. According to the [`rand::rng()`] documentation this should be
    /// fast and not require additional an `spawn_blocking()` call.
    #[must_use]
    pub fn rand() -> Self {
        Self::Id64(rand::rng().random::<i64>())
    }

    /// Return i64 representation for database storage purposes.
    #[must_use]
    pub fn to_i64(self) -> i64 {
        match self {
            Self::Id32(n) => n.into(),
            Self::Id64(n) => n,
        }
    }

    /// Build a URL path segment (`<encoded-id>` or `<encoded-id>.<ext>`) for
    /// this id under `scheme`.
    #[must_use]
    pub fn to_url_path(self, entry: &Entry, scheme: UrlScheme) -> String {
        let encoded = EncodedId::from_id(self, scheme);
        match entry.extension.as_ref() {
            Some(ext) => format!("{encoded}.{ext}"),
            None => encoded.into_string(),
        }
    }

    /// Encode this id in the compact 6/11-character base64-style format.
    #[must_use]
    #[expect(clippy::cast_sign_loss)]
    pub fn to_compact_string(self) -> String {
        match self {
            Self::Id32(n) => {
                let mut s = String::with_capacity(6);

                s.push(CHAR_TABLE[((n >> 26) & 0x3f) as usize]);
                s.push(CHAR_TABLE[((n >> 20) & 0x3f) as usize]);
                s.push(CHAR_TABLE[((n >> 14) & 0x3f) as usize]);
                s.push(CHAR_TABLE[((n >> 8) & 0x3f) as usize]);
                s.push(CHAR_TABLE[((n >> 2) & 0x3f) as usize]);
                s.push(CHAR_TABLE[(n & 0x3) as usize]);

                s
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

                s
            }
        }
    }

    /// Encode this id as 7 dash-separated words following the pattern
    /// `det-adjective-noun-verb-det-adjective-noun`. Bit-casts `i64` to
    /// `u64` so shifts operate on raw bits. See [`SLOTS`] for the layout.
    #[must_use]
    #[expect(clippy::cast_sign_loss)]
    pub fn to_words_string(self) -> String {
        let n = self.to_i64() as u64;
        let mut s = String::with_capacity(64);
        let mut shift: u32 = u64::BITS;

        for (i, &(width, list)) in SLOTS.iter().enumerate() {
            if i > 0 {
                s.push('-');
            }
            shift -= width;
            let mask = (1u64 << width) - 1;
            s.push_str(list[((n >> shift) & mask) as usize]);
        }

        s
    }

    pub fn from_compact(value: &str) -> Result<Self, Error> {
        if value.len() == 6 {
            let mut n: u32 = 0;

            for (pos, char) in value.chars().enumerate() {
                #[expect(clippy::cast_possible_truncation)]
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
                #[expect(clippy::cast_possible_wrap)]
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
            Err(Error::WrongSize)
        }
    }

    pub fn from_words(value: &str) -> Result<Self, Error> {
        let parts: Vec<&str> = value.split('-').collect();
        if parts.len() != SLOTS.len() {
            return Err(Error::WrongSize);
        }

        let mut n: u64 = 0;
        for (part, &(width, list)) in parts.iter().zip(SLOTS.iter()) {
            let idx = list.binary_search(part).map_err(|_| Error::UnknownWord)?;
            n = (n << width) | (idx as u64);
        }

        #[expect(clippy::cast_possible_wrap)]
        Ok(Self::Id64(n as i64))
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

/// A paste [`Id`] encoded as a compact 6/11-char URL segment (e.g. `eVI4Z48hybf`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CompactId(String);

impl CompactId {
    /// Encode `id` in compact form.
    #[must_use]
    pub fn from_id(id: Id) -> Self {
        Self(id.to_compact_string())
    }

    /// Validate `value` as a compact-encoded id and return both the wrapper
    /// and the decoded numeric [`Id`].
    pub fn parse(value: &str) -> Result<(Self, Id), Error> {
        let id = Id::from_compact(value)?;
        Ok((Self(value.to_string()), id))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for CompactId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A paste [`Id`] encoded as 7 dash-separated words (e.g. `the-quiet-cloud-…`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WordsId(String);

impl WordsId {
    /// Encode `id` in words form.
    #[must_use]
    pub fn from_id(id: Id) -> Self {
        Self(id.to_words_string())
    }

    /// Validate `value` as a words-encoded id and return both the wrapper and
    /// the decoded numeric [`Id`].
    pub fn parse(value: &str) -> Result<(Self, Id), Error> {
        let id = Id::from_words(value)?;
        Ok((Self(value.to_string()), id))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for WordsId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// An encoded paste id, dispatched on a [`UrlScheme`].
///
/// Use this when the active scheme is determined at runtime (e.g. carried in
/// `AppState`). For statically-known schemes prefer [`CompactId`] or
/// [`WordsId`] directly.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum EncodedId {
    Compact(CompactId),
    Words(WordsId),
}

impl EncodedId {
    /// Encode `id` under `scheme`.
    #[must_use]
    pub fn from_id(id: Id, scheme: UrlScheme) -> Self {
        match scheme {
            UrlScheme::Compact => Self::Compact(CompactId::from_id(id)),
            UrlScheme::Words => Self::Words(WordsId::from_id(id)),
        }
    }

    /// Validate `value` under `scheme` and return both the wrapper and the
    /// decoded numeric [`Id`].
    pub fn parse(value: &str, scheme: UrlScheme) -> Result<(Self, Id), Error> {
        match scheme {
            UrlScheme::Compact => {
                let (compact, id) = CompactId::parse(value)?;
                Ok((Self::Compact(compact), id))
            }
            UrlScheme::Words => {
                let (words, id) = WordsId::parse(value)?;
                Ok((Self::Words(words), id))
            }
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Compact(c) => c.as_str(),
            Self::Words(w) => w.as_str(),
        }
    }

    #[must_use]
    pub fn into_string(self) -> String {
        match self {
            Self::Compact(c) => c.into_string(),
            Self::Words(w) => w.into_string(),
        }
    }
}

impl fmt::Display for EncodedId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_i64_to_id_and_back() {
        let id = Id::from(0u32);
        assert_eq!(id.to_compact_string(), "aaaaaa");
        assert_eq!(id.to_i64(), 0);

        let id = Id::from(0i64);
        assert_eq!(id.to_compact_string(), "aaaaaaaaaaa");
        assert_eq!(id.to_i64(), 0);

        let id = Id::from(0xffff_ffff_u32);
        assert_eq!(id.to_compact_string(), "+++++d");
        assert_eq!(id.to_i64(), 0xffff_ffff);

        let id = Id::from(0xfff_ffff_ffff_ffff_i64);
        assert_eq!(id.to_compact_string(), "d+++++++++p");
        assert_eq!(id.to_i64(), 0xfff_ffff_ffff_ffff);
    }

    #[test]
    fn convert_string_to_id_and_back() {
        let id = Id::from_compact("bJZCna").unwrap();
        assert_eq!(id.to_i64(), 104_651_828);
        assert_eq!(id.to_compact_string(), "bJZCna");

        let id = Id::from_compact("eVI4Z48hybf").unwrap();
        assert_eq!(id.to_i64(), 1_367_045_688_504_311_829);
        assert_eq!(id.to_compact_string(), "eVI4Z48hybf");
    }

    #[test]
    fn conversion_failures() {
        assert!(Id::from_compact("abDE+-").is_ok());
        assert!(Id::from_compact("abDE+-12345").is_ok());
        assert!(matches!(
            Id::from_compact("#bDE+-"),
            Err(Error::IllegalCharacters)
        ));
        assert!(matches!(Id::from_compact("abDE+-1"), Err(Error::WrongSize)));
        assert!(matches!(Id::from_compact("abDE+"), Err(Error::WrongSize)));
    }

    #[test]
    fn slot_widths_sum_to_64() {
        let total: u32 = SLOTS.iter().map(|(w, _)| w).sum();
        assert_eq!(total, u64::BITS);
    }

    #[test]
    fn wordlist_invariants() {
        assert_eq!(
            DETERMINERS.len(),
            64,
            "determiners must have 64 words"
        );
        assert_eq!(
            ADJECTIVES.len(),
            2048,
            "adjectives must have 2048 words"
        );
        assert_eq!(NOUNS.len(), 1024, "nouns must have 1024 words");
        assert_eq!(VERBS.len(), 1024, "verbs must have 1024 words");

        for (name, list) in &[
            ("determiners", DETERMINERS),
            ("adjectives", ADJECTIVES),
            ("nouns", NOUNS),
            ("verbs", VERBS),
        ] {
            for w in *list {
                assert!(
                    (1..=12).contains(&w.len())
                        && w.chars().all(|c| c.is_ascii_lowercase()),
                    "{name}: invalid word {w:?} (must be 1-12 ascii lowercase)"
                );
            }
            for pair in list.windows(2) {
                assert!(
                    pair[0] < pair[1],
                    "{name}: not sorted at {pair:?}"
                );
            }
        }
    }

    #[test]
    fn words_round_trip_basic() {
        let cases: &[i64] = &[
            0,
            1,
            -1,
            i64::MIN,
            i64::MAX,
            0xdead_beef,
            0x0123_4567_89ab_cdef,
            -0x0123_4567_89ab_cdef,
            104_651_828,
            1_367_045_688_504_311_829,
        ];
        for &n in cases {
            let id = Id::Id64(n);
            let s = id.to_words_string();
            let decoded = Id::from_words(&s);
            assert!(decoded.is_ok(), "decode failed for {n}: {decoded:?}");
            assert_eq!(
                decoded.unwrap().to_i64(),
                n,
                "round-trip failed for {n}, got {s}"
            );
        }
    }

    #[test]
    fn words_format_shape() {
        let s = Id::Id64(0).to_words_string();
        let parts: Vec<&str> = s.split('-').collect();
        assert_eq!(parts.len(), 7, "expected 7 words, got {}: {s}", parts.len());

        // First and fifth words should be determiners
        assert!(DETERMINERS.binary_search(&parts[0]).is_ok());
        assert!(DETERMINERS.binary_search(&parts[4]).is_ok());
        // Second and sixth words should be adjectives
        assert!(ADJECTIVES.binary_search(&parts[1]).is_ok());
        assert!(ADJECTIVES.binary_search(&parts[5]).is_ok());
        // Third and seventh words should be nouns
        assert!(NOUNS.binary_search(&parts[2]).is_ok());
        assert!(NOUNS.binary_search(&parts[6]).is_ok());
        // Fourth word should be a verb
        assert!(VERBS.binary_search(&parts[3]).is_ok());
    }

    #[test]
    fn words_unknown_word_rejected() {
        assert!(matches!(
            Id::from_words("notaword-abdominal-abdomen-absorbs-the-abdominal-abdomen"),
            Err(Error::UnknownWord)
        ));
    }

    #[test]
    fn words_wrong_count_rejected() {
        let det = DETERMINERS[0];
        let adj = ADJECTIVES[0];
        let noun = NOUNS[0];
        let verb = VERBS[0];

        let too_few = [det, adj, noun, verb, det, adj].join("-");
        let too_many = [det, adj, noun, verb, det, adj, noun, det].join("-");
        assert!(matches!(Id::from_words(&too_few), Err(Error::WrongSize)));
        assert!(matches!(Id::from_words(&too_many), Err(Error::WrongSize)));
    }

    #[test]
    fn words_wrong_category_rejected() {
        // A determiner in the adjective position should be rejected
        let det = DETERMINERS[0];
        let adj = ADJECTIVES[0];
        let noun = NOUNS[0];
        let verb = VERBS[0];
        let s = format!("{det}-{det}-{noun}-{verb}-{det}-{adj}-{noun}");
        assert!(matches!(Id::from_words(&s), Err(Error::UnknownWord)));
    }

    #[test]
    fn url_scheme_parse() {
        assert_eq!("compact".parse::<UrlScheme>().unwrap(), UrlScheme::Compact);
        assert_eq!("words".parse::<UrlScheme>().unwrap(), UrlScheme::Words);
        assert!("hex".parse::<UrlScheme>().is_err());
    }

    #[test]
    fn encoded_id_round_trip() {
        let id = Id::from(1_367_045_688_504_311_829_i64);

        let compact = EncodedId::from_id(id, UrlScheme::Compact);
        assert_eq!(compact.as_str(), "eVI4Z48hybf");
        let (parsed_compact, parsed_id) =
            EncodedId::parse(compact.as_str(), UrlScheme::Compact).unwrap();
        assert_eq!(parsed_compact, compact);
        assert_eq!(parsed_id, id);

        let words = EncodedId::from_id(id, UrlScheme::Words);
        let (parsed_words, parsed_id) =
            EncodedId::parse(words.as_str(), UrlScheme::Words).unwrap();
        assert_eq!(parsed_words, words);
        assert_eq!(parsed_id, id);
    }

    #[test]
    fn encoded_id_rejects_wrong_scheme() {
        // A compact-encoded string should not parse as words and vice versa.
        let id = Id::from(1_367_045_688_504_311_829_i64);
        let compact = EncodedId::from_id(id, UrlScheme::Compact);
        assert!(EncodedId::parse(compact.as_str(), UrlScheme::Words).is_err());

        let words = EncodedId::from_id(id, UrlScheme::Words);
        assert!(EncodedId::parse(words.as_str(), UrlScheme::Compact).is_err());
    }
}
