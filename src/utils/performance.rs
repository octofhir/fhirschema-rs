// Performance monitoring utilities

#[allow(dead_code)]
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PerformanceMetrics {
    pub operation_name: String,
    pub start_time: Instant,
    pub duration: Option<Duration>,
    pub success: bool,
    pub metadata: std::collections::HashMap<String, String>,
}

impl PerformanceMetrics {
    pub fn start(operation_name: &str) -> Self {
        Self {
            operation_name: operation_name.to_string(),
            start_time: Instant::now(),
            duration: None,
            success: false,
            metadata: std::collections::HashMap::new(),
        }
    }

    pub fn finish(mut self, success: bool) -> Self {
        self.duration = Some(self.start_time.elapsed());
        self.success = success;
        self
    }

    pub fn add_metadata(&mut self, key: &str, value: &str) {
        self.metadata.insert(key.to_string(), value.to_string());
    }
}

/// Simple performance timer
pub struct Timer {
    start: Instant,
}

impl Timer {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    pub fn elapsed_ms(&self) -> u128 {
        self.start.elapsed().as_millis()
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}
