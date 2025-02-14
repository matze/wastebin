use crate::errors::Error;
use crate::highlight::Html;
use crate::id::Id;
use cached::{Cached, SizedCache};
use std::fmt::Display;
use std::num::NonZeroUsize;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

/// Cache based on identifier and format.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    pub id: Id,
    pub ext: String,
}

/// Stores formatted HTML.
#[derive(Clone)]
pub struct Cache {
    inner: Arc<Mutex<SizedCache<Key, Html>>>,
}

impl Cache {
    pub fn new(size: NonZeroUsize) -> Self {
        let inner = Arc::new(Mutex::new(SizedCache::with_size(size.into())));

        Self { inner }
    }

    pub fn put(&self, key: Key, value: Html) {
        self.inner
            .lock()
            .expect("getting lock")
            .cache_set(key, value);
    }

    pub fn get(&self, key: &Key) -> Option<Html> {
        self.inner
            .lock()
            .expect("getting lock")
            .cache_get(key)
            .cloned()
    }
}

impl Key {
    /// Make a copy of the owned id.
    pub fn id(&self) -> String {
        self.id.to_string()
    }
}

impl Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.ext.is_empty() {
            write!(f, "{}", self.id)
        } else {
            write!(f, "{}.{}", self.id, self.ext)
        }
    }
}

impl FromStr for Key {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (id, ext) = match value.split_once('.') {
            None => (value.parse()?, "txt".to_string()),
            Some((id, ext)) => (id.parse()?, ext.to_string()),
        };

        Ok(Self { id, ext })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key() {
        let key = Key::from_str("bJZCna").unwrap();
        assert_eq!(key.id(), "bJZCna");
        assert_eq!(key.id, 104651828.into());
        assert_eq!(key.ext, "txt");

        let key = Key::from_str("sIiFec.rs").unwrap();
        assert_eq!(key.id(), "sIiFec");
        assert_eq!(key.id, 1243750162.into());
        assert_eq!(key.ext, "rs");

        assert!(Key::from_str("foo").is_err());
        assert!(Key::from_str("bar.rs").is_err());
    }
}
