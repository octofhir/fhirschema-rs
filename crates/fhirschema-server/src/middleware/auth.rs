//! Authentication middleware

use crate::config::AuthConfig;

/// Authentication layer (placeholder)
pub struct AuthLayer;

impl AuthLayer {
    pub fn new(_config: &AuthConfig) -> tower::layer::util::Identity {
        // TODO: Implement authentication middleware
        tower::layer::util::Identity::new()
    }
}
