use crate::id::Id;
use lru::LruCache;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex};

#[derive(PartialEq, Eq, Hash)]
pub struct Key {
    id: Id,
    ext: String,
}

pub struct Inner {
    cache: LruCache<Key, String>,
    cached: HashMap<Id, Vec<String>>,
}

pub type Cache = Arc<Mutex<Inner>>;

pub fn new(size: usize) -> Cache {
    Arc::new(Mutex::new(Inner::new(size)))
}

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
            cached.push(k.ext.clone())
        } else {
            self.cached.insert(k.id, vec![k.ext.clone()]);
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
