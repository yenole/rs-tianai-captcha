use dashmap::DashMap;
use serde_json::Value;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct MemoryCache {
    entries: Arc<DashMap<String, (Instant, Value)>>,
}
impl Default for MemoryCache {
    fn default() -> Self {
        Self {
            entries: Arc::new(DashMap::new()),
        }
    }
}
impl MemoryCache {
    pub fn put(&self, key: String, value: Value, ttl: Duration) {
        self.entries.insert(key, (Instant::now() + ttl, value));
    }
    /// Atomically consumes a non-expired entry, matching Java getAndRemoveCache.
    pub fn take(&self, key: &str) -> Option<Value> {
        let (_, (expires, value)) = self.entries.remove(key)?;
        (expires > Instant::now()).then_some(value)
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
