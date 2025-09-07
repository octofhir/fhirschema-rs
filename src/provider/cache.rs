use lru::LruCache;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::Result;
use crate::provider::fhir_model_provider::TypeHierarchy;

#[derive(Debug)]
pub struct ModelProviderCache {
    hierarchy_cache: Arc<RwLock<LruCache<String, CachedHierarchy>>>,
    stats: Arc<RwLock<CacheStats>>,
}

#[derive(Debug, Clone)]
struct CachedHierarchy {
    hierarchy: TypeHierarchy,
    timestamp: std::time::Instant,
    access_count: u32,
}

#[derive(Debug, Clone, Default)]
struct CacheStats {
    hierarchy_hits: u64,
    hierarchy_misses: u64,
    total_entries: usize,
}

impl ModelProviderCache {
    pub fn new() -> Self {
        Self {
            hierarchy_cache: Arc::new(RwLock::new(LruCache::new(2000.try_into().unwrap()))),
            stats: Arc::new(RwLock::new(CacheStats::default())),
        }
    }

    pub async fn get_hierarchy(&self, type_name: &str) -> Option<TypeHierarchy> {
        let mut cache = self.hierarchy_cache.write().await;
        let mut stats = self.stats.write().await;

        if let Some(cached) = cache.get_mut(type_name) {
            // Check if cache entry is still fresh (30 minutes)
            if cached.timestamp.elapsed().as_secs() < 1800 {
                cached.access_count += 1;
                stats.hierarchy_hits += 1;
                return Some(cached.hierarchy.clone());
            } else {
                // Remove stale entry
                cache.pop(type_name);
            }
        }

        stats.hierarchy_misses += 1;
        None
    }

    pub async fn cache_hierarchy(&self, type_name: &str, hierarchy: &TypeHierarchy) -> Result<()> {
        let mut cache = self.hierarchy_cache.write().await;
        let mut stats = self.stats.write().await;

        cache.put(
            type_name.to_string(),
            CachedHierarchy {
                hierarchy: hierarchy.clone(),
                timestamp: std::time::Instant::now(),
                access_count: 0,
            },
        );

        stats.total_entries = cache.len();
        Ok(())
    }

    pub async fn clear(&self) -> Result<()> {
        let mut cache = self.hierarchy_cache.write().await;
        let mut stats = self.stats.write().await;

        cache.clear();
        stats.total_entries = 0;

        Ok(())
    }

    pub async fn get_stats(&self) -> serde_json::Value {
        let stats = self.stats.read().await;
        let cache = self.hierarchy_cache.read().await;

        let hit_rate = if stats.hierarchy_hits + stats.hierarchy_misses > 0 {
            stats.hierarchy_hits as f64 / (stats.hierarchy_hits + stats.hierarchy_misses) as f64
        } else {
            0.0
        };

        serde_json::json!({
            "hierarchy_cache": {
                "hits": stats.hierarchy_hits,
                "misses": stats.hierarchy_misses,
                "hit_rate": hit_rate,
                "entries": cache.len(),
                "capacity": cache.cap()
            }
        })
    }
}

impl Default for ModelProviderCache {
    fn default() -> Self {
        Self::new()
    }
}
