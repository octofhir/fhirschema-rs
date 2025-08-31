use crate::error::Result;
use crate::storage::SchemaStorage;
use crate::types::FhirSchema;
use lru::LruCache;
use papaya::HashMap as PapayaMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use url::Url;

#[derive(Debug)]
pub struct CacheEntry {
    pub schema: Arc<FhirSchema>,
    pub version: u64,
    pub created_at: SystemTime,
    pub last_accessed: AtomicU64,
    pub access_count: AtomicU32,
}

impl Clone for CacheEntry {
    fn clone(&self) -> Self {
        Self {
            schema: Arc::clone(&self.schema), // Zero-cost Arc clone
            version: self.version,
            created_at: self.created_at,
            last_accessed: AtomicU64::new(self.last_accessed.load(Ordering::Relaxed)),
            access_count: AtomicU32::new(self.access_count.load(Ordering::Relaxed)),
        }
    }
}

impl CacheEntry {
    pub fn new(schema: FhirSchema, version: u64) -> Self {
        Self {
            schema: Arc::new(schema), // Wrap in Arc for efficient sharing
            version,
            created_at: SystemTime::now(),
            last_accessed: AtomicU64::new(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            ),
            access_count: AtomicU32::new(1),
        }
    }

    pub fn touch(&self) {
        self.last_accessed.store(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            Ordering::Relaxed,
        );
        self.access_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn is_hot(&self) -> bool {
        self.access_count.load(Ordering::Relaxed) > 10
    }

    pub fn get_access_count(&self) -> u32 {
        self.access_count.load(Ordering::Relaxed)
    }

    pub fn get_last_accessed(&self) -> u64 {
        self.last_accessed.load(Ordering::Relaxed)
    }
}

#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub l1_size: usize,
    pub l2_size: usize,
    pub promotion_threshold: u32,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            l1_size: 100,
            l2_size: 1000,
            promotion_threshold: 5,
        }
    }
}

pub struct HierarchicalCache {
    l1_hot: Arc<PapayaMap<Url, CacheEntry>>, // Frequently accessed
    l2_warm: Arc<RwLock<LruCache<Url, CacheEntry>>>, // Recently accessed
    l3_storage: Arc<dyn SchemaStorage>,      // Persistent storage

    l1_max_size: usize,
    l2_capacity: usize,
    promotion_threshold: u32,
    current_version: AtomicU64,
}

impl std::fmt::Debug for HierarchicalCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HierarchicalCache")
            .field("l1_max_size", &self.l1_max_size)
            .field("l2_capacity", &self.l2_capacity)
            .field("promotion_threshold", &self.promotion_threshold)
            .field(
                "current_version",
                &self.current_version.load(Ordering::Relaxed),
            )
            .field("l1_hot_size", &self.l1_hot.len())
            .finish()
    }
}

impl HierarchicalCache {
    pub fn new(config: CacheConfig, storage: Arc<dyn SchemaStorage>) -> Self {
        Self {
            l1_hot: Arc::new(PapayaMap::new()),
            l2_warm: Arc::new(RwLock::new(LruCache::new(
                config.l2_size.try_into().unwrap(),
            ))),
            l3_storage: storage,
            l1_max_size: config.l1_size,
            l2_capacity: config.l2_size,
            promotion_threshold: config.promotion_threshold,
            current_version: AtomicU64::new(1),
        }
    }

    pub async fn get(&self, url: &Url) -> Result<Option<Arc<FhirSchema>>> {
        // Check L1 cache
        let result = {
            let l1_guard = self.l1_hot.pin_owned();
            l1_guard.get(url).map(|entry| {
                entry.touch();
                Arc::clone(&entry.schema)
            })
        };
        if let Some(schema) = result {
            return Ok(Some(schema));
        }

        // Check L2 cache
        let promotion_data = {
            let mut l2 = self.l2_warm.write().await;
            if let Some(entry) = l2.get_mut(url) {
                entry.touch();
                let schema = Arc::clone(&entry.schema); // Zero-cost Arc clone
                let should_promote =
                    entry.access_count.load(Ordering::Relaxed) >= self.promotion_threshold;

                if should_promote {
                    // Remove from L2 to promote to L1
                    let entry_for_promotion = l2.pop(url).unwrap();
                    Some((url.clone(), entry_for_promotion, schema))
                } else {
                    Some((url.clone(), entry.clone(), schema))
                }
            } else {
                None
            }
        };

        // Handle L2 cache hit and potential promotion
        if let Some((url, entry, schema)) = promotion_data {
            if entry.access_count.load(Ordering::Relaxed) >= self.promotion_threshold {
                // Promote to L1
                self.promote_to_l1(url, entry).await;
            }
            return Ok(Some(schema));
        }

        // Fetch from storage
        if let Some(schema) = self.l3_storage.get(url).await? {
            let entry = CacheEntry::new(schema, self.get_current_version());
            let schema_arc = Arc::clone(&entry.schema); // Get Arc reference before moving entry

            // Add to L2 cache
            self.l2_warm.write().await.put(url.clone(), entry);

            return Ok(Some(schema_arc));
        }

        Ok(None)
    }

    pub async fn put(&self, url: Url, schema: FhirSchema) -> Result<()> {
        // Store in L3 (persistent storage)
        self.l3_storage.put(url.clone(), schema.clone()).await?;

        // Add to L2 cache
        let entry = CacheEntry::new(schema, self.get_current_version());
        self.l2_warm.write().await.put(url, entry);

        Ok(())
    }

    pub async fn invalidate(&self, url: &Url) {
        // Remove from all cache levels
        {
            let l1_guard = self.l1_hot.pin_owned();
            l1_guard.remove(url);
        }
        self.l2_warm.write().await.pop(url);
    }

    pub async fn mark_stale(&self, url: &Url) {
        // For lazy invalidation, we could mark entries as stale
        // For now, we'll just remove them
        self.invalidate(url).await;
    }

    async fn promote_to_l1(&self, url: Url, entry: CacheEntry) {
        // Check L1 size limit
        let should_evict = {
            let l1_guard = self.l1_hot.pin_owned();
            l1_guard.len() >= self.l1_max_size
        };

        if should_evict {
            self.evict_from_l1().await;
        }

        let l1_guard = self.l1_hot.pin_owned();
        l1_guard.insert(url, entry);
    }

    async fn evict_from_l1(&self) {
        // Find least recently used entry in L1
        let mut oldest_url = None;
        let mut oldest_time = u64::MAX;

        {
            let l1_guard = self.l1_hot.pin_owned();
            for (key, value) in l1_guard.iter() {
                let last_accessed = value.last_accessed.load(Ordering::Relaxed);
                if last_accessed < oldest_time {
                    oldest_time = last_accessed;
                    oldest_url = Some(key.clone());
                }
            }
        }

        if let Some(url) = oldest_url {
            let entry = {
                let l1_guard = self.l1_hot.pin_owned();
                l1_guard.remove(&url).cloned()
            };

            if let Some(entry) = entry {
                // Demote to L2
                self.l2_warm.write().await.put(url, entry);
            }
        }
    }

    fn get_current_version(&self) -> u64 {
        self.current_version.load(Ordering::Relaxed)
    }

    pub fn increment_version(&self) -> u64 {
        self.current_version.fetch_add(1, Ordering::Relaxed)
    }

    pub fn get_l1_size(&self) -> usize {
        let l1_guard = self.l1_hot.pin_owned();
        l1_guard.len()
    }

    pub async fn get_l2_size(&self) -> usize {
        self.l2_warm.read().await.len()
    }

    pub async fn clear(&self) {
        {
            let l1_guard = self.l1_hot.pin_owned();
            l1_guard.clear();
        }
        self.l2_warm.write().await.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::MemoryStorage;

    fn test_config() -> CacheConfig {
        CacheConfig {
            l1_size: 2,
            l2_size: 5,
            promotion_threshold: 3,
        }
    }

    fn test_url() -> Url {
        Url::parse("http://example.com/test").unwrap()
    }

    fn test_schema() -> FhirSchema {
        FhirSchema {
            url: Some(Url::parse("http://example.com/test").unwrap()),
            name: Some("TestSchema".to_string()),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_cache_entry_creation() {
        let schema = test_schema();
        let entry = CacheEntry::new(schema.clone(), 1);

        assert_eq!(*entry.schema, schema); // Dereference Arc for comparison
        assert_eq!(entry.version, 1);
        assert_eq!(entry.get_access_count(), 1);
    }

    #[tokio::test]
    async fn test_cache_entry_touch() {
        let schema = test_schema();
        let entry = CacheEntry::new(schema, 1);

        entry.touch();
        assert_eq!(entry.get_access_count(), 2);

        entry.touch();
        assert_eq!(entry.get_access_count(), 3);
    }

    #[tokio::test]
    async fn test_hierarchical_cache_get_put() {
        let storage = Arc::new(MemoryStorage::new());
        let cache = HierarchicalCache::new(test_config(), storage);
        let url = test_url();
        let schema = test_schema();

        // Put schema
        cache.put(url.clone(), schema.clone()).await.unwrap();

        // Get schema
        let retrieved = cache.get(&url).await.unwrap().unwrap();
        assert_eq!(*retrieved, schema); // Dereference Arc for comparison
    }

    #[tokio::test]
    async fn test_l2_to_l1_promotion() {
        let storage = Arc::new(MemoryStorage::new());
        let cache = HierarchicalCache::new(test_config(), storage);
        let url = test_url();
        let schema = test_schema();

        // Put schema (goes to L2)
        cache.put(url.clone(), schema.clone()).await.unwrap();
        assert_eq!(cache.get_l1_size(), 0);

        // Access multiple times to trigger promotion
        for _ in 0..4 {
            cache.get(&url).await.unwrap();
        }

        // Should be promoted to L1
        assert_eq!(cache.get_l1_size(), 1);
    }

    #[tokio::test]
    async fn test_l1_eviction() {
        let storage = Arc::new(MemoryStorage::new());
        let cache = HierarchicalCache::new(test_config(), storage);

        // Fill L1 cache beyond capacity
        for i in 0..3 {
            let url = Url::parse(&format!("http://example.com/test{i}")).unwrap();
            let mut schema = test_schema();
            schema.url = Some(url.clone());

            cache.put(url.clone(), schema).await.unwrap();

            // Access enough times to promote to L1
            for _ in 0..4 {
                cache.get(&url).await.unwrap();
            }
        }

        // L1 should be at max capacity (2)
        assert_eq!(cache.get_l1_size(), 2);
    }

    #[tokio::test]
    async fn test_cache_invalidation() {
        let storage = Arc::new(MemoryStorage::new());
        let cache = HierarchicalCache::new(test_config(), storage);
        let url = test_url();
        let schema = test_schema();

        // Put and promote to L1
        cache.put(url.clone(), schema).await.unwrap();
        for _ in 0..4 {
            cache.get(&url).await.unwrap();
        }
        assert_eq!(cache.get_l1_size(), 1);

        // Invalidate
        cache.invalidate(&url).await;
        assert_eq!(cache.get_l1_size(), 0);
        assert_eq!(cache.get_l2_size().await, 0);
    }
}
