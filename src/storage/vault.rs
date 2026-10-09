//! Browser profiles that are encrypted before they reach the disk.
//!
//! A profile holds cookies, saved logins and proxy passwords, so a plain
//! `profiles.json` is a file anyone who can read the disk can read. A
//! [`ProfileVault`] keeps the same documents in an embedded Turso database, but
//! every document is sealed with AES-256-GCM under a key derived from a
//! passphrase, so the file alone reveals nothing but the ids and sizes of what
//! it holds.
//!
//! The protection is against the *file* being read: a stolen laptop, a backup,
//! a synced folder. It does not help against code running as the user while the
//! vault is open, because the key has to be in memory to be used.
//!
//! # Examples
//!
//! ```
//! use serde::{Deserialize, Serialize};
//! use seleniumbase_rs::storage::ProfileVault;
//!
//! #[derive(Serialize, Deserialize, PartialEq, Debug)]
//! struct Profile { name: String, proxy: String }
//!
//! # async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! let vault = ProfileVault::open("profiles.vault", "correct horse battery staple").await?;
//! let profile = Profile { name: "EU shop".into(), proxy: "http://user:pass@eu:8080".into() };
//!
//! vault.put("profiles", "p1", &profile).await?;
//!
//! let back: Option<Profile> = vault.get("profiles", "p1").await?;
//! assert_eq!(back, Some(profile));
//! # Ok(())
//! # }
//! ```

use std::fmt;
use std::path::Path;

use base64::Engine;
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
use ring::rand::{SecureRandom, SystemRandom};
use serde::de::DeserializeOwned;
use serde::Serialize;
use turso::{Connection, Database};

use super::db;
use crate::common::encryption::{derive_key, PBKDF2_ITERATIONS, SALT_LEN};
use crate::error::{Result, SeleniumBaseError};

const KIND: &str = "vault";
const SCHEMA_VERSION: i64 = 1;
const SCHEMA: &str = "
CREATE TABLE docs (
    collection TEXT NOT NULL,
    id TEXT NOT NULL,
    body BLOB NOT NULL,
    updated_ms INTEGER NOT NULL,
    PRIMARY KEY (collection, id)
);
";

/// What the vault seals to prove a passphrase is the right one.
const CHECK_PLAINTEXT: &[u8] = b"seleniumbase-rs profile vault";

/// Nonce bytes that start every sealed document.
const NONCE_LEN: usize = 12;

/// The longest collection or id accepted, in bytes.
const MAX_NAME_LEN: usize = 256;

/// A 256-bit key that never prints itself.
struct VaultKey([u8; 32]);

impl fmt::Debug for VaultKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("VaultKey(..)")
    }
}

/// An encrypted document store for browser profiles.
///
/// Documents live in named collections (`profiles`, `tags`, ...) under string
/// ids. Share a vault between tasks by wrapping it in an `Arc`.
#[derive(Debug)]
pub struct ProfileVault {
    conn: Connection,
    // Keeps the database open for as long as the vault lives.
    _db: Database,
    key: VaultKey,
    iterations: u32,
}

impl ProfileVault {
    /// Opens the vault at `path`, creating it with `passphrase` if it does not
    /// exist.
    ///
    /// Deriving the key takes a fraction of a second on purpose: it is what
    /// makes guessing a passphrase slow.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] for an empty passphrase,
    /// [`SeleniumBaseError::Authentication`] if the passphrase does not open an
    /// existing vault, and [`SeleniumBaseError::Database`] if the file is not a
    /// vault or the database fails.
    pub async fn open(path: impl AsRef<Path>, passphrase: &str) -> Result<Self> {
        Self::on(
            db::open_file(path.as_ref()).await?,
            passphrase,
            PBKDF2_ITERATIONS,
        )
        .await
    }

    /// A vault that is discarded when dropped.
    ///
    /// # Errors
    ///
    /// As [`open`](Self::open), except that it never finds an existing vault.
    pub async fn in_memory(passphrase: &str) -> Result<Self> {
        Self::on(db::open_memory().await?, passphrase, PBKDF2_ITERATIONS).await
    }

    async fn on(database: Database, passphrase: &str, new_iterations: u32) -> Result<Self> {
        if passphrase.is_empty() {
            return Err(SeleniumBaseError::InvalidConfig(
                "the vault passphrase must not be empty".to_owned(),
            ));
        }
        let conn = database.connect()?;
        db::prepare(&conn, KIND, SCHEMA_VERSION, SCHEMA).await?;

        let (salt, iterations, check) = match read_header(&conn).await? {
            Some(header) => header,
            None => {
                let salt = random_salt()?;
                write_header(&conn, &salt, new_iterations).await?;
                (salt.to_vec(), new_iterations, None)
            }
        };
        let key = VaultKey(derive_key(passphrase, &salt, iterations)?);
        match check {
            Some(sealed) => {
                let opened = open_sealed(&key, CHECK_AAD, &sealed).map_err(|_| {
                    SeleniumBaseError::Authentication(
                        "that passphrase does not open this vault".to_owned(),
                    )
                })?;
                if opened != CHECK_PLAINTEXT {
                    return Err(SeleniumBaseError::Authentication(
                        "that passphrase does not open this vault".to_owned(),
                    ));
                }
            }
            None => {
                let sealed = seal(&key, CHECK_AAD, CHECK_PLAINTEXT)?;
                conn.execute(
                    "INSERT INTO meta (key, value) VALUES ('check', ?1)",
                    [b64(&sealed)],
                )
                .await?;
            }
        }
        Ok(Self {
            conn,
            _db: database,
            key,
            iterations,
        })
    }

    /// Stores `value` under `id` in `collection`, replacing what was there.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] for an empty or oversized
    /// collection or id, [`SeleniumBaseError::Json`] if `value` cannot be
    /// serialised, and [`SeleniumBaseError::Database`] if it cannot be stored.
    pub async fn put<T: Serialize>(&self, collection: &str, id: &str, value: &T) -> Result<()> {
        check_name("collection", collection)?;
        check_name("id", id)?;
        let json = serde_json::to_vec(value)?;
        let sealed = seal(&self.key, &doc_aad(collection, id), &json)?;
        self.conn
            .execute(
                "INSERT INTO docs (collection, id, body, updated_ms) VALUES (?1, ?2, ?3, ?4) \
                 ON CONFLICT (collection, id) DO UPDATE SET \
                 body = excluded.body, updated_ms = excluded.updated_ms",
                (
                    collection,
                    id,
                    sealed,
                    chrono::Utc::now().timestamp_millis(),
                ),
            )
            .await?;
        Ok(())
    }

    /// The document stored under `id` in `collection`, or `None`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Database`] if the stored bytes fail their
    /// integrity check (they were modified, or moved from another id) or no
    /// longer deserialise as `T`.
    pub async fn get<T: DeserializeOwned>(&self, collection: &str, id: &str) -> Result<Option<T>> {
        let mut rows = self
            .conn
            .query(
                "SELECT body FROM docs WHERE collection = ?1 AND id = ?2",
                [collection, id],
            )
            .await?;
        match rows.next().await? {
            Some(row) => Ok(Some(self.decode(collection, id, &blob(&row, 0)?)?)),
            None => Ok(None),
        }
    }

    /// Every document in `collection` with its id, in the order first stored.
    ///
    /// # Errors
    ///
    /// As [`get`](Self::get), for the first document that fails.
    pub async fn list<T: DeserializeOwned>(&self, collection: &str) -> Result<Vec<(String, T)>> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, body FROM docs WHERE collection = ?1 ORDER BY rowid",
                [collection],
            )
            .await?;
        let mut docs = Vec::new();
        while let Some(row) = rows.next().await? {
            let id = db::text(&row, 0)?;
            let value = self.decode(collection, &id, &blob(&row, 1)?)?;
            docs.push((id, value));
        }
        Ok(docs)
    }

    /// Removes the document under `id`; returns whether there was one.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Database`] if the delete fails.
    pub async fn delete(&self, collection: &str, id: &str) -> Result<bool> {
        let removed = self
            .conn
            .execute(
                "DELETE FROM docs WHERE collection = ?1 AND id = ?2",
                [collection, id],
            )
            .await?;
        Ok(removed > 0)
    }

    /// Re-encrypts every document under `new_passphrase`.
    ///
    /// All documents are rewritten in one transaction, so a failure part-way
    /// leaves the vault readable with the old passphrase. A fresh salt is used,
    /// so the new key shares nothing with the old one.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] for an empty passphrase and
    /// [`SeleniumBaseError::Database`] if a document fails its integrity check
    /// or the rewrite fails.
    pub async fn change_passphrase(&mut self, new_passphrase: &str) -> Result<()> {
        if new_passphrase.is_empty() {
            return Err(SeleniumBaseError::InvalidConfig(
                "the vault passphrase must not be empty".to_owned(),
            ));
        }
        let mut docs = Vec::new();
        let mut rows = self
            .conn
            .query("SELECT collection, id, body FROM docs", ())
            .await?;
        while let Some(row) = rows.next().await? {
            let (collection, id) = (db::text(&row, 0)?, db::text(&row, 1)?);
            let plain = open_sealed(&self.key, &doc_aad(&collection, &id), &blob(&row, 2)?)
                .map_err(|_| integrity_error(&collection, &id))?;
            docs.push((collection, id, plain));
        }
        drop(rows);

        let salt = random_salt()?;
        let key = VaultKey(derive_key(new_passphrase, &salt, self.iterations)?);
        let tx = self.conn.unchecked_transaction().await?;
        for (collection, id, plain) in &docs {
            let sealed = seal(&key, &doc_aad(collection, id), plain)?;
            tx.execute(
                "UPDATE docs SET body = ?1 WHERE collection = ?2 AND id = ?3",
                (sealed, collection.as_str(), id.as_str()),
            )
            .await?;
        }
        tx.execute(
            "UPDATE meta SET value = ?1 WHERE key = 'salt'",
            [b64(&salt)],
        )
        .await?;
        tx.execute(
            "UPDATE meta SET value = ?1 WHERE key = 'check'",
            [b64(&seal(&key, CHECK_AAD, CHECK_PLAINTEXT)?)],
        )
        .await?;
        tx.commit().await?;
        self.key = key;
        Ok(())
    }

    fn decode<T: DeserializeOwned>(&self, collection: &str, id: &str, sealed: &[u8]) -> Result<T> {
        let plain = open_sealed(&self.key, &doc_aad(collection, id), sealed)
            .map_err(|_| integrity_error(collection, id))?;
        serde_json::from_slice(&plain).map_err(|error| {
            SeleniumBaseError::Database(format!(
                "{collection}/{id} is not the expected document: {error}"
            ))
        })
    }
}

const CHECK_AAD: &[u8] = b"sbvault1:check";

/// What a document is bound to, so a sealed body copied to another id, or into
/// another collection, fails to open.
fn doc_aad(collection: &str, id: &str) -> Vec<u8> {
    let mut aad = b"sbvault1:doc:".to_vec();
    aad.extend_from_slice(collection.as_bytes());
    aad.push(0);
    aad.extend_from_slice(id.as_bytes());
    aad
}

fn check_name(what: &str, name: &str) -> Result<()> {
    if name.is_empty() || name.len() > MAX_NAME_LEN {
        return Err(SeleniumBaseError::InvalidConfig(format!(
            "a vault {what} must be 1 to {MAX_NAME_LEN} bytes, not {}",
            name.len()
        )));
    }
    Ok(())
}

fn integrity_error(collection: &str, id: &str) -> SeleniumBaseError {
    SeleniumBaseError::Database(format!(
        "{collection}/{id} failed its integrity check: it was modified, or does not belong here"
    ))
}

fn random_salt() -> Result<[u8; SALT_LEN]> {
    let mut salt = [0_u8; SALT_LEN];
    SystemRandom::new().fill(&mut salt).map_err(|_| {
        SeleniumBaseError::Unsupported("the system random number generator failed".to_owned())
    })?;
    Ok(salt)
}

/// Seals `plaintext`: a fresh random nonce, then the ciphertext and its tag.
fn seal(key: &VaultKey, aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
    let sealing = aead_key(key)?;
    let mut nonce = [0_u8; NONCE_LEN];
    SystemRandom::new().fill(&mut nonce).map_err(|_| {
        SeleniumBaseError::Unsupported("the system random number generator failed".to_owned())
    })?;
    let mut sealed = plaintext.to_vec();
    sealing
        .seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(aad),
            &mut sealed,
        )
        .map_err(|_| SeleniumBaseError::Unsupported("encryption failed".to_owned()))?;
    let mut out = nonce.to_vec();
    out.append(&mut sealed);
    Ok(out)
}

/// Opens what [`seal`] produced; fails if the key, the binding or any byte is
/// wrong.
fn open_sealed(key: &VaultKey, aad: &[u8], sealed: &[u8]) -> Result<Vec<u8>> {
    let opening = aead_key(key)?;
    let (nonce, body) = sealed
        .split_at_checked(NONCE_LEN)
        .ok_or_else(|| SeleniumBaseError::Unsupported("sealed data is too short".to_owned()))?;
    let nonce: [u8; NONCE_LEN] = nonce
        .try_into()
        .ok()
        .ok_or_else(|| SeleniumBaseError::Unsupported("sealed data is too short".to_owned()))?;
    let mut buffer = body.to_vec();
    let plain = opening
        .open_in_place(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(aad),
            &mut buffer,
        )
        .map_err(|_| SeleniumBaseError::Unsupported("decryption failed".to_owned()))?;
    Ok(plain.to_vec())
}

fn aead_key(key: &VaultKey) -> Result<LessSafeKey> {
    UnboundKey::new(&AES_256_GCM, &key.0)
        .map(LessSafeKey::new)
        .map_err(|_| SeleniumBaseError::Unsupported("the vault key is unusable".to_owned()))
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn unb64(text: &str) -> Result<Vec<u8>> {
    base64::engine::general_purpose::STANDARD
        .decode(text)
        .map_err(|_| SeleniumBaseError::Database("the vault header is damaged".to_owned()))
}

fn blob(row: &turso::Row, idx: usize) -> Result<Vec<u8>> {
    match row.get_value(idx)? {
        turso::Value::Blob(bytes) => Ok(bytes),
        _ => Err(SeleniumBaseError::Database(format!(
            "column {idx} should be a blob"
        ))),
    }
}

type Header = (Vec<u8>, u32, Option<Vec<u8>>);

/// Reads the salt, work factor and passphrase check of an existing vault.
async fn read_header(conn: &Connection) -> Result<Option<Header>> {
    let mut rows = conn
        .query(
            "SELECT key, value FROM meta WHERE key IN ('salt', 'iterations', 'check')",
            (),
        )
        .await?;
    let (mut salt, mut iterations, mut check) = (None, None, None);
    while let Some(row) = rows.next().await? {
        let value = db::text(&row, 1)?;
        match db::text(&row, 0)?.as_str() {
            "salt" => salt = Some(unb64(&value)?),
            "iterations" => iterations = value.parse::<u32>().ok(),
            "check" => check = Some(unb64(&value)?),
            _ => {}
        }
    }
    match (salt, iterations) {
        (Some(salt), Some(iterations)) => {
            if !(1..=10_000_000).contains(&iterations) {
                return Err(SeleniumBaseError::Database(
                    "the vault's work factor is out of range".to_owned(),
                ));
            }
            Ok(Some((salt, iterations, check)))
        }
        (None, None) => Ok(None),
        _ => Err(SeleniumBaseError::Database(
            "the vault header is incomplete".to_owned(),
        )),
    }
}

async fn write_header(conn: &Connection, salt: &[u8], iterations: u32) -> Result<()> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES ('salt', ?1)",
        [b64(salt)],
    )
    .await?;
    conn.execute(
        "INSERT INTO meta (key, value) VALUES ('iterations', ?1)",
        [iterations.to_string()],
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    /// Few enough PBKDF2 rounds that tests do not wait on the key derivation.
    const FAST: u32 = 1_000;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct Profile {
        name: String,
        proxy: String,
    }

    fn profile(name: &str) -> Profile {
        Profile {
            name: name.to_owned(),
            proxy: "http://alice:s3cret@proxy.example.com:8080".to_owned(),
        }
    }

    async fn vault(passphrase: &str) -> ProfileVault {
        ProfileVault::on(db::open_memory().await.unwrap(), passphrase, FAST)
            .await
            .unwrap()
    }

    async fn on_disk(path: &Path, passphrase: &str) -> Result<ProfileVault> {
        ProfileVault::on(db::open_file(path).await.unwrap(), passphrase, FAST).await
    }

    #[tokio::test]
    async fn a_document_round_trips() {
        let vault = vault("pw").await;
        vault
            .put("profiles", "p1", &profile("EU shop"))
            .await
            .unwrap();

        let back: Option<Profile> = vault.get("profiles", "p1").await.unwrap();

        assert_eq!(back, Some(profile("EU shop")));
    }

    #[tokio::test]
    async fn a_missing_document_is_none_not_an_error() {
        let vault = vault("pw").await;

        assert_eq!(
            vault.get::<Profile>("profiles", "nope").await.unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn putting_again_replaces_and_keeps_the_original_position() {
        let vault = vault("pw").await;
        for id in ["a", "b", "c"] {
            vault.put("profiles", id, &profile(id)).await.unwrap();
        }
        vault
            .put("profiles", "a", &profile("a, renamed"))
            .await
            .unwrap();

        let all: Vec<(String, Profile)> = vault.list("profiles").await.unwrap();

        assert_eq!(
            all.iter()
                .map(|(id, p)| (id.as_str(), p.name.as_str()))
                .collect::<Vec<_>>(),
            [("a", "a, renamed"), ("b", "b"), ("c", "c")]
        );
    }

    #[tokio::test]
    async fn collections_do_not_mix() {
        let vault = vault("pw").await;
        vault
            .put("profiles", "x", &profile("a profile"))
            .await
            .unwrap();
        vault.put("tags", "x", &"a tag").await.unwrap();

        assert_eq!(vault.list::<Profile>("profiles").await.unwrap().len(), 1);
        assert_eq!(
            vault.get::<String>("tags", "x").await.unwrap().as_deref(),
            Some("a tag")
        );
    }

    #[tokio::test]
    async fn delete_reports_whether_something_was_removed() {
        let vault = vault("pw").await;
        vault.put("profiles", "p", &profile("p")).await.unwrap();

        assert!(vault.delete("profiles", "p").await.unwrap());
        assert!(!vault.delete("profiles", "p").await.unwrap());
        assert!(vault.list::<Profile>("profiles").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn empty_and_oversized_names_are_refused() {
        let vault = vault("pw").await;
        let long = "x".repeat(MAX_NAME_LEN + 1);

        for (collection, id) in [
            ("", "id"),
            ("c", ""),
            (long.as_str(), "id"),
            ("c", long.as_str()),
        ] {
            let error = vault.put(collection, id, &1).await.unwrap_err();
            assert!(
                matches!(error, SeleniumBaseError::InvalidConfig(_)),
                "{error}"
            );
        }
    }

    #[tokio::test]
    async fn an_empty_passphrase_is_refused() {
        let error = ProfileVault::on(db::open_memory().await.unwrap(), "", FAST)
            .await
            .unwrap_err();

        assert!(
            matches!(error, SeleniumBaseError::InvalidConfig(_)),
            "{error}"
        );
    }

    #[tokio::test]
    async fn the_file_on_disk_contains_no_plaintext() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v.db");
        {
            let vault = on_disk(&path, "pw").await.unwrap();
            vault
                .put("profiles", "p1", &profile("Very Recognisable Name"))
                .await
                .unwrap();
            // A control: text stored unencrypted must be findable, or the scan
            // below could not tell a sealed vault from one never flushed to disk.
            vault
                .conn
                .execute("CREATE TABLE control (v TEXT)", ())
                .await
                .unwrap();
            vault
                .conn
                .execute(
                    "INSERT INTO control (v) VALUES ('PLAINTEXT-CONTROL-MARKER')",
                    (),
                )
                .await
                .unwrap();
        }

        let mut bytes = Vec::new();
        for entry in std::fs::read_dir(dir.path()).unwrap() {
            bytes.extend(std::fs::read(entry.unwrap().path()).unwrap());
        }
        let contains = |needle: &str| bytes.windows(needle.len()).any(|w| w == needle.as_bytes());

        assert!(
            contains("PLAINTEXT-CONTROL-MARKER"),
            "the scan must see unencrypted data, or it proves nothing"
        );
        for needle in ["Very Recognisable Name", "s3cret", "proxy.example.com"] {
            assert!(
                !contains(needle),
                "{needle:?} is readable in the vault files"
            );
        }
    }

    #[tokio::test]
    async fn the_right_passphrase_reopens_and_the_wrong_one_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v.db");
        {
            let vault = on_disk(&path, "right").await.unwrap();
            vault.put("profiles", "p", &profile("kept")).await.unwrap();
        }

        let reopened = on_disk(&path, "right").await.unwrap();
        let back: Profile = reopened.get("profiles", "p").await.unwrap().unwrap();
        assert_eq!(back.name, "kept");
        drop(reopened);

        let error = on_disk(&path, "wrong").await.unwrap_err();
        assert!(
            matches!(error, SeleniumBaseError::Authentication(_)),
            "{error}"
        );
        assert!(
            !error.to_string().contains("wrong"),
            "the guess is not echoed"
        );
    }

    #[tokio::test]
    async fn a_document_moved_to_another_id_or_collection_is_rejected() {
        let vault = vault("pw").await;
        vault
            .put("profiles", "victim", &profile("victim"))
            .await
            .unwrap();
        vault
            .put("profiles", "attacker", &profile("attacker"))
            .await
            .unwrap();
        // Someone with write access to the file copies one sealed body over another.
        vault
            .conn
            .execute(
                "UPDATE docs SET body = (SELECT body FROM docs WHERE id = 'attacker') \
                 WHERE id = 'victim'",
                (),
            )
            .await
            .unwrap();
        vault
            .conn
            .execute(
                "UPDATE docs SET collection = 'tags' WHERE id = 'attacker'",
                (),
            )
            .await
            .unwrap();

        let swapped = vault
            .get::<Profile>("profiles", "victim")
            .await
            .unwrap_err();
        let moved = vault.get::<Profile>("tags", "attacker").await.unwrap_err();

        assert!(swapped.to_string().contains("integrity check"), "{swapped}");
        assert!(moved.to_string().contains("integrity check"), "{moved}");
    }

    #[tokio::test]
    async fn a_modified_body_is_rejected() {
        let vault = vault("pw").await;
        vault.put("profiles", "p", &profile("p")).await.unwrap();
        let mut rows = vault
            .conn
            .query("SELECT body FROM docs WHERE id = 'p'", ())
            .await
            .unwrap();
        let mut body = blob(&rows.next().await.unwrap().unwrap(), 0).unwrap();
        drop(rows);
        let last = body.len() - 1;
        body[last] ^= 0x01;
        vault
            .conn
            .execute("UPDATE docs SET body = ?1 WHERE id = 'p'", [body])
            .await
            .unwrap();

        let error = vault.get::<Profile>("profiles", "p").await.unwrap_err();

        assert!(error.to_string().contains("integrity check"), "{error}");
    }

    #[tokio::test]
    async fn sealing_the_same_document_twice_gives_different_bytes() {
        let key = VaultKey([7; 32]);

        let a = seal(&key, b"aad", b"same").unwrap();
        let b = seal(&key, b"aad", b"same").unwrap();

        assert_ne!(a, b, "a nonce is never reused");
        assert_eq!(open_sealed(&key, b"aad", &a).unwrap(), b"same");
        assert!(open_sealed(&key, b"other", &a).is_err());
        assert!(open_sealed(&VaultKey([8; 32]), b"aad", &a).is_err());
        assert!(open_sealed(&key, b"aad", &a[..NONCE_LEN]).is_err());
        assert!(open_sealed(&key, b"aad", &[]).is_err());
    }

    #[tokio::test]
    async fn changing_the_passphrase_keeps_every_document() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v.db");
        {
            let mut vault = on_disk(&path, "old").await.unwrap();
            for id in ["a", "b"] {
                vault.put("profiles", id, &profile(id)).await.unwrap();
            }
            vault.put("tags", "t", &"work").await.unwrap();
            vault.change_passphrase("new").await.unwrap();
            // The open handle keeps working under the new key.
            assert_eq!(vault.list::<Profile>("profiles").await.unwrap().len(), 2);
        }

        assert!(matches!(
            on_disk(&path, "old").await.unwrap_err(),
            SeleniumBaseError::Authentication(_)
        ));
        let reopened = on_disk(&path, "new").await.unwrap();
        assert_eq!(reopened.list::<Profile>("profiles").await.unwrap().len(), 2);
        assert_eq!(
            reopened
                .get::<String>("tags", "t")
                .await
                .unwrap()
                .as_deref(),
            Some("work")
        );
    }

    #[tokio::test]
    async fn a_failed_passphrase_change_leaves_the_old_one_working() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v.db");
        {
            let mut vault = on_disk(&path, "old").await.unwrap();
            vault.put("profiles", "a", &profile("a")).await.unwrap();
            // A corrupt document makes the re-encryption refuse to start.
            vault
                .conn
                .execute("UPDATE docs SET body = x'00' WHERE id = 'a'", ())
                .await
                .unwrap();
            assert!(vault.change_passphrase("new").await.is_err());
            assert!(vault.change_passphrase("").await.is_err());
        }

        assert!(
            on_disk(&path, "old").await.is_ok(),
            "the old passphrase still opens it"
        );
        assert!(on_disk(&path, "new").await.is_err());
    }

    #[tokio::test]
    async fn a_results_database_is_not_a_vault() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("r.db");
        super::super::ResultStore::open(&path).await.unwrap();

        let error = on_disk(&path, "pw").await.unwrap_err();

        assert!(error.to_string().contains("not a vault"), "{error}");
    }

    #[tokio::test]
    async fn debug_output_never_shows_the_key() {
        let vault = vault("pw").await;

        let shown = format!("{vault:?}");

        assert!(shown.contains("VaultKey(..)"), "{shown}");
        assert!(!shown.contains("pw"));
    }
}
