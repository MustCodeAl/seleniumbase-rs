use s3::creds::Credentials;
use s3::{Bucket, Region};
use std::path::Path;
use std::str::FromStr;
use thiserror::Error;
use tokio::fs::File;

/// Errors that can occur during S3 upload.
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
    /// - `AWS_REGION` (used as a fallback when no explicit region is supplied)
    /// - `AWS_ENDPOINT_URL` (optional, enables S3-compatible stores such as MinIO)
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

        let credentials = Credentials::new(Some(&access_key), Some(&secret_key), None, None, None)
            .map_err(|e| S3UploadError::Credentials(e.to_string()))?;

        let region = match std::env::var("AWS_ENDPOINT_URL") {
            Ok(endpoint) => Region::Custom {
                region: region.to_owned(),
                endpoint,
            },
            Err(_) => Region::from_str(region).map_err(|e| S3UploadError::Region(e.to_string()))?,
        };

        let bucket = Bucket::new(bucket, region, credentials)
            .map_err(|e| S3UploadError::Bucket(e.to_string()))?
            .with_path_style();

        let mut file = File::open(path).await?;
        bucket
            .put_object_stream(&mut file, key)
            .await
            .map_err(|e| S3UploadError::Upload(e.to_string()))?;

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
        // Ensure missing credentials produce a clear error without touching the network.
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
