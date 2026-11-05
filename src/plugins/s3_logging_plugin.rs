//! S3 logging plugin.
//!
//! Uploads test artifacts through [`object_store::aws::AmazonS3Builder`]
//! (object_store 0.12, feature `aws`). Docs: <https://docs.rs/object_store/0.12.4/object_store/aws/struct.AmazonS3Builder.html>.

use std::path::Path;

use object_store::aws::AmazonS3Builder;
use thiserror::Error;
use tokio::fs;

use super::object_log;

/// Errors that can occur during an S3 upload.
#[derive(Debug, Error)]
pub enum S3UploadError {
    #[error("missing environment variable {0}")]
    MissingEnv(&'static str),
    #[error("invalid credentials: {0}")]
    Credentials(String),
    #[error("invalid region: {0}")]
    Region(String),
    #[error("failed to create bucket handle: {0}")]
    Bucket(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("upload failed: {0}")]
    Upload(String),
}

/// Uploads test artifacts, such as the files the failure plugins save, to S3.
///
/// It has no hooks of its own: call [`upload_file`](Self::upload_file) after a
/// test, for example on the screenshot a
/// [`ScreenshotOnFailurePlugin`](crate::plugins::screen_shots::ScreenshotOnFailurePlugin)
/// wrote.
pub struct S3LoggingPlugin;

impl S3LoggingPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Upload `path` to the given S3 `bucket` under `key`.
    ///
    /// Credentials are read from the environment:
    /// - `AWS_ACCESS_KEY_ID`
    /// - `AWS_SECRET_ACCESS_KEY`
    /// - `AWS_REGION` is not required; pass `region`
    /// - `AWS_ENDPOINT_URL` (optional, enables S3-compatible stores such as MinIO)
    ///
    /// # Errors
    ///
    /// Returns [`S3UploadError::MissingEnv`] when a credential is absent, and
    /// [`S3UploadError::Upload`] when the store rejects the write.
    pub async fn upload_file(
        bucket: &str,
        key: &str,
        path: &Path,
        region: &str,
    ) -> Result<(), S3UploadError> {
        let access_key = std::env::var("AWS_ACCESS_KEY_ID")
            .map_err(|_| S3UploadError::MissingEnv("AWS_ACCESS_KEY_ID"))?;
        let secret_key = std::env::var("AWS_SECRET_ACCESS_KEY")
            .map_err(|_| S3UploadError::MissingEnv("AWS_SECRET_ACCESS_KEY"))?;

        let mut builder = AmazonS3Builder::new()
            .with_bucket_name(bucket)
            .with_region(region)
            .with_access_key_id(access_key)
            .with_secret_access_key(secret_key);
        if let Ok(endpoint) = std::env::var("AWS_ENDPOINT_URL") {
            if endpoint.starts_with("http://") {
                builder = builder.with_allow_http(true);
            }
            builder = builder.with_endpoint(endpoint);
        }
        let store = builder
            .build()
            .map_err(|error| S3UploadError::Bucket(error.to_string()))?;

        let bytes = fs::read(path).await?;
        object_log::put_bytes(&store, key, bytes)
            .await
            .map_err(|error| S3UploadError::Upload(error.to_string()))?;
        Ok(())
    }
}

impl Default for S3LoggingPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_file_requires_env_vars() {
        let err = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(S3LoggingPlugin::upload_file(
                "test-bucket",
                "test/key.log",
                Path::new("/nonexistent/path"),
                "us-east-1",
            ))
            .unwrap_err();
        assert!(matches!(err, S3UploadError::MissingEnv(_)));
    }
}
