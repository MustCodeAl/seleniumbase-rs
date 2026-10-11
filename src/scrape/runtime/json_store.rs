//! The `JsonStateStore`: one state file, written atomically with owner-only permissions.
//!
//! The file holds `{ "schema": 1, "sites": { "<site>": { ... } } }`. A missing file is an empty
//! store. A file that cannot be read as state is an error and is left alone: it is never reset,
//! because a reset would silently throw away the history of every site. A write goes to a
//! sibling temporary file that is synced and then renamed over the target, so a crash leaves the
//! old file or the new one, never half of one.
//!
//! One process writes a file at a time; there is no lock between processes. A store keeps the
//! whole table in memory, and each change writes the whole file, so it suits a few thousand sites
//! at most ([`MAX_STATE_SITES`] is the bound).

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use serde::de::Error as _;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::scrape::ledger::state::{
    insert_bounded, DomainState, StateError, StateStore, MAX_STATE_SITES, STATE_SCHEMA,
};

/// The largest state file that is read. A state file this big holds far more sites than
/// [`MAX_STATE_SITES`] allows, so something else wrote it.
pub const MAX_STATE_FILE_BYTES: u64 = 16 * 1024 * 1024;

/// The permissions of a state file on Unix: the owner reads and writes, nobody else.
#[cfg(unix)]
const STATE_FILE_MODE: u32 = 0o600;

/// Makes the name of each temporary file unique within the process.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Just the schema of a file, read first so that a file of another layout is reported as that
/// and not as corrupt.
#[derive(Deserialize)]
struct Header {
    schema: u32,
}

/// The sites of a file.
#[derive(Deserialize)]
struct Sites {
    sites: BTreeMap<String, DomainState>,
}

/// A file as written.
#[derive(Serialize)]
struct StateFile<'a> {
    schema: u32,
    sites: &'a BTreeMap<String, DomainState>,
}

/// A state store backed by one JSON file.
pub struct JsonStateStore {
    path: PathBuf,
    sites: Mutex<BTreeMap<String, DomainState>>,
}

impl fmt::Debug for JsonStateStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JsonStateStore")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl JsonStateStore {
    /// Opens the store at `path`, reading the file if there is one. This reads the file at once
    /// on the calling thread; do it at start-up.
    ///
    /// # Errors
    ///
    /// Returns [`StateError::Io`] when the file exists and cannot be read, [`StateError::TooLarge`]
    /// when it is over [`MAX_STATE_FILE_BYTES`], [`StateError::Schema`] when it was written with
    /// another layout, and [`StateError::Corrupt`] when it is not valid state. A missing file is
    /// not an error: the store is empty, and the file appears with the first change.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, StateError> {
        Self::open_with_limit(path.into(), MAX_STATE_FILE_BYTES)
    }

    fn open_with_limit(path: PathBuf, limit: u64) -> Result<Self, StateError> {
        let sites = load(&path, limit)?;
        Ok(Self {
            path,
            sites: Mutex::new(sites),
        })
    }

    /// The file this store writes.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Writes `sites` to the file, off the async threads.
    async fn save(&self, sites: &BTreeMap<String, DomainState>) -> Result<(), StateError> {
        let io_error = |source| StateError::Io {
            path: self.path.clone(),
            source,
        };
        let bytes = serde_json::to_vec_pretty(&StateFile {
            schema: STATE_SCHEMA,
            sites,
        })
        .map_err(|source| io_error(io::Error::other(source)))?;
        let path = self.path.clone();
        tokio::task::spawn_blocking(move || persist(&path, &bytes))
            .await
            .map_err(|source| io_error(io::Error::other(source)))?
            .map_err(io_error)
    }
}

#[async_trait]
impl StateStore for JsonStateStore {
    #[tracing::instrument(skip_all, fields(site = %site))]
    async fn get(&self, site: &str) -> Result<Option<DomainState>, StateError> {
        Ok(self.sites.lock().await.get(site).cloned())
    }

    #[tracing::instrument(skip_all, fields(site = %state.site))]
    async fn put(&self, state: DomainState) -> Result<(), StateError> {
        let mut sites = self.sites.lock().await;
        // The change is written first and kept second, so memory never gets ahead of the file.
        let mut next = sites.clone();
        insert_bounded(&mut next, state, MAX_STATE_SITES);
        self.save(&next).await?;
        *sites = next;
        Ok(())
    }

    #[tracing::instrument(skip_all, fields(site = %site))]
    async fn remove(&self, site: &str) -> Result<(), StateError> {
        let mut sites = self.sites.lock().await;
        if !sites.contains_key(site) {
            return Ok(());
        }
        let mut next = sites.clone();
        next.remove(site);
        self.save(&next).await?;
        *sites = next;
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn sites(&self) -> Result<Vec<String>, StateError> {
        Ok(self.sites.lock().await.keys().cloned().collect())
    }
}

/// Reads the state file at `path`: no file is an empty table, anything unreadable is an error.
fn load(path: &Path, limit: u64) -> Result<BTreeMap<String, DomainState>, StateError> {
    let io_error = |source| StateError::Io {
        path: path.to_path_buf(),
        source,
    };
    let corrupt = |source| StateError::Corrupt {
        path: path.to_path_buf(),
        source,
    };
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => return Err(io_error(error)),
    };
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if u64::try_from(bytes.len()).map_or(true, |length| length > limit) {
        return Err(StateError::TooLarge {
            path: path.to_path_buf(),
            limit,
        });
    }
    let header: Header = serde_json::from_slice(&bytes).map_err(corrupt)?;
    if header.schema != STATE_SCHEMA {
        return Err(StateError::Schema(header.schema));
    }
    let file: Sites = serde_json::from_slice(&bytes).map_err(corrupt)?;
    for (key, state) in &file.sites {
        if key != &state.site {
            return Err(corrupt(serde_json::Error::custom(format!(
                "the entry {key:?} holds the state of {:?}",
                state.site
            ))));
        }
    }
    Ok(file.sites)
}

/// Writes `bytes` to `path`, creating the directories above it first.
fn persist(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    atomic_write(path, bytes)
}

/// Replaces the file at `path` with `bytes` so that a reader, or a crash, sees the old content or
/// the new, never a mixture. The bytes go to a sibling temporary file (created new, readable by
/// the owner only on Unix), which is synced and renamed over `path`; then the directory is synced
/// so the rename survives a power cut. A temporary file left by a failure is removed.
///
/// A failed directory sync is only logged: the data is in place by then, and not every file
/// system lets a directory be synced.
///
/// # Errors
///
/// Returns the first I/O error of creating, writing, syncing or renaming the file.
pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "the path has no file name"))?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let temp = parent.join(format!(
        ".{}.{}.{}.tmp",
        name.to_string_lossy(),
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let written = write_synced(&temp, bytes).and_then(|()| fs::rename(&temp, path));
    if written.is_err() {
        // Best effort: the original error is the one worth reporting.
        let _removed = fs::remove_file(&temp);
    }
    written?;
    if let Err(error) = sync_directory(parent) {
        tracing::debug!(directory = %parent.display(), %error, "could not sync the directory");
    }
    Ok(())
}

/// Creates `path` (it must not exist), writes `bytes` and flushes them to disk.
fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    owner_only(&mut options);
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Asks for a file that only its owner can read or write. Other systems have no such mode, and
/// the file there gets the permissions of its directory.
#[cfg(unix)]
fn owner_only(options: &mut fs::OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(STATE_FILE_MODE);
}

/// Asks for a file that only its owner can read or write. Other systems have no such mode, and
/// the file there gets the permissions of its directory.
#[cfg(not(unix))]
fn owner_only(_options: &mut fs::OpenOptions) {}

/// Flushes the directory entry of a renamed file.
fn sync_directory(directory: &Path) -> io::Result<()> {
    fs::File::open(directory)?.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scrape::challenge::detect::{ChallengeKind, Detection, Evidence};
    use crate::scrape::challenge::mitigation::Mode;
    use crate::scrape::egress::geo::CountryCode;
    use crate::scrape::egress::route::EgressKey;
    use std::sync::Arc;

    fn site(name: &str) -> DomainState {
        let mut state = DomainState::new(name);
        state.record(
            &Detection {
                kind: ChallengeKind::JsChallenge,
                evidence: Evidence::default(),
            },
            Mode::Http,
            1_000,
        );
        state.start_cooldown(5_000);
        state.last_egress = Some(EgressKey {
            scheme: "http".to_owned(),
            host: "p1.example".to_owned(),
            port: 8080,
            country: CountryCode::new("DE").ok(),
        });
        state
    }

    fn state_path(dir: &tempfile::TempDir) -> PathBuf {
        dir.path().join("state.json")
    }

    fn open(path: &Path) -> JsonStateStore {
        JsonStateStore::open(path).expect("opens")
    }

    /// The names of the files in `dir`, sorted.
    fn files(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .expect("lists")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }

    #[tokio::test]
    async fn a_missing_file_is_an_empty_store_and_stays_missing_until_a_change() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = state_path(&dir);
        let store = open(&path);
        assert_eq!(store.sites().await.expect("reads"), Vec::<String>::new());
        assert_eq!(store.get("a.test").await.expect("reads"), None);
        store
            .remove("a.test")
            .await
            .expect("removing nothing is fine");
        assert!(!path.exists(), "reading and removing nothing write nothing");
        store.put(site("a.test")).await.expect("writes");
        assert!(path.exists());
    }

    #[tokio::test]
    async fn state_survives_a_reopen() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = state_path(&dir);
        let store = open(&path);
        store.put(site("b.test")).await.expect("writes");
        store.put(site("a.test")).await.expect("writes");
        let again = open(&path);
        assert_eq!(again.sites().await.expect("reads"), ["a.test", "b.test"]);
        assert_eq!(
            again.get("a.test").await.expect("reads"),
            Some(site("a.test"))
        );
        assert_eq!(again.path(), path);
    }

    #[tokio::test]
    async fn a_put_replaces_and_a_remove_forgets_on_disk_too() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = state_path(&dir);
        let store = open(&path);
        store.put(site("a.test")).await.expect("writes");
        let mut changed = site("a.test");
        changed.successes = 41;
        store.put(changed.clone()).await.expect("writes");
        store.put(site("b.test")).await.expect("writes");
        store.remove("b.test").await.expect("removes");
        let again = open(&path);
        assert_eq!(again.sites().await.expect("reads"), ["a.test"]);
        assert_eq!(again.get("a.test").await.expect("reads"), Some(changed));
    }

    #[tokio::test]
    async fn the_file_carries_its_schema_and_is_plain_json() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = state_path(&dir);
        open(&path).put(site("a.test")).await.expect("writes");
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).expect("reads")).expect("is json");
        assert_eq!(value["schema"], 1);
        assert_eq!(value["sites"]["a.test"]["site"], "a.test");
        assert_eq!(value["sites"]["a.test"]["last_egress"]["country"], "DE");
    }

    #[tokio::test]
    async fn a_put_creates_the_directories_above_the_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("deep").join("er").join("state.json");
        open(&path).put(site("a.test")).await.expect("writes");
        assert!(path.exists());
    }

    #[test]
    fn a_corrupt_file_is_an_error_and_is_left_exactly_as_it_was() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = state_path(&dir);
        let broken: [&[u8]; 8] = [
            b"",
            b"not json",
            b"{\"schema\": 1, \"sites\": {",
            b"{\"schema\": 1}",
            b"{\"sites\": {}}",
            b"{\"schema\": 1, \"sites\": {\"a.test\": {\"successes\": 1}}}",
            b"[1, 2, 3]",
            &[0xff, 0xfe, 0x00, 0x01, 0x80],
        ];
        for bytes in broken {
            fs::write(&path, bytes).expect("writes");
            let error = JsonStateStore::open(&path).expect_err("refused");
            assert!(
                matches!(error, StateError::Corrupt { .. }),
                "{bytes:?}: {error}"
            );
            assert!(error.to_string().contains("state.json"), "{error}");
            assert_eq!(
                fs::read(&path).expect("reads"),
                bytes,
                "the file must not be touched"
            );
        }
    }

    #[test]
    fn a_half_written_file_is_corrupt_not_empty() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = state_path(&dir);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime");
        runtime.block_on(async {
            open(&path).put(site("a.test")).await.expect("writes");
        });
        let whole = fs::read(&path).expect("reads");
        fs::write(&path, &whole[..whole.len() / 2]).expect("cuts");
        assert!(matches!(
            JsonStateStore::open(&path),
            Err(StateError::Corrupt { .. })
        ));
    }

    #[test]
    fn an_entry_filed_under_the_wrong_site_is_corrupt() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = state_path(&dir);
        fs::write(
            &path,
            r#"{"schema":1,"sites":{"a.test":{"site":"b.test"}}}"#,
        )
        .expect("writes");
        let error = JsonStateStore::open(&path).expect_err("refused");
        assert!(matches!(error, StateError::Corrupt { .. }), "{error}");
        assert!(error.to_string().contains("b.test"), "{error}");
    }

    #[test]
    fn hostile_nesting_is_an_error_not_a_stack_overflow() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = state_path(&dir);
        let deep = format!("{{\"schema\":1,\"sites\":{}", "[".repeat(200_000));
        fs::write(&path, deep).expect("writes");
        assert!(matches!(
            JsonStateStore::open(&path),
            Err(StateError::Corrupt { .. })
        ));
    }

    #[test]
    fn another_schema_is_named_not_called_corrupt() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = state_path(&dir);
        for (text, schema) in [
            (r#"{"schema":2,"sites":{}}"#, 2),
            (r#"{"schema":0,"sites":{}}"#, 0),
            (r#"{"schema":7,"sites":"some other layout"}"#, 7),
        ] {
            fs::write(&path, text).expect("writes");
            let error = JsonStateStore::open(&path).expect_err("refused");
            assert!(
                matches!(error, StateError::Schema(found) if found == schema),
                "{text}: {error}"
            );
        }
    }

    #[test]
    fn a_file_over_the_limit_is_refused_unread() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = state_path(&dir);
        fs::write(
            &path,
            format!("{{\"schema\":1,\"sites\":{{}}}}{}", " ".repeat(100)),
        )
        .expect("writes");
        let error = JsonStateStore::open_with_limit(path.clone(), 64).expect_err("too big");
        assert!(
            matches!(error, StateError::TooLarge { limit: 64, .. }),
            "{error}"
        );
        assert!(JsonStateStore::open_with_limit(path, 4096).is_ok());
    }

    #[test]
    fn a_directory_where_the_file_should_be_is_an_io_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let error = JsonStateStore::open(dir.path()).expect_err("refused");
        assert!(matches!(error, StateError::Io { .. }), "{error}");
    }

    #[tokio::test]
    async fn a_leftover_temp_file_is_ignored_and_swept_by_nothing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = state_path(&dir);
        let leftover = dir.path().join(".state.json.99999.0.tmp");
        fs::write(&leftover, "half a wri").expect("writes");
        let store = open(&path);
        assert_eq!(store.sites().await.expect("reads"), Vec::<String>::new());
        store.put(site("a.test")).await.expect("writes");
        assert!(
            leftover.exists(),
            "a file the store did not make is not the store's to delete"
        );
        assert_eq!(files(dir.path()), [".state.json.99999.0.tmp", "state.json"]);
    }

    #[tokio::test]
    async fn a_write_leaves_no_temp_file_behind() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = open(&state_path(&dir));
        for name in ["a.test", "b.test", "c.test"] {
            store.put(site(name)).await.expect("writes");
        }
        assert_eq!(files(dir.path()), ["state.json"]);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn the_file_is_readable_by_its_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("tempdir");
        let path = state_path(&dir);
        open(&path).put(site("a.test")).await.expect("writes");
        let mode = |p: &Path| fs::metadata(p).expect("metadata").permissions().mode() & 0o777;
        assert_eq!(mode(&path), 0o600);
        // A looser file that was there before is replaced by a private one.
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("chmod");
        open(&path).put(site("b.test")).await.expect("writes");
        assert_eq!(mode(&path), 0o600);
    }

    #[test]
    fn atomic_write_replaces_the_whole_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("data.bin");
        atomic_write(&path, b"first, and a good deal longer").expect("writes");
        atomic_write(&path, b"second").expect("writes");
        assert_eq!(fs::read(&path).expect("reads"), b"second");
        atomic_write(&path, b"").expect("writes");
        assert_eq!(fs::read(&path).expect("reads"), b"");
        assert_eq!(files(dir.path()), ["data.bin"]);
    }

    #[test]
    fn atomic_write_into_a_missing_directory_fails_cleanly() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("missing").join("data.bin");
        let error = atomic_write(&path, b"x").expect_err("no directory");
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert_eq!(files(dir.path()), Vec::<String>::new());
    }

    #[test]
    fn atomic_write_over_a_directory_fails_and_cleans_up() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("target");
        fs::create_dir(&target).expect("mkdir");
        fs::write(target.join("keep"), "inside").expect("writes");
        assert!(atomic_write(&target, b"x").is_err());
        assert_eq!(files(dir.path()), ["target"], "the temp file is removed");
        assert_eq!(fs::read(target.join("keep")).expect("reads"), b"inside");
    }

    #[test]
    fn atomic_write_needs_a_file_name() {
        for path in [Path::new("/"), Path::new("..")] {
            let error = atomic_write(path, b"x").expect_err("no name");
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{path:?}");
        }
    }

    #[test]
    fn writers_racing_on_one_file_never_leave_a_mixture() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = Arc::new(dir.path().join("race.bin"));
        let contents: Vec<Vec<u8>> = (0..8u8).map(|i| vec![b'a' + i; 64 * 1024]).collect();
        let handles: Vec<_> = contents
            .iter()
            .cloned()
            .map(|bytes| {
                let path = Arc::clone(&path);
                std::thread::spawn(move || {
                    for _ in 0..20 {
                        atomic_write(&path, &bytes).expect("writes");
                    }
                })
            })
            .collect();
        for handle in handles {
            handle.join().expect("joins");
        }
        let finished = fs::read(&*path).expect("reads");
        assert!(
            contents.contains(&finished),
            "the file is one writer's whole content"
        );
        assert_eq!(files(dir.path()), ["race.bin"]);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_puts_all_land() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = state_path(&dir);
        let store = Arc::new(open(&path));
        let tasks: Vec<_> = (0..16)
            .map(|i| {
                let store = Arc::clone(&store);
                tokio::spawn(async move { store.put(site(&format!("s{i:02}.test"))).await })
            })
            .collect();
        for task in tasks {
            task.await.expect("joins").expect("writes");
        }
        assert_eq!(store.sites().await.expect("reads").len(), 16);
        assert_eq!(open(&path).sites().await.expect("reads").len(), 16);
    }

    #[tokio::test]
    async fn a_failed_write_changes_nothing_in_memory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sub").join("state.json");
        let store = open(&path);
        store.put(site("a.test")).await.expect("writes");
        // Something that is not a directory now sits where the directory must be.
        fs::remove_dir_all(dir.path().join("sub")).expect("rm");
        fs::write(dir.path().join("sub"), "a file").expect("writes");
        let error = store.put(site("b.test")).await.expect_err("cannot write");
        assert!(matches!(error, StateError::Io { .. }), "{error}");
        assert_eq!(store.sites().await.expect("reads"), ["a.test"]);
    }

    #[test]
    fn the_debug_form_names_the_file_and_nothing_else() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = open(&state_path(&dir));
        let shown = format!("{store:?}");
        assert!(shown.starts_with("JsonStateStore"), "{shown}");
        assert!(shown.contains("state.json"), "{shown}");
    }
}
