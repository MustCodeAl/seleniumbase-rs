//! Azure Blob Storage logging plugin.
//!
//! This module is gated behind the `azure` feature. It uploads test artifacts to
//! Azure Blob Storage using `azure_storage_blob::BlobClient`.
//!
//! Authentication is performed with a SAS URL. Set `AZURE_BLOB_URL` to the full
//! blob URL including the SAS token, for example:
//!
//! ```bash
//! export AZURE_BLOB_URL="https://myaccount.blob.core.windows.net/mycontainer/myblob?sv=...&sig=..."
//! ```
//!
//! A future release may support Entra ID credentials and shared-key auth.

use azure_storage_blob::BlobClient;
use std::path::Path;
use thiserror::Error;
use tokio::fs;

/// Errors that can occur during Azure Blob upload.
#[derive(Debug, Error)]
pub enum AzureUploadError {
    #[error("missing environment variable {0}")]
    MissingEnv(&'static str),
    #[error("invalid blob URL: {0}")]
    InvalidUrl(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("upload failed: {0}")]
    Upload(String),
}

/// Uploads test artifacts, such as the files the failure plugins save, to Azure
/// Blob Storage.
///
/// It has no hooks of its own: call [`upload_file`](Self::upload_file) after a
/// test, for example on the screenshot a
/// [`ScreenshotOnFailurePlugin`](crate::plugins::screen_shots::ScreenshotOnFailurePlugin)
/// wrote.
pub struct AzureLoggingPlugin;

impl AzureLoggingPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Upload `path` to the Azure Blob URL given by `AZURE_BLOB_URL`.
    ///
    /// The URL must include a SAS token with write permission.
    pub async fn upload_file(path: &Path) -> Result<(), AzureUploadError> {
        let url_str = std::env::var("AZURE_BLOB_URL")
            .map_err(|_| AzureUploadError::MissingEnv("AZURE_BLOB_URL"))?;
        let url =
            url::Url::parse(&url_str).map_err(|e| AzureUploadError::InvalidUrl(e.to_string()))?;

        let bytes = fs::read(path).await?;
        let client = BlobClient::new(url, None, None)
            .map_err(|e| AzureUploadError::Upload(e.to_string()))?;
        client
            .upload(azure_core::http::RequestContent::from(bytes), None)
            .await
            .map_err(|e| AzureUploadError::Upload(e.to_string()))?;
        Ok(())
    }
}

impl Default for AzureLoggingPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_file_requires_blob_url() {
        let err = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(AzureLoggingPlugin::upload_file(Path::new(
                "/nonexistent/path",
            )))
            .unwrap_err();
        assert!(matches!(err, AzureUploadError::MissingEnv(_)));
    }
}
