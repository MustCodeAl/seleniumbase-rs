//! S3 logging plugin.
//!
//! Uploads test artifacts through [`object_store::aws::AmazonS3Builder`]
//! (object_store 0.14.2, feature `aws`). Docs: <https://docs.rs/object_store/0.14.2/object_store/aws/struct.AmazonS3Builder.html>.
//!
//! [`from_env`](S3LoggingPlugin::from_env) reads the account.
//! [`from_lookup`](S3LoggingPlugin::from_lookup) takes the same names from a
//! closure, so a test does not change the process environment.

use std::fmt;
use std::path::Path;
use std::sync::Arc;

use object_store::aws::AmazonS3Builder;
use object_store::ObjectStore;
use thiserror::Error;
use tokio::fs;

use super::object_log;

/// Errors that can occur during an S3 upload.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum S3UploadError {
    /// A required variable was missing or blank.
    #[error("missing environment variable {0}")]
    MissingEnv(&'static str),
    /// The credentials were rejected.
    #[error("invalid credentials: {0}")]
    Credentials(String),
    /// The region was rejected.
    #[error("invalid region: {0}")]
    Region(String),
    /// The store could not be built. The builder error is
    /// [`Error::source`](std::error::Error::source).
    #[error("failed to create bucket handle: {message}")]
    Bucket {
        /// What the builder reported.
        message: String,
        /// The builder error.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
    /// The file could not be read.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// The store rejected the write. The store error is
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

impl S3UploadError {
    fn bucket(error: impl std::error::Error + Send + Sync + 'static) -> Self {
        let message = error.to_string();
        Self::Bucket {
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

/// Uploads test artifacts, such as the files the failure plugins save, to one S3 bucket.
///
/// It has no hooks of its own: build it with [`from_env`](Self::from_env) or
/// [`from_lookup`](Self::from_lookup), then call [`upload`](Self::upload) after a
/// test, for example on the screenshot a
/// [`ScreenshotOnFailurePlugin`](crate::plugins::screen_shots::ScreenshotOnFailurePlugin)
/// wrote.
///
/// `Debug` names the type and not the access key or the secret.
#[non_exhaustive]
pub struct S3LoggingPlugin {
    store: Arc<dyn ObjectStore>,
}

impl fmt::Debug for S3LoggingPlugin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("S3LoggingPlugin")
            .finish_non_exhaustive()
    }
}

impl S3LoggingPlugin {
    /// The bucket `SB_S3_BUCKET` names, in `SB_S3_REGION` or else `AWS_REGION`
    /// or else `us-east-1`.
    ///
    /// Credentials are `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY`.
    /// `AWS_ENDPOINT_URL`, when set, selects an S3-compatible store.
    ///
    /// # Errors
    ///
    /// Returns [`S3UploadError::MissingEnv`] when the bucket or a credential is
    /// absent, and [`S3UploadError::Bucket`] when the store cannot be built.
    pub fn from_env() -> Result<Self, S3UploadError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// The same account as [`from_env`](Self::from_env), read through `lookup`.
    ///
    /// # Errors
    ///
    /// As [`from_env`](Self::from_env).
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, S3UploadError> {
        let bucket = required(&lookup, "SB_S3_BUCKET")?;
        let region = present(&lookup, "SB_S3_REGION")
            .or_else(|| present(&lookup, "AWS_REGION"))
            .unwrap_or_else(|| "us-east-1".to_owned());
        let access_key = required(&lookup, "AWS_ACCESS_KEY_ID")?;
        let secret_key = required(&lookup, "AWS_SECRET_ACCESS_KEY")?;
        let endpoint = present(&lookup, "AWS_ENDPOINT_URL");
        Self::connect(bucket, region, access_key, secret_key, endpoint)
    }

    /// Uploads the file at `path` as `key`.
    ///
    /// # Errors
    ///
    /// Returns [`S3UploadError::Io`] when the file cannot be read, and
    /// [`S3UploadError::Upload`] when the store rejects the write.
    pub async fn upload(&self, path: impl AsRef<Path>, key: &str) -> Result<(), S3UploadError> {
        let bytes = fs::read(path.as_ref()).await?;
        object_log::put_bytes(self.store.as_ref(), key, bytes)
            .await
            .map_err(S3UploadError::upload)?;
        Ok(())
    }

    fn connect(
        bucket: String,
        region: String,
        access_key: String,
        secret_key: String,
        endpoint: Option<String>,
    ) -> Result<Self, S3UploadError> {
        let mut builder = AmazonS3Builder::new()
            .with_bucket_name(bucket)
            .with_region(region)
            .with_access_key_id(access_key)
            .with_secret_access_key(secret_key);
        if let Some(endpoint) = endpoint {
            if endpoint.starts_with("http://") {
                builder = builder.with_allow_http(true);
            }
            builder = builder.with_endpoint(endpoint);
        }
        let store = builder.build().map_err(S3UploadError::bucket)?;
        Ok(Self {
            store: Arc::new(store),
        })
    }

    #[cfg(test)]
    fn for_store(store: Arc<dyn ObjectStore>) -> Self {
        Self { store }
    }
}

fn present(lookup: &impl Fn(&str) -> Option<String>, name: &str) -> Option<String> {
    lookup(name).filter(|value| !value.trim().is_empty())
}

fn required(
    lookup: &impl Fn(&str) -> Option<String>,
    name: &'static str,
) -> Result<String, S3UploadError> {
    present(lookup, name).ok_or(S3UploadError::MissingEnv(name))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use object_store::local::LocalFileSystem;
    use object_store::memory::InMemory;
    use object_store::path::Path as ObjectPath;
    use object_store::ObjectStoreExt;

    use super::*;

    #[test]
    fn from_lookup_reports_a_missing_bucket_without_reading_the_process() {
        let error = S3LoggingPlugin::from_lookup(|_| None).unwrap_err();
        assert!(matches!(error, S3UploadError::MissingEnv("SB_S3_BUCKET")));
    }

    #[test]
    fn debug_does_not_include_the_secret() {
        let plugin = S3LoggingPlugin::from_lookup(|name| match name {
            "SB_S3_BUCKET" => Some("logs".to_owned()),
            "AWS_ACCESS_KEY_ID" => Some("key-id".to_owned()),
            "AWS_SECRET_ACCESS_KEY" => Some("super-secret".to_owned()),
            _ => None,
        })
        .expect("the client is built locally");
        let rendered = format!("{plugin:?}");
        assert!(!rendered.contains("super-secret"));
        assert!(!rendered.contains("key-id"));
    }

    #[tokio::test]
    async fn upload_writes_the_file_to_an_in_memory_store() {
        let store = Arc::new(InMemory::new());
        let plugin = S3LoggingPlugin::for_store(Arc::clone(&store) as Arc<dyn ObjectStore>);
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("a.txt");
        std::fs::write(&path, b"hello").expect("write");
        plugin.upload(&path, "logs/a.txt").await.expect("upload");
        let got = store
            .get(&ObjectPath::from("logs/a.txt"))
            .await
            .expect("get")
            .bytes()
            .await
            .expect("body");
        assert_eq!(&got[..], b"hello");
    }

    #[tokio::test]
    async fn upload_writes_the_file_to_a_local_directory() {
        let store_dir = tempfile::tempdir().expect("store dir");
        let source_dir = tempfile::tempdir().expect("source dir");
        let store = LocalFileSystem::new_with_prefix(store_dir.path()).expect("local store");
        let plugin = S3LoggingPlugin::for_store(Arc::new(store));
        let file = source_dir.path().join("shot.png");
        std::fs::write(&file, b"png").expect("write");
        plugin.upload(&file, "shot.png").await.expect("upload");
        let saved = std::fs::read(store_dir.path().join("shot.png")).expect("read back");
        assert_eq!(saved, b"png");
    }

    #[test]
    fn an_upload_failure_keeps_the_store_error() {
        let error = S3UploadError::upload(std::io::Error::new(std::io::ErrorKind::Other, "full"));
        assert_eq!(error.to_string(), "upload failed: full");
        let source = std::error::Error::source(&error).expect("the store error");
        assert_eq!(source.to_string(), "full");
    }
}
