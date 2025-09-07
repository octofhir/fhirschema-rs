// Async utility functions

#[allow(dead_code)]
use std::future::Future;
use std::time::Duration;
use tokio::time::timeout;

/// Runs a future with a timeout, returning an error if it times out
#[allow(dead_code)]
pub async fn with_timeout<F, T>(future: F, duration: Duration) -> crate::error::Result<T>
where
    F: Future<Output = T>,
{
    timeout(duration, future)
        .await
        .map_err(|_| crate::error::FhirSchemaError::Runtime {
            message: format!("Operation timed out after {duration:?}"),
        })
}

/// Utility for batching async operations
#[allow(dead_code)]
pub async fn batch_process<T, R, F, Fut>(items: Vec<T>, batch_size: usize, processor: F) -> Vec<R>
where
    T: Clone,
    F: Fn(T) -> Fut,
    Fut: Future<Output = R>,
{
    let mut results = Vec::new();

    for chunk in items.chunks(batch_size) {
        let futures: Vec<_> = chunk.iter().cloned().map(&processor).collect();

        let batch_results = futures::future::join_all(futures).await;
        results.extend(batch_results);
    }

    results
}
