// Debug implementations for storage components that don't have automatic Debug derive

use crate::storage::{SchemaStorage, StorageConfig};
use std::fmt;

impl fmt::Debug for dyn SchemaStorage + Send + Sync {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SchemaStorage")
            .field("type", &std::any::type_name::<Self>())
            .finish()
    }
}

// Custom Debug implementation for StorageConfig that handles the trait object
impl fmt::Debug for StorageConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StorageConfig")
            .field("storage", &"Arc<dyn SchemaStorage>")
            .field("cache", &self.cache)
            .finish()
    }
}
