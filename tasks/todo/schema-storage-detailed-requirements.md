# Schema Storage & Caching - Detailed Requirements

**Status:** TODO  
**Priority:** High  
**Phase Integration:** Spans Phases 1, 2, and 5  

## Overview
Implement comprehensive schema storage and caching capabilities to support saving converted schemas in memory and optionally to disk for future read and use.

## Storage Architecture

### Memory Storage
- **Concurrent HashMap (DashMap)**: Thread-safe storage for active schemas
- **LRU Cache**: Frequently accessed schemas with configurable size limits
- **Automatic Cleanup**: Memory pressure handling and garbage collection
- **Reference Counting**: Track schema usage and dependencies

### Disk Storage (Optional)
- **Feature Flag**: `disk-storage` for optional disk persistence
- **File Format**: JSON/Binary for efficient storage and loading
- **Directory Structure**: Organized by canonical URL hash
- **Metadata**: Store conversion timestamp, source hash, version info
- **Compression**: Optional compression for large schemas

### Storage Manager
- **Unified Interface**: Single API for memory/disk operations
- **Strategy Pattern**: Configurable storage strategies
- **Cache Hierarchy**: Memory → Disk → Network resolution
- **Lazy Loading**: Load schemas on-demand
- **Background Sync**: Async persistence to disk

## Key Features

### 1. Schema Identity & Versioning
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaMetadata {
    pub canonical_url: String,
    pub version: Option<String>,
    pub source_hash: String,      // Hash of source StructureDefinition
    pub conversion_time: SystemTime,
    pub dependencies: Vec<String>, // Other schemas this depends on
    pub size_bytes: usize,
}
```

### 2. Storage Interface
```rust
#[async_trait]
pub trait SchemaStorage: Send + Sync {
    async fn store(&self, url: &str, schema: FhirSchema, metadata: SchemaMetadata) -> Result<(), StorageError>;
    async fn load(&self, url: &str) -> Result<Option<(FhirSchema, SchemaMetadata)>, StorageError>;
    async fn remove(&self, url: &str) -> Result<bool, StorageError>;
    async fn list(&self) -> Result<Vec<SchemaMetadata>, StorageError>;
    async fn clear(&self) -> Result<(), StorageError>;
    fn size(&self) -> usize;
}
```

### 3. Cache Configuration
```rust
#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub memory_limit_mb: usize,        // Default: 256MB
    pub max_entries: usize,            // Default: 1000
    pub disk_cache_dir: Option<PathBuf>, // Optional disk caching
    pub ttl_hours: Option<u64>,        // Time-to-live for entries
    pub enable_compression: bool,       // For disk storage
    pub auto_cleanup: bool,            // Automatic cleanup on memory pressure
}
```

## Implementation Tasks

### Phase 1 Foundation Tasks
- [ ] Define storage traits and error types
- [ ] Implement basic MemoryStorage with DashMap
- [ ] Add LRU cache implementation (using `lru` crate)
- [ ] Create SchemaMetadata and identity handling
- [ ] Set up storage configuration structures

### Phase 2 Integration Tasks
- [ ] Integrate storage with converter pipeline
- [ ] Add automatic schema caching after conversion
- [ ] Implement schema deduplication (by source hash)
- [ ] Add dependency tracking for schema relationships
- [ ] Create storage-aware schema loading

### Phase 5 CLI Tasks
- [ ] Implement `cache list` command with metadata display
- [ ] Add `cache clear` with selective clearing options
- [ ] Create `cache save` for manual schema storage
- [ ] Add `cache stats` for usage statistics
- [ ] Implement `cache export/import` for backup/restore

## Storage Strategies

### 1. Memory-Only Strategy
- **Use Case**: Development, testing, short-lived processes
- **Benefits**: Fast access, no disk I/O
- **Limitations**: Lost on process restart

### 2. Memory + Disk Strategy  
- **Use Case**: Production applications, long-running services
- **Benefits**: Persistent across restarts, hierarchical caching
- **Considerations**: Disk space management, sync overhead

### 3. Disk-Only Strategy
- **Use Case**: Memory-constrained environments
- **Benefits**: Low memory footprint
- **Trade-offs**: Slower access, more I/O

## Performance Considerations

### Memory Management
- **LRU Eviction**: Remove least recently used schemas
- **Size-Based Limits**: Prevent memory exhaustion
- **Background Cleanup**: Async cleanup operations
- **Memory Pressure**: Respond to system memory pressure

### Disk I/O Optimization
- **Async Operations**: Non-blocking disk I/O
- **Batch Operations**: Batch multiple operations
- **Compression**: Reduce disk space usage
- **File Organization**: Efficient directory structure

### Concurrency
- **Thread Safety**: All operations thread-safe
- **Read-Heavy Optimization**: Optimize for frequent reads
- **Lock-Free Operations**: Minimize locking overhead
- **Async/Await**: Full async support

## Error Handling

### Storage Errors
```rust
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Schema not found: {url}")]
    NotFound { url: String },
    
    #[error("Storage full: cannot store more schemas")]
    StorageFull,
    
    #[error("Disk I/O error: {0}")]
    DiskError(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("Concurrent access error")]
    ConcurrencyError,
}
```

## Testing Strategy

### Unit Tests
- [ ] Memory storage operations
- [ ] LRU cache behavior
- [ ] Disk storage (when feature enabled)
- [ ] Error conditions and edge cases
- [ ] Concurrent access patterns

### Integration Tests
- [ ] Storage integration with converter
- [ ] Cache hierarchy behavior
- [ ] Memory pressure handling
- [ ] Disk persistence across restarts

### Performance Tests
- [ ] Memory usage under load
- [ ] Cache hit/miss ratios
- [ ] Disk I/O performance
- [ ] Concurrent access benchmarks

## Configuration Examples

### Basic Memory-Only
```toml
[storage]
strategy = "memory"
memory_limit_mb = 128
max_entries = 500
```

### Memory + Disk
```toml
[storage]
strategy = "memory-disk"
memory_limit_mb = 256
max_entries = 1000
disk_cache_dir = "~/.octofhir/schema-cache"
enable_compression = true
ttl_hours = 24
```

### Disk-Only (Memory Constrained)
```toml
[storage]
strategy = "disk"
disk_cache_dir = "/var/cache/octofhir/schemas"
enable_compression = true
max_disk_size_gb = 1
```

## Success Criteria
- [ ] Schemas persist correctly across application restarts (disk storage)
- [ ] Memory usage stays within configured limits
- [ ] Cache hit rates > 80% for repeated schema access
- [ ] All storage operations are thread-safe
- [ ] CLI cache management commands work correctly
- [ ] Storage integrates seamlessly with converter and validator