//! Azure Blob Storage logging plugin.
//!
//! Uploads through [`object_store::azure::MicrosoftAzureBuilder`]
//! (object_store 0.14.2, feature `azure`). Docs: <https://docs.rs/object_store/0.14.2/object_store/azure/struct.MicrosoftAzureBuilder.html>.
//!
//! [`from_env`](AzureLoggingPlugin::from_env) reads `AZURE_BLOB_URL`, a blob URL
//! with a SAS token that has write permission. [`from_lookup`](AzureLoggingPlugin::from_lookup)
//! takes that name from a closure.

use std::fmt;
use std::path::Path;
use std::sync::Arc;

use object_store::azure::MicrosoftAzureBuilder;
use object_store::ObjectStore;
use thiserror::Error;
use tokio::fs;

use super::object_log;

/// Errors that can occur during an Azure Blob upload.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AzureUploadError {
    /// A required variable was missing or blank.
    #[error("missing environment variable {0}")]
    MissingEnv(&'static str),
    /// The blob URL could not be read as a URL we build.
    #[error("invalid blob URL: {0}")]
    InvalidUrl(String),
    /// `AZURE_BLOB_URL` was not a URL. The parser error is
    /// [`Error::source`](std::error::Error::source).
    #[error("invalid blob URL: {message}")]
    MalformedUrl {
        /// What the parser reported.
        message: String,
        /// The parser error.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
    /// The file could not be read.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// The store rejected the write or could not be built. The store error is
    /// [`Error::source`](std::error::Error::source).
    #[error("upload failed: {message}")]
    Upload {
        /// What the store reported.
        message: String,
        /// The store error.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
}

impl AzureUploadError {
    fn malformed(error: impl std::error::Error + Send + Sync + 'static) -> Self {
        let message = error.to_string();
        Self::MalformedUrl {
            message,
            source: Box::new(error),
        }
    }

    fn upload(error: impl std::error::Error + Send + Sync + 'static) -> Self {
        let message = error.to_string();
        Self::Upload {
            message,
            source: Box::new(error),
        }
    }
}

/// Uploads test artifacts, such as the files the failure plugins save, to one Azure blob.
///
/// It has no hooks of its own: build it with [`from_env`](Self::from_env) or
/// [`from_lookup`](Self::from_lookup), then call [`upload`](Self::upload) after a
/// test, for example on the screenshot a
/// [`ScreenshotOnFailurePlugin`](crate::plugins::screen_shots::ScreenshotOnFailurePlugin)
/// wrote.
///
/// `Debug` names the type and not the SAS token.
#[non_exhaustive]
pub struct AzureLoggingPlugin {
    store: Arc<dyn ObjectStore>,
    blob: String,
}

impl fmt::Debug for AzureLoggingPlugin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AzureLoggingPlugin")
            .field("blob", &self.blob)
            .finish_non_exhaustive()
    }
}

impl AzureLoggingPlugin {
    /// The blob `AZURE_BLOB_URL` names.
    ///
    /// The URL must include a SAS token with write permission. The account is
    /// the first label of the host, the container is the first path segment,
    /// and the rest of the path is the blob name.
    ///
    /// # Errors
    ///
    /// Returns [`AzureUploadError::MissingEnv`] when `AZURE_BLOB_URL` is unset,
    /// [`AzureUploadError::MalformedUrl`] when it is not a URL, and
    /// [`AzureUploadError::InvalidUrl`] when it has no account, container, or blob
    /// name.
    pub fn from_env() -> Result<Self, AzureUploadError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// The same blob as [`from_env`](Self::from_env), read through `lookup`.
    ///
    /// # Errors
    ///
    /// As [`from_env`](Self::from_env).
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, AzureUploadError> {
        let url_str = lookup("AZURE_BLOB_URL")
            .filter(|value| !value.trim().is_empty())
            .ok_or(AzureUploadError::MissingEnv("AZURE_BLOB_URL"))?;
        let url = url::Url::parse(&url_str).map_err(AzureUploadError::malformed)?;
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
            .map_err(AzureUploadError::upload)?;
        Ok(Self {
            store: Arc::new(store),
            blob: blob.to_owned(),
        })
    }

    /// Uploads the file at `path` to the blob this plugin was built for.
    ///
    /// # Errors
    ///
    /// Returns [`AzureUploadError::Io`] when the file cannot be read, and
    /// [`AzureUploadError::Upload`] when the store rejects the write.
    pub async fn upload(&self, path: impl AsRef<Path>) -> Result<(), AzureUploadError> {
        let bytes = fs::read(path.as_ref()).await?;
        object_log::put_bytes(self.store.as_ref(), &self.blob, bytes)
            .await
            .map_err(AzureUploadError::upload)?;
        Ok(())
    }

    #[cfg(test)]
    fn for_store(store: Arc<dyn ObjectStore>, blob: String) -> Self {
        Self { store, blob }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use object_store::memory::InMemory;
    use object_store::path::Path as ObjectPath;
    use object_store::{ObjectStore, ObjectStoreExt};

    use super::*;

    #[test]
    fn from_lookup_reports_a_missing_url_without_reading_the_process() {
        let error = AzureLoggingPlugin::from_lookup(|_| None).unwrap_err();
        assert!(matches!(
            error,
            AzureUploadError::MissingEnv("AZURE_BLOB_URL")
        ));
    }

    #[test]
    fn from_lookup_rejects_a_url_with_no_blob_name() {
        let error = AzureLoggingPlugin::from_lookup(|name| {
            (name == "AZURE_BLOB_URL")
                .then(|| "https://account.blob.core.windows.net/container".to_owned())
        })
        .unwrap_err();
        assert!(matches!(error, AzureUploadError::InvalidUrl(_)));
    }

    #[test]
    fn debug_names_the_blob_and_not_the_sas_token() {
        let plugin =
            AzureLoggingPlugin::for_store(Arc::new(InMemory::new()), "logs/a.txt".to_owned());
        let rendered = format!("{plugin:?}");
        assert!(rendered.contains("logs/a.txt"));
        assert!(!rendered.contains("sig="));
    }

    #[tokio::test]
    async fn upload_writes_the_file_to_an_in_memory_store() {
        let store = Arc::new(InMemory::new());
        let plugin = AzureLoggingPlugin::for_store(
            Arc::clone(&store) as Arc<dyn ObjectStore>,
            "logs/a.txt".to_owned(),
        );
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("a.txt");
        std::fs::write(&path, b"hello").expect("write");
        plugin.upload(&path).await.expect("upload");
        let got = store
            .get(&ObjectPath::from("logs/a.txt"))
            .await
            .expect("get")
            .bytes()
            .await
            .expect("body");
        assert_eq!(&got[..], b"hello");
    }

    #[test]
    fn a_blob_url_that_is_not_a_url_keeps_the_parser_error() {
        let error = AzureLoggingPlugin::from_lookup(|name| {
            (name == "AZURE_BLOB_URL").then(|| "not a url".to_owned())
        })
        .unwrap_err();
        assert!(error.to_string().starts_with("invalid blob URL:"));
        assert!(std::error::Error::source(&error).is_some());
    }

    #[test]
    fn an_upload_failure_keeps_the_store_error() {
        let error =
            AzureUploadError::upload(std::io::Error::new(std::io::ErrorKind::Other, "full"));
        assert_eq!(error.to_string(), "upload failed: full");
        let source = std::error::Error::source(&error).expect("the store error");
        assert_eq!(source.to_string(), "full");
    }
}
