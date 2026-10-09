//! Where profiles live on disk: an encrypted vault, not a JSON file.
//!
//! Profiles carry cookies and proxy passwords, so they are kept in a
//! [`ProfileVault`] (`profiles.vault` in the app-data directory) with
//! `profiles`, `tags` and `folders` collections. Every document is sealed with
//! AES-256-GCM under a key derived from a passphrase; see
//! [`crate::passphrase`] for where that comes from.
//!
//! Earlier versions wrote `profiles.json`, `tags.json` and `folders.json` in
//! clear text. If any of those exist, [`open_storage`] imports them, reads every
//! document back out of the vault and compares it with what was imported, and
//! only then deletes the files. If anything is off the files stay exactly where
//! they are and the app reports why.
//!
//! The protection is against the *file* being read (a backup, a synced folder, a
//! stolen disk). Code running as the user while the app is open can still reach
//! the profiles, because the key has to be in memory to be used.

use std::collections::BTreeMap;
use std::fmt;
use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ring::digest::{digest, SHA256};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{info, warn};

use seleniumbase_rs::storage::ProfileVault;
use seleniumbase_rs::SeleniumBaseError;

use crate::models::{Folder, Profile, StorageStatus, Tag};
use crate::passphrase::{
    hex, passphrase_from_env, resolve_passphrase, KeyringSecretStore, PassphraseError,
    PassphraseSource, SecretStore,
};

/// The vault file inside the app-data directory.
pub const VAULT_FILE: &str = "profiles.vault";

const PROFILES: &str = "profiles";
const TAGS: &str = "tags";
const FOLDERS: &str = "folders";

/// Bookkeeping for an import that has been verified but not yet cleaned up.
const META: &str = "meta";
const IMPORT_MARKER: &str = "legacy-import";

/// What went wrong with the profile store.
///
/// None of the messages contains a passphrase or a stored value.
#[derive(Debug)]
pub enum StorageError {
    /// No passphrase could be obtained.
    Passphrase(PassphraseError),
    /// The passphrase does not open the existing vault. The vault was not
    /// touched.
    WrongPassphrase,
    /// The vault or the disk failed.
    Vault(String),
    /// Moving the old clear-text files into the vault failed. They were left
    /// where they are.
    Migration(MigrationError),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Passphrase(error) => write!(f, "No passphrase for the profile vault: {error}."),
            Self::WrongPassphrase => f.write_str(
                "The profile vault could not be opened: its passphrase does not match. If you \
                 set SB_PROFILE_PASSPHRASE, use the value the vault was created with; otherwise \
                 restore the system keychain item. The vault was not changed.",
            ),
            Self::Vault(reason) => write!(f, "The profile vault failed: {reason}."),
            Self::Migration(error) => write!(
                f,
                "Your existing profile files could not be moved into the encrypted vault: \
                 {error}. They were left exactly as they were."
            ),
        }
    }
}

impl std::error::Error for StorageError {}

impl From<SeleniumBaseError> for StorageError {
    fn from(error: SeleniumBaseError) -> Self {
        match error {
            SeleniumBaseError::Authentication(_) => Self::WrongPassphrase,
            other => Self::Vault(describe_vault_error(&other)),
        }
    }
}

/// Words a vault error without the value that may have caused it.
///
/// A stored document that no longer deserialises is reported by the vault with
/// the serde message, and serde quotes the offending value ("invalid type:
/// string \"...\"").
fn describe_vault_error(error: &SeleniumBaseError) -> String {
    let text = error.to_string();
    const MARKER: &str = " is not the expected document";
    match text.find(MARKER) {
        Some(at) => text[..at + MARKER.len()].to_owned(),
        None => text,
    }
}

/// Why importing the old clear-text files failed.
#[derive(Debug, PartialEq, Eq)]
pub enum MigrationError {
    /// A file could not be read or is not valid JSON.
    Unreadable {
        /// The file name.
        file: &'static str,
        /// What is wrong, without any of the file's contents.
        reason: String,
    },
    /// An entry of a file is not a profile, tag or folder.
    BadItem {
        /// The file name.
        file: &'static str,
        /// The position of the entry.
        index: usize,
    },
    /// A document read back out of the vault does not match what was imported.
    VerificationFailed {
        /// The collection.
        collection: &'static str,
        /// The document id.
        id: String,
    },
    /// The vault says this file was imported before, but it has changed since.
    /// Nothing is imported or deleted, because the vault may have been edited
    /// since.
    ChangedSinceImport {
        /// The file name.
        file: &'static str,
    },
}

impl fmt::Display for MigrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable { file, reason } => write!(f, "{file} cannot be read ({reason})"),
            Self::BadItem { file, index } => {
                write!(f, "entry {index} of {file} is not valid")
            }
            Self::VerificationFailed { collection, id } => write!(
                f,
                "{collection}/{id} did not read back from the vault the way it was written"
            ),
            Self::ChangedSinceImport { file } => write!(
                f,
                "{file} was imported earlier but has changed since; move it out of the app-data \
                 directory if you no longer need it"
            ),
        }
    }
}

impl std::error::Error for MigrationError {}

impl From<MigrationError> for StorageError {
    fn from(error: MigrationError) -> Self {
        Self::Migration(error)
    }
}

// ----------------------------------------------------------------------
// Documents
// ----------------------------------------------------------------------

/// JSON documents in named collections: the part of a vault this module uses.
///
/// A trait so that a vault that loses data can stand in for the real one in
/// tests of the import.
pub(crate) trait DocStore: Sync {
    fn put_doc(
        &self,
        collection: &str,
        id: &str,
        doc: &Value,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;

    fn get_doc(
        &self,
        collection: &str,
        id: &str,
    ) -> impl Future<Output = Result<Option<Value>, StorageError>> + Send;

    fn list_docs(
        &self,
        collection: &str,
    ) -> impl Future<Output = Result<Vec<(String, Value)>, StorageError>> + Send;

    fn delete_doc(
        &self,
        collection: &str,
        id: &str,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send;
}

impl DocStore for ProfileVault {
    async fn put_doc(&self, collection: &str, id: &str, doc: &Value) -> Result<(), StorageError> {
        Ok(self.put(collection, id, doc).await?)
    }

    async fn get_doc(&self, collection: &str, id: &str) -> Result<Option<Value>, StorageError> {
        Ok(self.get(collection, id).await?)
    }

    async fn list_docs(&self, collection: &str) -> Result<Vec<(String, Value)>, StorageError> {
        Ok(self.list(collection).await?)
    }

    async fn delete_doc(&self, collection: &str, id: &str) -> Result<bool, StorageError> {
        Ok(self.delete(collection, id).await?)
    }
}

async fn put_typed<S: DocStore, T: Serialize>(
    store: &S,
    collection: &str,
    id: &str,
    value: &T,
) -> Result<(), StorageError> {
    let doc = serde_json::to_value(value)
        .map_err(|_| StorageError::Vault("a document could not be serialised".to_owned()))?;
    store.put_doc(collection, id, &doc).await
}

async fn load_typed<S: DocStore, T: DeserializeOwned>(
    store: &S,
    collection: &str,
) -> Result<Vec<T>, StorageError> {
    store
        .list_docs(collection)
        .await?
        .into_iter()
        .map(|(id, doc)| {
            // The serde error is dropped on purpose: it can quote a value.
            serde_json::from_value(doc).map_err(|_| {
                StorageError::Vault(format!("{collection}/{id} is not the expected document"))
            })
        })
        .collect()
}

/// Everything the vault holds.
#[derive(Debug, Default)]
pub struct LoadedData {
    pub profiles: Vec<Profile>,
    pub tags: Vec<Tag>,
    pub folders: Vec<Folder>,
}

/// An open, unlocked vault of profiles, tags and folders.
pub struct ProfileStorage {
    vault: ProfileVault,
}

impl fmt::Debug for ProfileStorage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProfileStorage").finish_non_exhaustive()
    }
}

impl ProfileStorage {
    /// Saves a profile, replacing the stored one with the same id.
    pub async fn put_profile(&self, profile: &Profile) -> Result<(), StorageError> {
        put_typed(&self.vault, PROFILES, &profile.id, profile).await
    }

    /// Removes a profile.
    pub async fn delete_profile(&self, id: &str) -> Result<(), StorageError> {
        self.vault.delete_doc(PROFILES, id).await.map(drop)
    }

    /// Saves a tag.
    pub async fn put_tag(&self, tag: &Tag) -> Result<(), StorageError> {
        put_typed(&self.vault, TAGS, &tag.id, tag).await
    }

    /// Removes a tag.
    pub async fn delete_tag(&self, id: &str) -> Result<(), StorageError> {
        self.vault.delete_doc(TAGS, id).await.map(drop)
    }

    /// Saves a folder.
    pub async fn put_folder(&self, folder: &Folder) -> Result<(), StorageError> {
        put_typed(&self.vault, FOLDERS, &folder.id, folder).await
    }

    /// Removes a folder.
    pub async fn delete_folder(&self, id: &str) -> Result<(), StorageError> {
        self.vault.delete_doc(FOLDERS, id).await.map(drop)
    }

    /// Reads everything. A document that cannot be read is an error, never a
    /// silently shorter list.
    async fn load(&self) -> Result<LoadedData, StorageError> {
        Ok(LoadedData {
            profiles: load_typed(&self.vault, PROFILES).await?,
            tags: load_typed(&self.vault, TAGS).await?,
            folders: load_typed(&self.vault, FOLDERS).await?,
        })
    }
}

// ----------------------------------------------------------------------
// The state the app holds
// ----------------------------------------------------------------------

/// Whether profiles are being kept, and where.
#[derive(Debug, Clone)]
pub enum StorageState {
    /// Nothing is persisted. Only tests use this.
    #[cfg(test)]
    Ephemeral,
    /// Profiles are loaded and every change is saved to the vault.
    Ready {
        storage: Arc<ProfileStorage>,
        source: PassphraseSource,
        /// Something the user should know even though the vault works.
        warning: Option<String>,
    },
    /// The vault could not be opened. No profiles are shown and nothing is
    /// written, so a wrong passphrase can never look like an empty list or
    /// overwrite the vault.
    Unavailable { message: String },
}

impl StorageState {
    /// What the window shows.
    pub fn status(&self) -> StorageStatus {
        match self {
            #[cfg(test)]
            Self::Ephemeral => StorageStatus {
                ok: true,
                error: None,
                warning: None,
                passphrase_source: None,
            },
            Self::Ready {
                source, warning, ..
            } => StorageStatus {
                ok: true,
                error: None,
                warning: warning.clone(),
                passphrase_source: Some(source.label().to_owned()),
            },
            Self::Unavailable { message } => StorageStatus {
                ok: false,
                error: Some(message.clone()),
                warning: None,
                passphrase_source: None,
            },
        }
    }
}

/// The result of opening the store at startup.
#[derive(Debug)]
pub struct Opened {
    pub state: StorageState,
    pub data: LoadedData,
}

/// Opens the vault in `dir`, importing the old clear-text files if there are
/// any.
///
/// Never fails: a vault that cannot be opened becomes
/// [`StorageState::Unavailable`] with an explanation and no data.
pub async fn open_storage(
    dir: &Path,
    env_passphrase: Option<&str>,
    keychain: &dyn SecretStore,
) -> Opened {
    match try_open(dir, env_passphrase, keychain).await {
        Ok(opened) => opened,
        Err(error) => unavailable(&error),
    }
}

/// Opens the vault in `dir` the way the app does: the passphrase comes from
/// `SB_PROFILE_PASSPHRASE` if it is set, otherwise from the system keychain.
pub async fn open_default(dir: &Path) -> Opened {
    match passphrase_from_env() {
        Ok(env) => open_storage(dir, env.as_deref(), &KeyringSecretStore).await,
        Err(error) => unavailable(&StorageError::Passphrase(error)),
    }
}

/// A store that could not be opened, with the reason, and no data.
fn unavailable(error: &StorageError) -> Opened {
    warn!(error = %error, "the profile vault is unavailable");
    Opened {
        state: StorageState::Unavailable {
            message: error.to_string(),
        },
        data: LoadedData::default(),
    }
}

async fn try_open(
    dir: &Path,
    env_passphrase: Option<&str>,
    keychain: &dyn SecretStore,
) -> Result<Opened, StorageError> {
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|error| StorageError::Vault(format!("cannot create the data folder: {error}")))?;

    let vault_path = dir.join(VAULT_FILE);
    // When in doubt, say it exists: that never generates a passphrase.
    let vault_existed = vault_path.try_exists().unwrap_or(true);
    let had_legacy_profiles = dir.join(Kind::Profiles.file_name()).exists();

    let (passphrase, source) =
        resolve_passphrase(env_passphrase, keychain, vault_existed).map_err(StorageError::from)?;
    let vault = ProfileVault::open(&vault_path, passphrase.expose()).await?;

    let report = migrate_legacy(&vault, dir, &|path| std::fs::remove_file(path)).await?;
    let storage = ProfileStorage { vault };
    let mut data = storage.load().await?;

    // A brand-new install starts with two sample profiles, as the JSON store
    // did. Only once: deleting them must not bring them back.
    if !vault_existed && !had_legacy_profiles {
        for profile in crate::store::default_profiles() {
            storage.put_profile(&profile).await?;
        }
        data.profiles = storage.load().await?.profiles;
    }
    ensure_default_folder(&mut data.folders);

    let warning = report.and_then(|report| report.warning());
    if let Some(warning) = &warning {
        warn!(warning = %warning, "profile vault opened with a warning");
    }
    info!(
        source = source.label(),
        profiles = data.profiles.len(),
        "profile vault opened"
    );
    Ok(Opened {
        state: StorageState::Ready {
            storage: Arc::new(storage),
            source,
            warning,
        },
        data,
    })
}

impl From<PassphraseError> for StorageError {
    fn from(error: PassphraseError) -> Self {
        Self::Passphrase(error)
    }
}

/// The "Default" folder always exists, whether or not it was ever stored.
pub fn ensure_default_folder(folders: &mut Vec<Folder>) {
    if !folders.iter().any(|folder| folder.id == "default") {
        folders.insert(
            0,
            Folder {
                id: "default".into(),
                name: "Default".into(),
            },
        );
    }
}

// ----------------------------------------------------------------------
// Importing the old clear-text files
// ----------------------------------------------------------------------

/// The three files an earlier version kept in clear text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Profiles,
    Tags,
    Folders,
}

impl Kind {
    const ALL: [Self; 3] = [Self::Profiles, Self::Tags, Self::Folders];

    fn file_name(self) -> &'static str {
        match self {
            Self::Profiles => "profiles.json",
            Self::Tags => "tags.json",
            Self::Folders => "folders.json",
        }
    }

    fn collection(self) -> &'static str {
        match self {
            Self::Profiles => PROFILES,
            Self::Tags => TAGS,
            Self::Folders => FOLDERS,
        }
    }

    /// Parses one entry as the type this file holds. Returns its id and the
    /// document as the app would save it, defaults filled in.
    fn normalize(self, raw: &Value) -> Option<(String, Value)> {
        fn run<T: DeserializeOwned + Serialize>(
            raw: &Value,
            id: impl Fn(&T) -> &str,
        ) -> Option<(String, Value)> {
            let typed: T = serde_json::from_value(raw.clone()).ok()?;
            let doc = serde_json::to_value(&typed).ok()?;
            Some((id(&typed).to_owned(), doc))
        }
        match self {
            Self::Profiles => run::<Profile>(raw, |p| &p.id),
            Self::Tags => run::<Tag>(raw, |t| &t.id),
            Self::Folders => run::<Folder>(raw, |f| &f.id),
        }
    }
}

/// What a finished import leaves behind.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct MigrationReport {
    /// Documents imported.
    pub imported: usize,
    /// Files that were imported and verified but could not be deleted.
    pub not_removed: Vec<&'static str>,
}

impl MigrationReport {
    /// A warning for the window, when a clear-text file is still there.
    fn warning(&self) -> Option<String> {
        if self.not_removed.is_empty() {
            return None;
        }
        Some(format!(
            "Your profiles were moved into the encrypted vault, but {} could not be deleted. \
             It still holds them in clear text: delete it yourself.",
            self.not_removed.join(", ")
        ))
    }
}

/// The digests of the files an import has verified, saved in the vault until
/// the files are gone.
///
/// If deleting a file fails, the next start must not import it again: the
/// vault may have been edited since, and importing would overwrite those
/// edits. With this record it deletes the file instead. The digest is what
/// tells "the file I imported" from "a different file that appeared later".
#[derive(Debug, Serialize, Deserialize)]
struct ImportMarker {
    files: BTreeMap<String, String>,
}

/// One entry of a legacy file.
struct LegacyDoc {
    id: String,
    /// The entry as it was written in the file.
    raw: Value,
    /// The entry as the app saves it.
    normalized: Value,
}

struct LegacyFile {
    kind: Kind,
    path: PathBuf,
    digest: String,
    bytes: Vec<u8>,
}

/// Imports `profiles.json`, `tags.json` and `folders.json` from `dir` into
/// `store`, if they exist.
///
/// The order is the whole point:
///
/// 1. every file is parsed first, so a broken one stops everything;
/// 2. every document is written to the vault;
/// 3. every document is read back and compared with the file it came from;
/// 4. only then are the files deleted.
///
/// If a step fails, the files are untouched. The comparison is strict: every
/// field of every entry in a file has to be present, unchanged, in the vault,
/// so a field the app would drop is a failure, not a silent loss.
///
/// Returns `None` when there was nothing to import.
pub(crate) async fn migrate_legacy<S: DocStore>(
    store: &S,
    dir: &Path,
    remove: &(dyn Fn(&Path) -> io::Result<()> + Sync),
) -> Result<Option<MigrationReport>, StorageError> {
    let marker = match store.get_doc(META, IMPORT_MARKER).await? {
        Some(doc) => serde_json::from_value::<ImportMarker>(doc).ok(),
        None => None,
    };

    let files = read_legacy_files(dir).await?;
    if files.is_empty() {
        if marker.is_some() {
            store.delete_doc(META, IMPORT_MARKER).await?;
        }
        return Ok(None);
    }

    if let Some(marker) = marker {
        // An earlier start imported and verified these files but could not
        // delete them. Finish that; do not import again.
        for file in &files {
            let name = file.kind.file_name();
            if marker.files.get(name) != Some(&file.digest) {
                return Err(MigrationError::ChangedSinceImport { file: name }.into());
            }
        }
        return finish(store, &files, 0, remove).await.map(Some);
    }

    let mut imported = Vec::new();
    for file in &files {
        imported.push((file.kind, parse_legacy(file)?));
    }

    for (kind, docs) in &imported {
        for doc in docs {
            store
                .put_doc(kind.collection(), &doc.id, &doc.normalized)
                .await?;
        }
    }

    for (kind, docs) in &imported {
        for doc in docs {
            let stored = store.get_doc(kind.collection(), &doc.id).await?;
            let matches = stored
                .as_ref()
                .is_some_and(|stored| *stored == doc.normalized && json_covers(&doc.raw, stored));
            if !matches {
                return Err(MigrationError::VerificationFailed {
                    collection: kind.collection(),
                    id: doc.id.clone(),
                }
                .into());
            }
        }
    }

    let marker = ImportMarker {
        files: files
            .iter()
            .map(|file| (file.kind.file_name().to_owned(), file.digest.clone()))
            .collect(),
    };
    put_typed(store, META, IMPORT_MARKER, &marker).await?;

    let count = imported.iter().map(|(_, docs)| docs.len()).sum();
    finish(store, &files, count, remove).await.map(Some)
}

/// Deletes the verified files, and the marker once none is left.
async fn finish<S: DocStore>(
    store: &S,
    files: &[LegacyFile],
    imported: usize,
    remove: &(dyn Fn(&Path) -> io::Result<()> + Sync),
) -> Result<MigrationReport, StorageError> {
    let mut report = MigrationReport {
        imported,
        not_removed: Vec::new(),
    };
    for file in files {
        match remove(&file.path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                warn!(file = file.kind.file_name(), error = %error, "could not delete a legacy file");
                report.not_removed.push(file.kind.file_name());
            }
        }
    }
    if report.not_removed.is_empty() {
        store.delete_doc(META, IMPORT_MARKER).await?;
    }
    Ok(report)
}

/// Reads whichever legacy files exist. A file that is there but cannot be
/// read is an error, not an absence.
async fn read_legacy_files(dir: &Path) -> Result<Vec<LegacyFile>, StorageError> {
    let mut files = Vec::new();
    for kind in Kind::ALL {
        let path = dir.join(kind.file_name());
        let present = path
            .try_exists()
            .map_err(|error| MigrationError::Unreadable {
                file: kind.file_name(),
                reason: error.kind().to_string(),
            })?;
        if !present {
            continue;
        }
        let bytes = tokio::fs::read(&path)
            .await
            .map_err(|error| MigrationError::Unreadable {
                file: kind.file_name(),
                reason: error.kind().to_string(),
            })?;
        files.push(LegacyFile {
            kind,
            digest: hex(digest(&SHA256, &bytes).as_ref()),
            path,
            bytes,
        });
    }
    Ok(files)
}

/// Parses a legacy file into documents. Where an id appears twice the later
/// entry wins, as it did when the app loaded the file.
fn parse_legacy(file: &LegacyFile) -> Result<Vec<LegacyDoc>, MigrationError> {
    let name = file.kind.file_name();
    // serde_json's messages can quote a value, so only the position is kept.
    let entries: Vec<Value> =
        serde_json::from_slice(&file.bytes).map_err(|error| MigrationError::Unreadable {
            file: name,
            reason: format!(
                "not a JSON array of entries, at line {} column {}",
                error.line(),
                error.column()
            ),
        })?;

    let mut docs: Vec<LegacyDoc> = Vec::with_capacity(entries.len());
    for (index, raw) in entries.into_iter().enumerate() {
        let (id, normalized) = file
            .kind
            .normalize(&raw)
            .ok_or(MigrationError::BadItem { file: name, index })?;
        let doc = LegacyDoc {
            id,
            raw,
            normalized,
        };
        match docs.iter_mut().find(|existing| existing.id == doc.id) {
            Some(existing) => *existing = doc,
            None => docs.push(doc),
        }
    }
    Ok(docs)
}

/// Whether `stored` holds everything `legacy` says.
///
/// Objects match when every key of `legacy` is in `stored` with a matching
/// value (extra keys in `stored`, such as defaults the app filled in, are
/// fine, and a `null` counts as absent). Numbers match by value, so `100` and
/// `100.0` agree.
fn json_covers(legacy: &Value, stored: &Value) -> bool {
    match (legacy, stored) {
        (Value::Object(legacy), Value::Object(stored)) => {
            legacy.iter().all(|(key, value)| match stored.get(key) {
                Some(found) => json_covers(value, found),
                None => value.is_null(),
            })
        }
        (Value::Array(legacy), Value::Array(stored)) => {
            legacy.len() == stored.len()
                && legacy
                    .iter()
                    .zip(stored)
                    .all(|(legacy, stored)| json_covers(legacy, stored))
        }
        (Value::Number(legacy), Value::Number(stored)) => {
            legacy == stored || legacy.as_f64() == stored.as_f64()
        }
        (legacy, stored) => legacy == stored,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use serde_json::json;

    use super::*;
    use crate::passphrase::memory::{Fault, MemorySecretStore};

    const FAKE: &str = "fake-passphrase-for-tests";
    const FAKE_COOKIE: &str = "fake-cookie-value-123";
    const FAKE_PROXY_PASSWORD: &str = "fake-proxy-password";

    fn profile_json(id: &str, name: &str) -> Value {
        json!({
            "id": id,
            "name": name,
            "container_url": "http://localhost:4444",
            "browser": "Chrome",
            "mode": "WebDriver",
            "user_agent": null,
            "proxy": format!("http://alice:{FAKE_PROXY_PASSWORD}@proxy.example:8080"),
            "locale": "en-US",
            "latitude": 40.7128,
            "longitude": -74.006,
            "accuracy": 100,
            "headless": false,
            "tags": [],
            "folder_id": "default",
            "cookies": [{
                "name": "session",
                "value": FAKE_COOKIE,
                "domain": ".example.com",
                "path": "/",
                "expires": 1893456000,
                "secure": true,
                "http_only": true,
                "same_site": "Lax"
            }]
        })
    }

    fn write_legacy(dir: &Path, file: &str, value: &Value) {
        std::fs::write(dir.join(file), serde_json::to_vec_pretty(value).unwrap()).unwrap();
    }

    fn legacy_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        write_legacy(
            dir.path(),
            "profiles.json",
            &json!([profile_json("p1", "Alpha"), profile_json("p2", "Beta")]),
        );
        write_legacy(
            dir.path(),
            "tags.json",
            &json!([{ "id": "t1", "name": "Work", "color": "#ff0000" }]),
        );
        write_legacy(
            dir.path(),
            "folders.json",
            &json!([{ "id": "f1", "name": "Clients" }]),
        );
        dir
    }

    fn exists(dir: &Path, file: &str) -> bool {
        dir.join(file).exists()
    }

    /// A store that behaves like a real one until told to lose or alter data.
    struct LossyStore {
        inner: ProfileVault,
        /// Documents with this id are stored with their cookies dropped.
        corrupt_id: Option<&'static str>,
        /// Documents with this id are not stored at all.
        drop_id: Option<&'static str>,
        puts: Mutex<usize>,
    }

    impl LossyStore {
        async fn new(corrupt_id: Option<&'static str>, drop_id: Option<&'static str>) -> Self {
            Self {
                inner: ProfileVault::in_memory(FAKE).await.unwrap(),
                corrupt_id,
                drop_id,
                puts: Mutex::new(0),
            }
        }
    }

    impl DocStore for LossyStore {
        async fn put_doc(
            &self,
            collection: &str,
            id: &str,
            doc: &Value,
        ) -> Result<(), StorageError> {
            *self.puts.lock().unwrap() += 1;
            if self.drop_id == Some(id) {
                return Ok(());
            }
            if self.corrupt_id == Some(id) {
                let mut changed = doc.clone();
                changed["cookies"] = json!([]);
                return self.inner.put_doc(collection, id, &changed).await;
            }
            self.inner.put_doc(collection, id, doc).await
        }

        async fn get_doc(&self, collection: &str, id: &str) -> Result<Option<Value>, StorageError> {
            self.inner.get_doc(collection, id).await
        }

        async fn list_docs(&self, collection: &str) -> Result<Vec<(String, Value)>, StorageError> {
            self.inner.list_docs(collection).await
        }

        async fn delete_doc(&self, collection: &str, id: &str) -> Result<bool, StorageError> {
            self.inner.delete_doc(collection, id).await
        }
    }

    fn really_remove(path: &Path) -> io::Result<()> {
        std::fs::remove_file(path)
    }

    // ---- error and comparison helpers ----

    #[test]
    fn a_wrong_passphrase_error_is_recognised_and_other_errors_are_not() {
        let wrong = StorageError::from(SeleniumBaseError::Authentication("anything".into()));
        assert!(matches!(wrong, StorageError::WrongPassphrase));

        let other = StorageError::from(SeleniumBaseError::Database("disk full".into()));
        assert!(matches!(other, StorageError::Vault(_)));
    }

    #[test]
    fn vault_errors_do_not_quote_the_value_that_broke_them() {
        let error = SeleniumBaseError::Database(
            "profiles/p1 is not the expected document: invalid type: string \"hunter2\", \
             expected a boolean"
                .to_owned(),
        );

        let described = describe_vault_error(&error);

        assert!(described.contains("profiles/p1 is not the expected document"));
        assert!(!described.contains("hunter2"), "{described}");
    }

    #[test]
    fn json_covers_ignores_added_defaults_and_nulls_but_not_changes() {
        let legacy = json!({ "a": 1, "b": null, "c": { "d": [1, 2.0] } });

        assert!(json_covers(
            &legacy,
            &json!({ "a": 1.0, "c": { "d": [1.0, 2], "extra": true }, "more": 5 })
        ));
        assert!(!json_covers(
            &legacy,
            &json!({ "a": 2, "c": { "d": [1, 2] } })
        ));
        assert!(!json_covers(&legacy, &json!({ "a": 1, "c": { "d": [1] } })));
        assert!(!json_covers(&legacy, &json!({ "a": 1, "c": {} })));
        assert!(!json_covers(&json!({ "gone": 1 }), &json!({})));
    }

    // ---- the import ----

    #[tokio::test]
    async fn legacy_files_are_imported_verified_and_then_deleted() {
        let dir = legacy_dir();
        let store = LossyStore::new(None, None).await;

        let report = migrate_legacy(&store, dir.path(), &really_remove)
            .await
            .unwrap()
            .expect("there was something to import");

        assert_eq!(report.imported, 4);
        assert!(report.not_removed.is_empty());
        for file in ["profiles.json", "tags.json", "folders.json"] {
            assert!(!exists(dir.path(), file), "{file} should be gone");
        }
        let profiles: Vec<Profile> = load_typed(&store, PROFILES).await.unwrap();
        let names: Vec<_> = profiles.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Alpha", "Beta"]);
        assert_eq!(profiles[0].cookies[0].value, FAKE_COOKIE);
        assert_eq!(profiles[0].cookies[0].expires, Some(1_893_456_000.0));
        assert_eq!(load_typed::<_, Tag>(&store, TAGS).await.unwrap().len(), 1);
        assert_eq!(
            load_typed::<_, Folder>(&store, FOLDERS).await.unwrap()[0].name,
            "Clients"
        );
        assert!(
            store.get_doc(META, IMPORT_MARKER).await.unwrap().is_none(),
            "the bookkeeping record is removed once the files are gone"
        );
    }

    #[tokio::test]
    async fn nothing_to_import_is_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let store = LossyStore::new(None, None).await;

        let report = migrate_legacy(&store, dir.path(), &really_remove)
            .await
            .unwrap();

        assert_eq!(report, None);
    }

    #[tokio::test]
    async fn a_document_that_reads_back_changed_keeps_every_legacy_file() {
        let dir = legacy_dir();
        let store = LossyStore::new(Some("p2"), None).await;

        let error = migrate_legacy(&store, dir.path(), &really_remove)
            .await
            .unwrap_err();

        assert!(
            matches!(
                error,
                StorageError::Migration(MigrationError::VerificationFailed { collection: "profiles", ref id }) if id == "p2"
            ),
            "{error}"
        );
        for file in ["profiles.json", "tags.json", "folders.json"] {
            assert!(exists(dir.path(), file), "{file} must not be deleted");
        }
        assert!(
            store.get_doc(META, IMPORT_MARKER).await.unwrap().is_none(),
            "an unverified import leaves no record that it was verified"
        );
    }

    #[tokio::test]
    async fn a_document_that_was_never_stored_keeps_every_legacy_file() {
        let dir = legacy_dir();
        let store = LossyStore::new(None, Some("t1")).await;

        let error = migrate_legacy(&store, dir.path(), &really_remove)
            .await
            .unwrap_err();

        assert!(
            matches!(
                error,
                StorageError::Migration(MigrationError::VerificationFailed {
                    collection: "tags",
                    ..
                })
            ),
            "{error}"
        );
        assert!(exists(dir.path(), "profiles.json"));
        assert!(exists(dir.path(), "tags.json"));
        assert!(exists(dir.path(), "folders.json"));
    }

    #[tokio::test]
    async fn a_field_the_app_would_drop_fails_verification_instead_of_vanishing() {
        let dir = legacy_dir();
        let mut profile = profile_json("p9", "Extra");
        profile["a_field_from_the_future"] = json!("keep me");
        write_legacy(dir.path(), "profiles.json", &json!([profile]));
        let store = LossyStore::new(None, None).await;

        let error = migrate_legacy(&store, dir.path(), &really_remove)
            .await
            .unwrap_err();

        assert!(
            matches!(
                error,
                StorageError::Migration(MigrationError::VerificationFailed { .. })
            ),
            "{error}"
        );
        assert!(exists(dir.path(), "profiles.json"));
    }

    #[tokio::test]
    async fn malformed_json_stops_the_import_before_anything_is_written() {
        let dir = legacy_dir();
        std::fs::write(
            dir.path().join("profiles.json"),
            format!("[{{\"cookies\": \"{FAKE_COOKIE}\" "),
        )
        .unwrap();
        let store = LossyStore::new(None, None).await;

        let error = migrate_legacy(&store, dir.path(), &really_remove)
            .await
            .unwrap_err();

        assert!(
            matches!(
                error,
                StorageError::Migration(MigrationError::Unreadable {
                    file: "profiles.json",
                    ..
                })
            ),
            "{error}"
        );
        assert_eq!(*store.puts.lock().unwrap(), 0, "nothing may be written");
        assert!(exists(dir.path(), "tags.json"));
        assert!(
            !error.to_string().contains(FAKE_COOKIE),
            "an error must not quote the file: {error}"
        );
    }

    #[tokio::test]
    async fn an_entry_that_is_not_a_profile_is_reported_by_position() {
        let dir = legacy_dir();
        write_legacy(
            dir.path(),
            "profiles.json",
            &json!([profile_json("p1", "Alpha"), { "id": 7, "name": FAKE_COOKIE }]),
        );
        let store = LossyStore::new(None, None).await;

        let error = migrate_legacy(&store, dir.path(), &really_remove)
            .await
            .unwrap_err();

        assert!(
            matches!(
                error,
                StorageError::Migration(MigrationError::BadItem {
                    file: "profiles.json",
                    index: 1
                })
            ),
            "{error}"
        );
        assert!(!error.to_string().contains(FAKE_COOKIE));
        assert_eq!(*store.puts.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn a_repeated_id_keeps_the_later_entry() {
        let dir = tempfile::tempdir().unwrap();
        write_legacy(
            dir.path(),
            "profiles.json",
            &json!([profile_json("p1", "First"), profile_json("p1", "Second")]),
        );
        let store = LossyStore::new(None, None).await;

        migrate_legacy(&store, dir.path(), &really_remove)
            .await
            .unwrap();

        let profiles: Vec<Profile> = load_typed(&store, PROFILES).await.unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].name, "Second");
    }

    #[tokio::test]
    async fn a_file_that_cannot_be_deleted_is_reported_and_not_imported_again() {
        let dir = legacy_dir();
        let store = LossyStore::new(None, None).await;
        let stubborn = |path: &Path| -> io::Result<()> {
            if path.ends_with("tags.json") {
                Err(io::Error::new(io::ErrorKind::PermissionDenied, "locked"))
            } else {
                std::fs::remove_file(path)
            }
        };

        let report = migrate_legacy(&store, dir.path(), &stubborn)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(report.not_removed, ["tags.json"]);
        assert!(report.warning().unwrap().contains("tags.json"));
        assert!(!exists(dir.path(), "profiles.json"));
        assert!(exists(dir.path(), "tags.json"));

        // The user edits a profile in the vault. The next start still sees
        // tags.json; it must delete it, not import it over those edits.
        let mut edited: Profile = load_typed::<_, Profile>(&store, PROFILES)
            .await
            .unwrap()
            .remove(0);
        edited.name = "Edited in the vault".into();
        put_typed(&store, PROFILES, &edited.id.clone(), &edited)
            .await
            .unwrap();
        write_legacy(
            dir.path(),
            "tags.json",
            &json!([{ "id": "t1", "name": "Work", "color": "#ff0000" }]),
        );

        let second = migrate_legacy(&store, dir.path(), &really_remove)
            .await
            .unwrap()
            .unwrap();

        assert!(second.not_removed.is_empty());
        assert!(!exists(dir.path(), "tags.json"));
        let profiles: Vec<Profile> = load_typed(&store, PROFILES).await.unwrap();
        assert_eq!(profiles[0].name, "Edited in the vault");
        assert!(store.get_doc(META, IMPORT_MARKER).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn a_different_file_after_an_interrupted_cleanup_is_refused() {
        let dir = legacy_dir();
        let store = LossyStore::new(None, None).await;
        let stubborn = |path: &Path| -> io::Result<()> {
            if path.ends_with("tags.json") {
                Err(io::Error::new(io::ErrorKind::PermissionDenied, "locked"))
            } else {
                std::fs::remove_file(path)
            }
        };
        migrate_legacy(&store, dir.path(), &stubborn).await.unwrap();
        write_legacy(
            dir.path(),
            "tags.json",
            &json!([{ "id": "t2", "name": "Different", "color": "#00ff00" }]),
        );

        let error = migrate_legacy(&store, dir.path(), &really_remove)
            .await
            .unwrap_err();

        assert!(
            matches!(
                error,
                StorageError::Migration(MigrationError::ChangedSinceImport { file: "tags.json" })
            ),
            "{error}"
        );
        assert!(
            exists(dir.path(), "tags.json"),
            "the file must be left alone"
        );
        let tags: Vec<Tag> = load_typed(&store, TAGS).await.unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "Work", "the different file was not imported");
    }

    // ---- opening the real vault ----

    #[tokio::test]
    async fn a_fresh_install_gets_the_sample_profiles_once() {
        let dir = tempfile::tempdir().unwrap();
        let keychain = MemorySecretStore::empty();

        let first = open_storage(dir.path(), None, &keychain).await;

        let StorageState::Ready {
            ref storage,
            source,
            ref warning,
        } = first.state
        else {
            panic!("expected a ready store, got {:?}", first.state);
        };
        assert_eq!(source, PassphraseSource::KeychainNew);
        assert!(warning.is_none());
        assert_eq!(first.data.profiles.len(), 2);
        assert!(first.data.folders.iter().any(|f| f.id == "default"));
        assert!(keychain.current().is_some(), "the passphrase was kept");

        // Delete both samples; they must not come back.
        for profile in &first.data.profiles {
            storage.delete_profile(&profile.id).await.unwrap();
        }
        drop(first);
        let second = open_storage(dir.path(), None, &keychain).await;
        assert!(matches!(second.state, StorageState::Ready { .. }));
        assert!(second.data.profiles.is_empty());
    }

    #[tokio::test]
    async fn changes_survive_closing_and_reopening() {
        let dir = tempfile::tempdir().unwrap();
        let opened = open_storage(dir.path(), Some(FAKE), &MemorySecretStore::empty()).await;
        let StorageState::Ready {
            storage, source, ..
        } = opened.state
        else {
            panic!("expected a ready store");
        };
        assert_eq!(source, PassphraseSource::Environment);
        let mut profile: Profile = serde_json::from_value(profile_json("keep", "Kept")).unwrap();
        profile.cookies[0].value = FAKE_COOKIE.into();
        storage.put_profile(&profile).await.unwrap();
        storage
            .put_tag(&Tag {
                id: "t".into(),
                name: "Tag".into(),
                color: "#000000".into(),
            })
            .await
            .unwrap();
        storage
            .put_folder(&Folder {
                id: "f".into(),
                name: "Folder".into(),
            })
            .await
            .unwrap();
        drop(storage);

        let reopened = open_storage(dir.path(), Some(FAKE), &MemorySecretStore::empty()).await;

        assert!(matches!(reopened.state, StorageState::Ready { .. }));
        let kept = reopened
            .data
            .profiles
            .iter()
            .find(|p| p.id == "keep")
            .unwrap();
        assert_eq!(kept.cookies[0].value, FAKE_COOKIE);
        assert_eq!(reopened.data.tags.len(), 1);
        assert!(reopened.data.folders.iter().any(|f| f.id == "f"));
    }

    #[tokio::test]
    async fn the_vault_file_holds_no_clear_text() {
        let dir = tempfile::tempdir().unwrap();
        let opened = open_storage(dir.path(), Some(FAKE), &MemorySecretStore::empty()).await;
        let StorageState::Ready { storage, .. } = opened.state else {
            panic!("expected a ready store");
        };
        let profile: Profile =
            serde_json::from_value(profile_json("p", "Recognisable Name")).unwrap();
        storage.put_profile(&profile).await.unwrap();
        drop(storage);

        // Everything in the directory, the write-ahead log included.
        for entry in std::fs::read_dir(dir.path()).unwrap() {
            let bytes = std::fs::read(entry.unwrap().path()).unwrap();
            let text = String::from_utf8_lossy(&bytes);
            for secret in [FAKE_COOKIE, FAKE_PROXY_PASSWORD, "Recognisable Name"] {
                assert!(!text.contains(secret), "{secret} is in the clear on disk");
            }
        }
    }

    #[tokio::test]
    async fn a_wrong_passphrase_is_an_error_and_leaves_the_vault_intact() {
        let dir = tempfile::tempdir().unwrap();
        let opened = open_storage(dir.path(), Some(FAKE), &MemorySecretStore::empty()).await;
        let StorageState::Ready { storage, .. } = opened.state else {
            panic!("expected a ready store");
        };
        storage
            .put_profile(&serde_json::from_value(profile_json("p", "Precious")).unwrap())
            .await
            .unwrap();
        drop(storage);

        let wrong = open_storage(
            dir.path(),
            Some("not-the-passphrase"),
            &MemorySecretStore::empty(),
        )
        .await;

        let StorageState::Unavailable { ref message } = wrong.state else {
            panic!(
                "a wrong passphrase must not open the vault: {:?}",
                wrong.state
            );
        };
        assert!(message.contains("passphrase does not match"), "{message}");
        assert!(!message.contains("not-the-passphrase"), "{message}");
        assert!(wrong.data.profiles.is_empty(), "no profiles may be shown");
        assert!(!wrong.state.status().ok);

        // Nothing was overwritten: the right passphrase still finds the data.
        let again = open_storage(dir.path(), Some(FAKE), &MemorySecretStore::empty()).await;
        assert!(matches!(again.state, StorageState::Ready { .. }));
        assert!(again.data.profiles.iter().any(|p| p.name == "Precious"));
    }

    #[tokio::test]
    async fn an_existing_vault_with_no_keychain_item_is_unavailable_not_recreated() {
        let dir = tempfile::tempdir().unwrap();
        let opened = open_storage(dir.path(), Some(FAKE), &MemorySecretStore::empty()).await;
        assert!(matches!(opened.state, StorageState::Ready { .. }));
        drop(opened);
        let keychain = MemorySecretStore::empty();

        let locked = open_storage(dir.path(), None, &keychain).await;

        let StorageState::Unavailable { ref message } = locked.state else {
            panic!("expected unavailable, got {:?}", locked.state);
        };
        assert!(message.contains("keychain"), "{message}");
        assert_eq!(keychain.writes(), 0, "no new passphrase may be generated");
    }

    #[tokio::test]
    async fn a_broken_keychain_makes_the_store_unavailable_with_a_reason() {
        let dir = tempfile::tempdir().unwrap();

        let opened = open_storage(dir.path(), None, &MemorySecretStore::failing(Fault::Read)).await;

        let StorageState::Unavailable { ref message } = opened.state else {
            panic!("expected unavailable");
        };
        assert!(message.contains("keychain"), "{message}");
        assert!(
            !dir.path().join(VAULT_FILE).exists(),
            "no vault may be created without a passphrase to keep"
        );
    }

    #[tokio::test]
    async fn opening_a_directory_with_legacy_files_migrates_them_and_deletes_them() {
        let dir = legacy_dir();

        let opened = open_storage(dir.path(), Some(FAKE), &MemorySecretStore::empty()).await;

        assert!(matches!(
            opened.state,
            StorageState::Ready { warning: None, .. }
        ));
        assert_eq!(
            opened.data.profiles.len(),
            2,
            "the samples are not added on top"
        );
        assert_eq!(opened.data.tags.len(), 1);
        assert!(opened.data.folders.iter().any(|f| f.id == "f1"));
        assert!(opened.data.folders.iter().any(|f| f.id == "default"));
        assert!(!exists(dir.path(), "profiles.json"));
        assert!(!exists(dir.path(), "tags.json"));
        assert!(!exists(dir.path(), "folders.json"));
        let cookie = &opened.data.profiles[0].cookies[0];
        assert_eq!(cookie.value, FAKE_COOKIE);
    }

    #[tokio::test]
    async fn a_failed_import_makes_the_store_unavailable_and_keeps_the_files() {
        let dir = legacy_dir();
        std::fs::write(dir.path().join("tags.json"), "not json").unwrap();

        let opened = open_storage(dir.path(), Some(FAKE), &MemorySecretStore::empty()).await;

        let StorageState::Unavailable { ref message } = opened.state else {
            panic!("expected unavailable, got {:?}", opened.state);
        };
        assert!(message.contains("tags.json"), "{message}");
        assert!(message.contains("left exactly as they were"), "{message}");
        assert!(exists(dir.path(), "profiles.json"));
        assert!(exists(dir.path(), "tags.json"));
        assert!(exists(dir.path(), "folders.json"));
        assert!(opened.data.profiles.is_empty());
    }

    #[test]
    fn the_status_the_window_shows_never_carries_a_passphrase() {
        let status = StorageState::Unavailable {
            message: StorageError::WrongPassphrase.to_string(),
        }
        .status();

        assert!(!status.ok);
        assert!(status.error.is_some());
        let json = serde_json::to_string(&status).unwrap();
        assert!(!json.contains(FAKE));
    }

    #[test]
    fn the_default_folder_is_added_once() {
        let mut folders = vec![Folder {
            id: "f".into(),
            name: "F".into(),
        }];

        ensure_default_folder(&mut folders);
        ensure_default_folder(&mut folders);

        assert_eq!(folders.iter().filter(|f| f.id == "default").count(), 1);
        assert_eq!(folders[0].id, "default");
    }

    /// The vault is shared between the Tauri runtime and the REST server's own
    /// runtime, so it has to work when opened on one and used on another.
    #[test]
    fn the_vault_works_across_runtimes() {
        let dir = tempfile::tempdir().unwrap();
        let opening = tokio::runtime::Runtime::new().unwrap();
        let using = tokio::runtime::Runtime::new().unwrap();

        let opened = opening.block_on(open_storage(
            dir.path(),
            Some(FAKE),
            &MemorySecretStore::empty(),
        ));
        drop(opening);
        let StorageState::Ready { storage, .. } = opened.state else {
            panic!("expected a ready store");
        };
        using.block_on(async {
            let profile: Profile = serde_json::from_value(profile_json("x", "Across")).unwrap();
            storage.put_profile(&profile).await.unwrap();
            let loaded = storage.load().await.unwrap();
            assert!(loaded.profiles.iter().any(|p| p.name == "Across"));
        });
    }
}
