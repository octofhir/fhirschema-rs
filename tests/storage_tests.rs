mod common;

#[cfg(feature = "memory-storage")]
mod memory_storage_tests {
    use super::common::*;
    use octofhir_fhirschema::*;
    use std::sync::Arc;
    use url::Url;

    #[tokio::test]
    async fn test_memory_storage_basic_operations() {
        let storage = MemoryStorage::new();
        let url = Url::parse("http://example.com/Patient").unwrap();
        let schema = create_test_schema();

        assert!(storage.put(url.clone(), schema.clone()).await.is_ok());

        let retrieved = storage.get(&url).await.unwrap();
        assert_eq!(retrieved, Some(schema));

        assert!(storage.contains(&url).await.unwrap());
        assert_eq!(storage.size().await.unwrap(), 1);

        let removed = storage.remove(&url).await.unwrap();
        assert!(removed);
        assert_eq!(storage.size().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_memory_storage_list() {
        let storage = MemoryStorage::new();
        let url1 = Url::parse("http://example.com/Patient").unwrap();
        let url2 = Url::parse("http://example.com/Observation").unwrap();
        let schema = create_test_schema();

        storage.put(url1.clone(), schema.clone()).await.unwrap();
        storage.put(url2.clone(), schema).await.unwrap();

        let urls = storage.list().await.unwrap();
        assert_eq!(urls.len(), 2);
        assert!(urls.contains(&url1));
        assert!(urls.contains(&url2));
    }

    #[tokio::test]
    async fn test_lru_cache_basic_operations() {
        let cache = LruSchemaCache::new(2);
        let url = Url::parse("http://example.com/Patient").unwrap();
        let schema = create_test_schema();

        assert!(cache.get(&url).await.is_none());

        cache.put(url.clone(), schema.clone()).await;
        let retrieved = cache.get(&url).await;
        assert_eq!(retrieved, Some(schema));

        assert_eq!(cache.size().await, 1);

        let removed = cache.remove(&url).await;
        assert!(removed);
        assert_eq!(cache.size().await, 0);
    }

    #[tokio::test]
    async fn test_lru_cache_eviction() {
        let cache = LruSchemaCache::new(2);
        let url1 = Url::parse("http://example.com/Patient").unwrap();
        let url2 = Url::parse("http://example.com/Observation").unwrap();
        let url3 = Url::parse("http://example.com/Condition").unwrap();
        let schema = create_test_schema();

        cache.put(url1.clone(), schema.clone()).await;
        cache.put(url2.clone(), schema.clone()).await;
        cache.put(url3.clone(), schema.clone()).await;

        // url1 should be evicted due to LRU policy
        assert!(cache.get(&url1).await.is_none());
        assert!(cache.get(&url2).await.is_some());
        assert!(cache.get(&url3).await.is_some());
    }

    #[tokio::test]
    async fn test_enhanced_storage_manager() {
        let storage = Arc::new(MemoryStorage::new());
        let config = StorageConfig {
            storage,
            cache: CacheConfig::default(),
        };
        let manager = EnhancedStorageManager::new(config);

        let url = Url::parse("http://example.com/Patient").unwrap();
        let schema = create_test_schema();

        // Put through manager
        assert!(manager.put(url.clone(), schema.clone()).await.is_ok());

        // Get should hit cache on second access
        let retrieved1 = manager.get(&url).await.unwrap();
        let retrieved2 = manager.get(&url).await.unwrap();

        assert_eq!(retrieved1, Some(schema.clone()));
        assert_eq!(retrieved2, Some(schema));

        // Remove through manager
        let removed = manager.delete(&url).await.unwrap();
        assert!(removed);

        let after_removal = manager.get(&url).await.unwrap();
        assert!(after_removal.is_none());
    }
}
