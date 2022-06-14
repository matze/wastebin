use crate::id::Id;
use lru::LruCache;
use std::sync::{Arc, Mutex};

#[derive(PartialEq, Eq, Hash)]
pub struct Key {
    id: Id,
    ext: String,
}

pub type Cache = Arc<Mutex<LruCache<Key, String>>>;

pub fn new(size: usize) -> Cache {
    Arc::new(Mutex::new(lru::LruCache::new(size)))
}

impl Key {
    pub fn new(id: Id, ext: String) -> Key {
        Self { id, ext }
    }
}
