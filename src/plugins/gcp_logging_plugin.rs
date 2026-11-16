//! Google Cloud Storage logging plugin.
//!
//! This module is gated behind the `gcp` feature. It uploads test artifacts to
//! Google Cloud Storage (GCS) using the JSON API. [`from_env`](GcpLoggingPlugin::from_env)
//! reads `SB_GCS_BUCKET` and `GCS_ACCESS_TOKEN`. A token comes from:
//!
//! ```bash
//! gcloud auth print-access-token
//! ```
//!
//! [`from_lookup`](GcpLoggingPlugin::from_lookup) takes those names from a
//! closure, so a test does not change the process environment.

use std::fmt;
use std::path::Path;

use reqwest::Client;
use thiserror::Error;
use tokio::fs;
use url::Url;

/// Errors that can occur during a GCS upload.
#[derive(Debug, Error)]
#[non_exhaustive]
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

/// Uploads test artifacts, such as the files the failure plugins save, to one GCS bucket.
///
/// It has no hooks of its own: build it with [`from_env`](Self::from_env) or
/// [`from_lookup`](Self::from_lookup), then call [`upload`](Self::upload) after a
/// test, for example on the screenshot a
/// [`ScreenshotOnFailurePlugin`](crate::plugins::screen_shots::ScreenshotOnFailurePlugin)
/// wrote.
///
/// `Debug` names the bucket and not the access token.
#[non_exhaustive]
pub struct GcpLoggingPlugin {
    bucket: String,
    token: String,
}

impl fmt::Debug for GcpLoggingPlugin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GcpLoggingPlugin")
            .field("bucket", &self.bucket)
            .finish_non_exhaustive()
    }
}

impl GcpLoggingPlugin {
    /// The bucket `SB_GCS_BUCKET` names, with `GCS_ACCESS_TOKEN`.
    ///
    /// # Errors
    ///
    /// Returns [`GcsUploadError::MissingEnv`] when either value is unset or blank.
    pub fn from_env() -> Result<Self, GcsUploadError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// The same bucket and token as [`from_env`](Self::from_env), read through `lookup`.
    ///
    /// # Errors
    ///
    /// As [`from_env`](Self::from_env).
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, GcsUploadError> {
        let bucket = required(&lookup, "SB_GCS_BUCKET")?;
        let token = required(&lookup, "GCS_ACCESS_TOKEN")?;
        Ok(Self { bucket, token })
    }

    /// Uploads the file at `path` as `object_name` in this plugin's bucket.
    ///
    /// # Errors
    ///
    /// Returns [`GcsUploadError::Io`] when the file cannot be read,
    /// [`GcsUploadError::Request`] when the call cannot be sent, and
    /// [`GcsUploadError::Upload`] when GCS rejects it.
    pub async fn upload(
        &self,
        path: impl AsRef<Path>,
        object_name: &str,
    ) -> Result<(), GcsUploadError> {
        let bytes = fs::read(path.as_ref()).await?;
        let url = Url::parse_with_params(
            &format!(
                "https://storage.googleapis.com/upload/storage/v1/b/{}/o",
                self.bucket
            ),
            &[("uploadType", "media"), ("name", object_name)],
        )
        .map_err(|error| GcsUploadError::Upload {
            status: 0,
            body: format!("invalid URL: {error}"),
        })?;

        crate::install_tls_provider();
        let client = Client::builder()
            .connect_timeout(std::time::Duration::from_secs(30))
            .read_timeout(std::time::Duration::from_secs(120))
            .build()?;
        let response = client
            .post(url.as_str())
            .header("Authorization", format!("Bearer {}", self.token))
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

fn required(
    lookup: &impl Fn(&str) -> Option<String>,
    name: &'static str,
) -> Result<String, GcsUploadError> {
    lookup(name)
        .filter(|value| !value.trim().is_empty())
        .ok_or(GcsUploadError::MissingEnv(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_lookup_reports_a_missing_bucket_without_reading_the_process() {
        let error = GcpLoggingPlugin::from_lookup(|_| None).unwrap_err();
        assert!(matches!(error, GcsUploadError::MissingEnv("SB_GCS_BUCKET")));
    }

    #[test]
    fn debug_names_the_bucket_and_not_the_token() {
        let plugin = GcpLoggingPlugin::from_lookup(|name| match name {
            "SB_GCS_BUCKET" => Some("logs".to_owned()),
            "GCS_ACCESS_TOKEN" => Some("secret-token".to_owned()),
            _ => None,
        })
        .unwrap();
        let rendered = format!("{plugin:?}");
        assert!(rendered.contains("logs"));
        assert!(!rendered.contains("secret-token"));
    }
}
