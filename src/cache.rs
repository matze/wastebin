use crate::errors::Error;
use crate::highlight::Html;
use crate::id::Id;
use std::num::NonZeroUsize;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

/// Cache based on identifier and format.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub id: Id,
    pub ext: String,
}

/// Stores formatted HTML.
#[derive(Clone)]
pub struct Cache {
    inner: Arc<Mutex<lru::LruCache<CacheKey, Html>>>,
}

impl Cache {
    pub fn new(size: NonZeroUsize) -> Self {
        let inner = Arc::new(Mutex::new(lru::LruCache::new(size)));

        Self { inner }
    }

    pub fn put(&self, key: CacheKey, value: Html) {
        self.inner.lock().expect("getting lock").put(key, value);
    }

    pub fn get(&self, key: &CacheKey) -> Option<Html> {
        self.inner.lock().expect("getting lock").get(key).cloned()
    }
}

impl CacheKey {
    /// Make a copy of the owned id.
    pub fn id(&self) -> String {
        self.id.to_string()
    }
}

impl FromStr for CacheKey {
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
        let key = CacheKey::from_str("bJZCna").unwrap();
        assert_eq!(key.id(), "bJZCna");
        assert_eq!(key.id, 104651828.into());
        assert_eq!(key.ext, "txt");

        let key = CacheKey::from_str("sIiFec.rs").unwrap();
        assert_eq!(key.id(), "sIiFec");
        assert_eq!(key.id, 1243750162.into());
        assert_eq!(key.ext, "rs");

        assert!(CacheKey::from_str("foo").is_err());
        assert!(CacheKey::from_str("bar.rs").is_err());
    }
}
