//! Google Cloud Storage logging plugin.
//!
//! This module is gated behind the `gcp` feature. It uploads test artifacts to
//! Google Cloud Storage (GCS) using the JSON API and an access token read from
//! the `GCS_ACCESS_TOKEN` environment variable. You can obtain a token with:
//!
//! ```bash
//! gcloud auth print-access-token
//! ```
//!
//! Alternatively, set `GOOGLE_APPLICATION_CREDENTIALS` to a service-account
//! JSON file; in a future release the plugin can exchange it for an access
//! token automatically.

use crate::plugins::base_plugin::SeleniumBasePlugin;
use reqwest::Client;
use std::path::Path;
use thiserror::Error;
use tokio::fs;
use url::Url;

/// Errors that can occur during a GCS upload.
#[derive(Debug, Error)]
pub enum GcsUploadError {
    #[error("missing environment variable {0}")]
    MissingEnv(&'static str),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("upload failed: status={status} body={body}")]
    Upload { status: u16, body: String },
    #[error("request failed: {0}")]
    Request(#[from] reqwest::Error),
}

/// Plugin that logs test activity to stdout and can upload artifacts to GCS.
pub struct GcpLoggingPlugin;

impl GcpLoggingPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Upload `path` to the GCS `bucket` under `object_name`.
    ///
    /// Authentication uses `GCS_ACCESS_TOKEN`.
    pub async fn upload_file(
        bucket: &str,
        object_name: &str,
        path: &Path,
    ) -> Result<(), GcsUploadError> {
        let token = std::env::var("GCS_ACCESS_TOKEN")
            .map_err(|_| GcsUploadError::MissingEnv("GCS_ACCESS_TOKEN"))?;

        let bytes = fs::read(path).await?;
        let url = Url::parse_with_params(
            &format!(
                "https://storage.googleapis.com/upload/storage/v1/b/{}/o",
                bucket
            ),
            &[("uploadType", "media"), ("name", object_name)],
        )
        .map_err(|e| GcsUploadError::Upload {
            status: 0,
            body: format!("invalid URL: {}", e),
        })?;

        let client = Client::new();
        let response = client
            .post(url.as_str())
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/octet-stream")
            .body(bytes)
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(GcsUploadError::Upload {
                status: status.as_u16(),
                body,
            })
        }
    }
}

impl SeleniumBasePlugin for GcpLoggingPlugin {
    fn on_start(&mut self) {
        println!("[GcpLoggingPlugin] test session started");
    }

    fn before_command(&mut self, name: &str, target: &str, value: &str) {
        println!(
            "[GcpLoggingPlugin] before {} target={} value={}",
            name, target, value
        );
    }

    fn after_command(&mut self, name: &str, target: &str, value: &str, passed: bool) {
        println!(
            "[GcpLoggingPlugin] after {} target={} value={} passed={}",
            name, target, value, passed
        );
    }

    fn on_stop(&mut self) {
        println!("[GcpLoggingPlugin] test session stopped");
    }
}

impl Default for GcpLoggingPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_file_requires_access_token() {
        let err = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(GcpLoggingPlugin::upload_file(
                "test-bucket",
                "test/object.log",
                Path::new("/nonexistent/path"),
            ))
            .unwrap_err();
        assert!(matches!(err, GcsUploadError::MissingEnv(_)));
    }
}
