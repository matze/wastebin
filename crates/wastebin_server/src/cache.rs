use std::fmt::{self, Display};
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use cached::{Cached, SizedCache};

use crate::errors::Error;

use wastebin_core::id::{EncodedId, Id, UrlScheme};
use wastebin_highlight::Html;

/// Cache based on identifier and format.
///
/// Carries the active [`UrlScheme`] so the [`Display`] impl renders the id and
/// extension into a URL fragment without consulting any global state.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Key {
    pub id: Id,
    pub ext: Option<String>,
    pub scheme: UrlScheme,
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
    inner: Arc<Mutex<SizedCache<Slot, Html>>>,
}

impl Cache {
    pub fn new(size: NonZeroUsize) -> Self {
        let inner = Arc::new(Mutex::new(SizedCache::with_size(size.into())));

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
    /// Render the id (without extension) under the active URL scheme.
    pub fn id(&self) -> String {
        EncodedId::from_id(self.id, self.scheme).into_string()
    }

    /// Parse `value` (`<id>` or `<id>.<ext>`) under `scheme`.
    pub fn parse(value: &str, scheme: UrlScheme) -> Result<Self, Error> {
        let (id, ext) = match value.split_once('.') {
            None => {
                let (_, id) = EncodedId::parse(value, scheme).map_err(Error::Id)?;
                (id, None)
            }
            Some((id, ext)) => {
                let (_, id) = EncodedId::parse(id, scheme).map_err(Error::Id)?;
                (id, Some(ext.to_string()))
            }
        };

        Ok(Self { id, ext, scheme })
    }
}

impl Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let encoded = EncodedId::from_id(self.id, self.scheme);
        if let Some(ext) = &self.ext {
            write!(f, "{encoded}.{ext}")
        } else {
            fmt::Display::fmt(&encoded, f)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key() {
        let key = Key::parse("bJZCna", UrlScheme::Compact).unwrap();
        assert_eq!(key.id(), "bJZCna");
        assert_eq!(key.id, Id::from(104_651_828_u32));
        assert_eq!(key.ext, None);

        let key = Key::parse("sIiFec.rs", UrlScheme::Compact).unwrap();
        assert_eq!(key.id(), "sIiFec");
        assert_eq!(key.id, 1_243_750_162_u32.into());
        assert_eq!(key.ext.unwrap(), "rs");

        assert!(Key::parse("foo", UrlScheme::Compact).is_err());
        assert!(Key::parse("bar.rs", UrlScheme::Compact).is_err());
    }
}
