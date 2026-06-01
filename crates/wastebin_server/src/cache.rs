use std::fmt::Display;
use std::num::NonZeroUsize;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use cached::{Cached, LruCache};

use crate::errors::Error;

use wastebin_core::id::Id;
use wastebin_highlight::Html;

/// Cache based on identifier and format.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Key {
    pub id: Id,
    pub ext: Option<String>,
}

/// Which representation of a paste a cached entry holds.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Mode {
    /// Syntax-highlighted source view.
    Source,
    /// Markdown rendered to HTML.
    Rendered,
}

/// Internal cache slot partitioning cached HTML by paste identity and render mode.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Slot {
    id: Id,
    ext: Option<String>,
    mode: Mode,
}

impl Slot {
    fn new(key: &Key, mode: Mode) -> Self {
        Self {
            id: key.id,
            ext: key.ext.clone(),
            mode,
        }
    }
}

/// Stores formatted HTML.
#[derive(Clone)]
pub(crate) struct Cache {
    inner: Arc<Mutex<LruCache<Slot, Html>>>,
}

impl Cache {
    pub fn new(size: NonZeroUsize) -> Self {
        let inner = Arc::new(Mutex::new(LruCache::with_size(size.into())));

        Self { inner }
    }

    pub fn put(&self, key: &Key, mode: Mode, value: Html) {
        self.inner
            .lock()
            .expect("getting lock")
            .cache_set(Slot::new(key, mode), value);
    }

    pub fn get(&self, key: &Key, mode: Mode) -> Option<Html> {
        self.inner
            .lock()
            .expect("getting lock")
            .cache_get(&Slot::new(key, mode))
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
        if let Some(ext) = &self.ext {
            write!(f, "{}.{}", self.id, ext)
        } else {
            write!(f, "{}", self.id)
        }
    }
}

impl FromStr for Key {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (id, ext) = match value.split_once('.') {
            None => (value.parse()?, None),
            Some((id, ext)) => (id.parse().map_err(Error::Id)?, Some(ext.to_string())),
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
        assert_eq!(key.id, Id::from(104_651_828_u32));
        assert_eq!(key.ext, None);

        let key = Key::from_str("sIiFec.rs").unwrap();
        assert_eq!(key.id(), "sIiFec");
        assert_eq!(key.id, 1_243_750_162_u32.into());
        assert_eq!(key.ext.unwrap(), "rs");

        assert!(Key::from_str("foo").is_err());
        assert!(Key::from_str("bar.rs").is_err());
    }
}
