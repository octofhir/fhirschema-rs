//! Rate limiting middleware

use crate::config::RateLimitConfig;

/// Rate limiting layer
pub struct RateLimitLayer;

impl RateLimitLayer {
    pub fn new(_config: &RateLimitConfig) -> tower::layer::util::Identity {
        // TODO: Implement rate limiting middleware
        tower::layer::util::Identity::new()
    }
}
