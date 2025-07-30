//! IG registry service

use std::sync::Arc;

use crate::{
    config::IgRegistryConfig,
    error::Result,
    jobs::JobManager,
    storage::S3Storage,
};

/// IG registry service
pub struct IgRegistryService {
    storage: Arc<S3Storage>,
    job_manager: Arc<JobManager>,
    config: IgRegistryConfig,
}

impl IgRegistryService {
    /// Create new IG registry service
    pub async fn new(
        storage: Arc<S3Storage>,
        job_manager: Arc<JobManager>,
        config: &IgRegistryConfig,
    ) -> Result<Self> {
        Ok(Self {
            storage,
            job_manager,
            config: config.clone(),
        })
    }

    // TODO: Implement IG registry methods
}
