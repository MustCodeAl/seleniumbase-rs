use std::collections::HashMap;
use std::fmt;

use tokio::sync::Mutex;
use uuid::Uuid;

use seleniumbase_rs::{BrowserConfig, Fingerprint, OsType};

use crate::models::{
    Engine, Folder, Profile, RandomizeRequest, Randomized, SessionInfo, StorageStatus, Tag,
};
use crate::session::Session;
use crate::storage::{ensure_default_folder, Opened, ProfileStorage, StorageError, StorageState};

pub struct AppState {
    pub profiles: Mutex<Vec<Profile>>,
    pub sessions: Mutex<HashMap<String, Session>>,
    pub session_info: Mutex<HashMap<String, SessionInfo>>,
    pub tags: Mutex<Vec<Tag>>,
    pub folders: Mutex<Vec<Folder>>,
    /// Shared secret the local REST API requires on every request.
    ///
    /// The API listens on loopback, but any web page the user visits can also
    /// reach loopback, so binding there is not by itself an access control.
    /// This token is minted fresh each run and handed to the app's own window
    /// over Tauri IPC, which a web page cannot use.
    pub api_token: String,
    /// Where profiles, tags and folders are kept.
    pub storage: StorageState,
}

/// Why a change could not be saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersistError {
    /// The vault could not be opened at startup, so nothing is shown or saved.
    Unavailable(String),
    /// Writing to the vault failed.
    Failed(String),
}

impl fmt::Display for PersistError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(message) => f.write_str(message),
            Self::Failed(reason) => write!(f, "Could not save to the profile vault: {reason}"),
        }
    }
}

impl std::error::Error for PersistError {}

impl From<StorageError> for PersistError {
    fn from(error: StorageError) -> Self {
        Self::Failed(error.to_string())
    }
}

impl AppState {
    /// State that keeps nothing: every change lives in memory only.
    #[cfg(test)]
    pub fn new() -> Self {
        Self::from_opened(Opened {
            state: StorageState::Ephemeral,
            data: crate::storage::LoadedData::default(),
        })
    }

    /// State backed by an opened store, holding what it loaded.
    pub fn from_opened(opened: Opened) -> Self {
        let mut folders = opened.data.folders;
        ensure_default_folder(&mut folders);
        Self {
            profiles: Mutex::new(opened.data.profiles),
            sessions: Mutex::new(HashMap::new()),
            session_info: Mutex::new(HashMap::new()),
            tags: Mutex::new(opened.data.tags),
            folders: Mutex::new(folders),
            api_token: generate_api_token(),
            storage: opened.state,
        }
    }

    /// What the window shows about the store.
    pub fn storage_status(&self) -> StorageStatus {
        self.storage.status()
    }

    /// Fails when the vault could not be opened.
    ///
    /// Call this before answering from `profiles`, `tags` or `folders`: when
    /// the vault is locked they are empty, and an empty list would be a lie.
    pub fn ensure_storage(&self) -> Result<(), PersistError> {
        self.writer().map(drop)
    }

    /// The store to write to; `None` for an ephemeral state.
    fn writer(&self) -> Result<Option<&ProfileStorage>, PersistError> {
        match &self.storage {
            #[cfg(test)]
            StorageState::Ephemeral => Ok(None),
            StorageState::Ready { storage, .. } => Ok(Some(storage.as_ref())),
            StorageState::Unavailable { message } => {
                Err(PersistError::Unavailable(message.clone()))
            }
        }
    }

    // The methods below change the in-memory list and the vault together.
    // The vault is written first, so a failed write leaves the list as it was
    // and the two never disagree. Each holds the list's lock for the whole
    // change, which keeps concurrent changes in order.

    /// Adds a profile. Fails if one with the same id exists, because the vault
    /// keeps one document per id and the list must not hold more.
    pub async fn add_profile(&self, profile: Profile) -> Result<(), PersistError> {
        let storage = self.writer()?;
        let mut profiles = self.profiles.lock().await;
        if profiles.iter().any(|existing| existing.id == profile.id) {
            return Err(PersistError::Failed(format!(
                "a profile with the id {} already exists",
                profile.id
            )));
        }
        if let Some(storage) = storage {
            storage.put_profile(&profile).await?;
        }
        profiles.push(profile);
        Ok(())
    }

    /// Replaces the profile that has the same id. `false` when there is none.
    pub async fn update_profile(&self, profile: Profile) -> Result<bool, PersistError> {
        let storage = self.writer()?;
        let mut profiles = self.profiles.lock().await;
        let Some(slot) = profiles.iter_mut().find(|p| p.id == profile.id) else {
            return Ok(false);
        };
        if let Some(storage) = storage {
            storage.put_profile(&profile).await?;
        }
        *slot = profile;
        Ok(true)
    }

    /// Gives a profile a new, internally consistent identity: a randomized
    /// fingerprint that replaces whatever it had. `None` when there is no such
    /// profile.
    ///
    /// The identity claims this machine's operating system unless the request
    /// names another, because a page can still see the real graphics stack and
    /// fonts behind a different claim. Without a seed, a fresh one is drawn.
    pub async fn randomize_fingerprint(
        &self,
        id: &str,
        request: RandomizeRequest,
    ) -> Result<Option<Randomized>, PersistError> {
        self.ensure_storage()?;
        let os = request.os.unwrap_or_else(host_os);
        let seed = match request.seed {
            Some(seed) => seed,
            None => random_seed()?,
        };
        let found = self
            .profiles
            .lock()
            .await
            .iter()
            .find(|p| p.id == id)
            .cloned();
        let Some(mut profile) = found else {
            return Ok(None);
        };
        profile.fingerprint = Some(Fingerprint::randomized(os, seed));
        if !self.update_profile(profile.clone()).await? {
            return Ok(None);
        }
        Ok(Some(Randomized { os, seed, profile }))
    }

    /// Removes a profile. `false` when there is none.
    pub async fn delete_profile(&self, id: &str) -> Result<bool, PersistError> {
        let storage = self.writer()?;
        let mut profiles = self.profiles.lock().await;
        let Some(at) = profiles.iter().position(|p| p.id == id) else {
            return Ok(false);
        };
        if let Some(storage) = storage {
            storage.delete_profile(id).await?;
        }
        profiles.remove(at);
        Ok(true)
    }

    /// Adds a tag.
    pub async fn add_tag(&self, tag: Tag) -> Result<(), PersistError> {
        let storage = self.writer()?;
        let mut tags = self.tags.lock().await;
        if let Some(storage) = storage {
            storage.put_tag(&tag).await?;
        }
        tags.push(tag);
        Ok(())
    }

    /// Replaces the tag that has the same id. `false` when there is none.
    pub async fn update_tag(&self, tag: Tag) -> Result<bool, PersistError> {
        let storage = self.writer()?;
        let mut tags = self.tags.lock().await;
        let Some(slot) = tags.iter_mut().find(|t| t.id == tag.id) else {
            return Ok(false);
        };
        if let Some(storage) = storage {
            storage.put_tag(&tag).await?;
        }
        *slot = tag;
        Ok(true)
    }

    /// Removes a tag. `false` when there is none.
    pub async fn delete_tag(&self, id: &str) -> Result<bool, PersistError> {
        let storage = self.writer()?;
        let mut tags = self.tags.lock().await;
        let Some(at) = tags.iter().position(|t| t.id == id) else {
            return Ok(false);
        };
        if let Some(storage) = storage {
            storage.delete_tag(id).await?;
        }
        tags.remove(at);
        Ok(true)
    }

    /// Adds a folder.
    pub async fn add_folder(&self, folder: Folder) -> Result<(), PersistError> {
        let storage = self.writer()?;
        let mut folders = self.folders.lock().await;
        if let Some(storage) = storage {
            storage.put_folder(&folder).await?;
        }
        folders.push(folder);
        Ok(())
    }

    /// Replaces the folder that has the same id. `false` when there is none.
    pub async fn update_folder(&self, folder: Folder) -> Result<bool, PersistError> {
        let storage = self.writer()?;
        let mut folders = self.folders.lock().await;
        let Some(slot) = folders.iter_mut().find(|f| f.id == folder.id) else {
            return Ok(false);
        };
        if let Some(storage) = storage {
            storage.put_folder(&folder).await?;
        }
        *slot = folder;
        Ok(true)
    }

    /// Removes a folder. `false` when there is none.
    pub async fn delete_folder(&self, id: &str) -> Result<bool, PersistError> {
        let storage = self.writer()?;
        let mut folders = self.folders.lock().await;
        let Some(at) = folders.iter().position(|f| f.id == id) else {
            return Ok(false);
        };
        if let Some(storage) = storage {
            storage.delete_folder(id).await?;
        }
        folders.remove(at);
        Ok(true)
    }
}

/// The operating system this machine runs.
fn host_os() -> OsType {
    if cfg!(target_os = "macos") {
        OsType::Macos
    } else if cfg!(target_os = "windows") {
        OsType::Windows
    } else {
        OsType::Linux
    }
}

/// A random seed of 53 bits, the most a JavaScript number holds exactly, so
/// the window can show it and send it back unchanged.
fn random_seed() -> Result<u64, PersistError> {
    let mut bytes = [0_u8; 8];
    getrandom::fill(&mut bytes).map_err(|_| {
        PersistError::Failed("the operating system could not supply a random seed".to_owned())
    })?;
    Ok(u64::from_le_bytes(bytes) & ((1 << 53) - 1))
}

/// Mints a 256-bit API token as hex.
///
/// Built from two v4 UUIDs, whose random bytes come from the operating
/// system's cryptographically secure generator.
fn generate_api_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

/// The two sample profiles a new install starts with.
pub(crate) fn default_profiles() -> Vec<Profile> {
    vec![
        Profile {
            id: "profile-a".into(),
            name: "Container A (NYC)".into(),
            container_url: "http://localhost:4444".into(),
            engine: Engine::WebDriver,
            browser: seleniumbase_rs::Browser::Chrome,
            mode: seleniumbase_rs::DriverMode::WebDriver,
            user_agent: Some("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".into()),
            proxy: None,
            locale: Some("en-US".into()),
            latitude: Some(40.7128),
            longitude: Some(-74.0060),
            accuracy: Some(100.0),
            headless: false,
            tags: vec![],
            folder_id: "default".into(),
            cookies: vec![],
            external_profile: None,
            fingerprint: None,
        },
        Profile {
            id: "profile-b".into(),
            name: "Container B (London)".into(),
            container_url: "http://localhost:4445".into(),
            engine: Engine::WebDriver,
            browser: seleniumbase_rs::Browser::Chrome,
            mode: seleniumbase_rs::DriverMode::WebDriver,
            user_agent: Some("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".into()),
            proxy: None,
            locale: Some("en-GB".into()),
            latitude: Some(51.5074),
            longitude: Some(-0.1278),
            accuracy: Some(100.0),
            headless: false,
            tags: vec![],
            folder_id: "default".into(),
            cookies: vec![],
            external_profile: None,
            fingerprint: None,
        },
    ]
}

pub fn build_config(profile: &Profile) -> BrowserConfig {
    let mut config = if let Some(params) = profile.external_profile.as_ref() {
        params.to_browser_config(&profile.container_url)
    } else {
        BrowserConfig {
            webdriver_url: profile.container_url.clone(),
            browser: profile.browser,
            headless: profile.headless,
            mode: profile.mode,
            user_agent: profile.user_agent.clone(),
            proxy: profile.proxy.clone(),
            locale: profile.locale.clone(),
            auto_start_driver: false,
            ..BrowserConfig::default()
        }
    };
    // Explicit profile fingerprint overrides the external profile payload.
    if let Some(fingerprint) = profile.fingerprint.as_ref() {
        config.fingerprint = Some(fingerprint.clone());
    }
    config
}

pub fn next_api_port() -> u16 {
    45001
}

pub fn make_session_id() -> String {
    Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::NewProfile;
    use crate::passphrase::memory::MemorySecretStore;
    use crate::storage::{open_storage, LoadedData};

    const FAKE_PASSPHRASE: &str = "fake-passphrase-for-tests";

    fn new_profile(id: &str, name: &str) -> Profile {
        let new: NewProfile = serde_json::from_value(serde_json::json!({
            "name": name,
            "container_url": "http://localhost:4444",
        }))
        .unwrap();
        Profile::from_new(id.to_owned(), new).unwrap()
    }

    async fn vault_state(dir: &std::path::Path) -> AppState {
        let keychain = MemorySecretStore::empty();
        let opened = open_storage(dir, Some(FAKE_PASSPHRASE), &keychain).await;
        assert!(matches!(opened.state, StorageState::Ready { .. }));
        AppState::from_opened(opened)
    }

    #[tokio::test]
    async fn the_vault_is_written_before_the_list_so_a_failed_write_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let state = vault_state(dir.path()).await;
        let before = state.profiles.lock().await.len();

        // The vault caps ids at 256 bytes.
        let error = state
            .add_profile(new_profile(&"x".repeat(300), "Too long"))
            .await
            .unwrap_err();

        assert!(matches!(error, PersistError::Failed(_)), "{error}");
        assert_eq!(state.profiles.lock().await.len(), before);
    }

    #[tokio::test]
    async fn the_same_os_and_seed_give_the_same_coherent_identity() {
        let state = AppState::new();
        state.add_profile(new_profile("a", "A")).await.unwrap();
        state.add_profile(new_profile("b", "B")).await.unwrap();
        let request = RandomizeRequest {
            os: Some(OsType::Macos),
            seed: Some(7),
        };

        let first = state
            .randomize_fingerprint("a", request.clone())
            .await
            .unwrap()
            .unwrap();
        let second = state
            .randomize_fingerprint("b", request)
            .await
            .unwrap()
            .unwrap();

        let fingerprint = first.profile.fingerprint.as_ref().unwrap();
        assert!(fingerprint.validate().is_coherent());
        assert_eq!(first.profile.fingerprint, second.profile.fingerprint);
        assert_eq!((first.os, first.seed), (OsType::Macos, 7));
        assert_eq!(
            state.profiles.lock().await[0].fingerprint,
            first.profile.fingerprint,
            "the stored profile must carry the new identity"
        );
    }

    #[tokio::test]
    async fn without_a_request_this_machines_os_and_a_fresh_seed_are_used() {
        let state = AppState::new();
        state.add_profile(new_profile("a", "A")).await.unwrap();

        let first = state
            .randomize_fingerprint("a", RandomizeRequest::default())
            .await
            .unwrap()
            .unwrap();
        let second = state
            .randomize_fingerprint("a", RandomizeRequest::default())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(first.os, host_os());
        assert_ne!(first.seed, second.seed, "each request draws its own seed");
        for seed in [first.seed, second.seed] {
            assert!(seed < 1 << 53, "{seed} would lose digits in JavaScript");
        }
    }

    #[tokio::test]
    async fn a_randomized_identity_survives_the_vault() {
        let dir = tempfile::tempdir().unwrap();
        let state = vault_state(dir.path()).await;
        state.add_profile(new_profile("a", "A")).await.unwrap();
        let randomized = state
            .randomize_fingerprint(
                "a",
                RandomizeRequest {
                    os: Some(OsType::Windows),
                    seed: Some(99),
                },
            )
            .await
            .unwrap()
            .unwrap();
        drop(state);

        let reopened = vault_state(dir.path()).await;

        let profiles = reopened.profiles.lock().await;
        let stored = profiles.iter().find(|p| p.id == "a").unwrap();
        assert_eq!(stored.fingerprint, randomized.profile.fingerprint);
    }

    #[tokio::test]
    async fn randomizing_a_missing_profile_finds_nothing() {
        let state = AppState::new();

        let outcome = state
            .randomize_fingerprint("ghost", RandomizeRequest::default())
            .await;

        assert_eq!(outcome.unwrap().map(|r| r.seed), None);
    }

    #[tokio::test]
    async fn an_unavailable_store_refuses_to_randomize() {
        let state = AppState::from_opened(Opened {
            state: StorageState::Unavailable {
                message: "locked".to_owned(),
            },
            data: LoadedData::default(),
        });

        let error = state
            .randomize_fingerprint("a", RandomizeRequest::default())
            .await
            .unwrap_err();

        assert_eq!(error, PersistError::Unavailable("locked".to_owned()));
    }

    #[tokio::test]
    async fn a_second_profile_with_the_same_id_is_refused() {
        let state = AppState::new();
        state
            .add_profile(new_profile("same", "First"))
            .await
            .unwrap();

        let error = state
            .add_profile(new_profile("same", "Second"))
            .await
            .unwrap_err();

        assert!(matches!(error, PersistError::Failed(_)), "{error}");
        let profiles = state.profiles.lock().await;
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].name, "First");
    }

    #[tokio::test]
    async fn updating_or_deleting_something_missing_reports_false_and_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let state = vault_state(dir.path()).await;

        assert!(!state
            .update_profile(new_profile("ghost", "Ghost"))
            .await
            .unwrap());
        assert!(!state.delete_profile("ghost").await.unwrap());
        assert!(!state.delete_tag("ghost").await.unwrap());
        assert!(!state.delete_folder("ghost").await.unwrap());

        drop(state);
        let again = vault_state(dir.path()).await;
        assert!(again.profiles.lock().await.iter().all(|p| p.id != "ghost"));
    }

    #[tokio::test]
    async fn an_unavailable_store_refuses_every_change() {
        let state = AppState::from_opened(Opened {
            state: StorageState::Unavailable {
                message: "locked".to_owned(),
            },
            data: LoadedData::default(),
        });

        let unavailable = PersistError::Unavailable("locked".to_owned());
        assert_eq!(state.ensure_storage(), Err(unavailable.clone()));
        assert_eq!(
            state.add_profile(new_profile("p", "P")).await,
            Err(unavailable.clone())
        );
        assert_eq!(
            state.update_profile(new_profile("p", "P")).await,
            Err(unavailable.clone())
        );
        assert_eq!(state.delete_profile("p").await, Err(unavailable));
        assert!(state.profiles.lock().await.is_empty());
        assert!(!state.storage_status().ok);
    }
}
