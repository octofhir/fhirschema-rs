//! S3-compatible storage implementation

use aws_sdk_s3::Client as S3Client;

use crate::error::Result;

/// S3-compatible storage client
pub struct S3Storage {
    client: S3Client,
    bucket: String,
    prefix: String,
}

impl S3Storage {
    /// Create new S3 storage client
    pub fn new(client: S3Client, bucket: String, prefix: String) -> Self {
        Self {
            client,
            bucket,
            prefix,
        }
    }

    // TODO: Implement S3 storage methods
}
