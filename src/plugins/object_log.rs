//! Bytes written through [`object_store`](https://docs.rs/object_store/0.14.2).
//!
//! The S3 and Azure plugins build a cloud store and call [`put_bytes`]. Tests use
//! the in-memory store and a local directory, which implement the same
//! [`ObjectStore`](object_store::ObjectStore) trait.

use bytes::Bytes;
use object_store::path::Path as ObjectPath;
use object_store::{ObjectStore, ObjectStoreExt, PutPayload};

/// Writes `bytes` at `key`.
///
/// `key` is an object path (`logs/run.txt`), not a URL. A leading `/` is dropped
/// by [`ObjectPath`].
///
/// # Errors
///
/// Returns the store error when the write fails.
pub async fn put_bytes(
    store: &dyn ObjectStore,
    key: &str,
    bytes: impl Into<Bytes>,
) -> Result<(), object_store::Error> {
    let path = ObjectPath::from(key);
    store.put(&path, PutPayload::from(bytes.into())).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use object_store::memory::InMemory;

    #[tokio::test]
    async fn in_memory_round_trip() {
        let store = InMemory::new();
        put_bytes(&store, "logs/a.txt", Bytes::from_static(b"hello"))
            .await
            .expect("put");
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
    async fn local_directory_round_trip() {
        let dir = tempfile::tempdir().expect("temp dir");
        let store =
            object_store::local::LocalFileSystem::new_with_prefix(dir.path()).expect("local store");
        put_bytes(&store, "shot.png", Bytes::from_static(b"png"))
            .await
            .expect("put");
        let got = store
            .get(&ObjectPath::from("shot.png"))
            .await
            .expect("get")
            .bytes()
            .await
            .expect("body");
        assert_eq!(&got[..], b"png");
    }
}
