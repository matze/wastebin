use crate::db::Database;
use crate::highlight::DATA;
use crate::id::Id;
use crate::{Entry, Error};
use lru::LruCache;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::sync::{Arc, Mutex};

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Key {
    id: Id,
    ext: String,
}

pub struct Inner {
    cache: LruCache<Key, String>,
    cached: HashMap<Id, HashSet<String>>,
}

type Cache = Arc<Mutex<Inner>>;

impl Key {
    pub fn new(id: Id, ext: String) -> Key {
        Self { id, ext }
    }
}

impl Inner {
    pub fn new(size: usize) -> Self {
        let cache = lru::LruCache::new(size);

        Self {
            cache,
            cached: HashMap::new(),
        }
    }

    pub fn get<'a>(&'a mut self, k: &Key) -> Option<&'a String> {
        self.cache.get(k)
    }

    pub fn put(&mut self, k: Key, v: String) -> Option<String> {
        if let Some(cached) = self.cached.get_mut(&k.id) {
            if !cached.contains(&k.ext) {
                cached.insert(k.ext.clone());
            }
        } else {
            let mut set = HashSet::new();
            set.insert(k.ext.clone());
            self.cached.insert(k.id, set);
        }

        self.cache.put(k, v)
    }

    pub fn remove(&mut self, id: Id) {
        if let Some(exts) = self.cached.remove(&id) {
            for ext in exts {
                tracing::debug!("evicting {id:?}.{ext}");
                self.cache.pop(&Key::new(id, ext));
            }
        }
    }
}

/// Cache layer combining database and cache access.
#[derive(Clone)]
pub struct Layer {
    db: Database,
    cache: Cache,
}

impl Layer {
    pub fn new(db: Database, cache_size: usize) -> Self {
        let cache = Arc::new(Mutex::new(Inner::new(cache_size)));
        Self { db, cache }
    }

    /// Insert `entry` into the database.
    pub async fn insert(&self, id: Id, entry: Entry) -> Result<(), Error> {
        self.db.insert(id, entry).await
    }

    /// Look up or generate HTML formatted data. Return `None` if `key` is not found.
    pub async fn get_formatted(&self, key: Key) -> Result<String, Error> {
        if let Some(cached) = self.cache.lock().unwrap().get(&key) {
            tracing::debug!(?key, "found cached item");
            return Ok(cached.to_string());
        }

        let entry = self.db.get(key.id).await?;
        let burn_after_reading = entry.burn_after_reading.unwrap_or(false);
        let ext = key.ext.clone();
        let formatted = tokio::task::spawn_blocking(move || DATA.highlight(&entry, &ext)).await??;

        if !burn_after_reading {
            tracing::debug!(?key, "cache item");
            self.cache.lock().unwrap().put(key, formatted.clone());
        }

        Ok(formatted)
    }

    /// Get raw content for `id` or `None` if not found.
    pub async fn get_raw(&self, id: Id) -> Result<String, Error> {
        self.db.get(id).await.map(|e| e.text)
    }

    /// Purge expired items from database and cache.
    pub async fn purge(&self) -> Result<(), Error> {
        for id in self.db.purge().await? {
            tracing::debug!(?id, "remove from cache");
            self.cache.lock().unwrap().remove(id);
        }
        Ok(())
    }
}

/// Purge `layer` every minute.
pub async fn purge_periodically(layer: Layer) -> Result<(), Error> {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));

    loop {
        interval.tick().await;
        layer.purge().await?;
    }
}
