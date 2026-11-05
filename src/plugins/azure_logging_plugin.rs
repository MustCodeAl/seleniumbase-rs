//! Azure Blob Storage logging plugin.
//!
//! Uploads through [`object_store::azure::MicrosoftAzureBuilder`]
//! (object_store 0.12, feature `azure`). Docs: <https://docs.rs/object_store/0.12.4/object_store/azure/struct.MicrosoftAzureBuilder.html>.
//!
//! Authentication is a SAS URL. Set `AZURE_BLOB_URL` to the full blob URL
//! including the SAS token, for example:
//!
//! ```bash
//! export AZURE_BLOB_URL="https://myaccount.blob.core.windows.net/mycontainer/myblob?sv=...&sig=..."
//! ```

use std::path::Path;

use object_store::azure::MicrosoftAzureBuilder;
use thiserror::Error;
use tokio::fs;

use super::object_log;

/// Errors that can occur during an Azure Blob upload.
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
    /// The URL must include a SAS token with write permission. The account is
    /// the first label of the host, the container is the first path segment,
    /// and the rest of the path is the blob name.
    ///
    /// # Errors
    ///
    /// Returns [`AzureUploadError::MissingEnv`] when `AZURE_BLOB_URL` is unset,
    /// and [`AzureUploadError::InvalidUrl`] when the URL has no account or container.
    pub async fn upload_file(path: &Path) -> Result<(), AzureUploadError> {
        let url_str = std::env::var("AZURE_BLOB_URL")
            .map_err(|_| AzureUploadError::MissingEnv("AZURE_BLOB_URL"))?;
        let url = url::Url::parse(&url_str)
            .map_err(|error| AzureUploadError::InvalidUrl(error.to_string()))?;
        let host = url
            .host_str()
            .ok_or_else(|| AzureUploadError::InvalidUrl("blob URL has no host".to_owned()))?;
        let account = host
            .split('.')
            .next()
            .filter(|label| !label.is_empty())
            .ok_or_else(|| AzureUploadError::InvalidUrl("blob URL has no account".to_owned()))?;
        let mut segments = url.path().trim_start_matches('/').splitn(2, '/');
        let container = segments
            .next()
            .filter(|segment| !segment.is_empty())
            .ok_or_else(|| AzureUploadError::InvalidUrl("blob URL has no container".to_owned()))?;
        let blob = segments
            .next()
            .filter(|segment| !segment.is_empty())
            .ok_or_else(|| AzureUploadError::InvalidUrl("blob URL has no blob name".to_owned()))?;
        let sas: Vec<(String, String)> = url
            .query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect();

        let store = MicrosoftAzureBuilder::new()
            .with_account(account)
            .with_container_name(container)
            .with_sas_authorization(sas)
            .build()
            .map_err(|error| AzureUploadError::Upload(error.to_string()))?;

        let bytes = fs::read(path).await?;
        object_log::put_bytes(&store, blob, bytes)
            .await
            .map_err(|error| AzureUploadError::Upload(error.to_string()))?;
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
