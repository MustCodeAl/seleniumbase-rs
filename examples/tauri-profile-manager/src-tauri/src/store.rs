use std::collections::HashMap;
use std::fmt;

use serde_json::json;
use tokio::sync::Mutex;
use uuid::Uuid;

use seleniumbase_rs::{BaseCase, BrowserConfig};

use crate::models::{BrowserCookie, Folder, Profile, SessionInfo, StorageStatus, Tag};
use crate::storage::{ensure_default_folder, Opened, ProfileStorage, StorageError, StorageState};

pub struct AppState {
    pub profiles: Mutex<Vec<Profile>>,
    pub sessions: Mutex<HashMap<String, BaseCase>>,
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

    /// Adds a profile.
    pub async fn add_profile(&self, profile: Profile) -> Result<(), PersistError> {
        let storage = self.writer()?;
        let mut profiles = self.profiles.lock().await;
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

pub async fn apply_profile_overrides(sb: &mut BaseCase, profile: &Profile) -> Result<(), String> {
    // Prefer External profile-style fingerprint values when present, falling back to
    // the flat profile fields for backward compatibility.
    let geo = profile
        .external_profile
        .as_ref()
        .and_then(|p| p.parameters.fingerprint.geolocation.as_ref())
        .map(|g| (g.latitude, g.longitude, g.accuracy));

    let (lat, lon, accuracy) = match geo {
        Some((lat, lon, acc)) => (Some(lat), Some(lon), Some(acc)),
        None => (profile.latitude, profile.longitude, profile.accuracy),
    };

    if let (Some(lat), Some(lon)) = (lat, lon) {
        let params = json!({
            "latitude": lat,
            "longitude": lon,
            "accuracy": accuracy.unwrap_or(100.0),
        });
        sb.execute_cdp_with_params("Emulation.setGeolocationOverride", params)
            .await
            .map_err(|e| format!("Failed to set geolocation: {e}"))?;
    }

    if let Some(screen) = profile
        .external_profile
        .as_ref()
        .and_then(|p| p.parameters.fingerprint.screen.as_ref())
    {
        sb.set_window_size(screen.width, screen.height)
            .await
            .map_err(|e| format!("Failed to set screen size: {e}"))?;
    }

    Ok(())
}

pub async fn set_cookies(sb: &mut BaseCase, cookies: &[BrowserCookie]) -> Result<(), String> {
    let cdp_cookies: Vec<serde_json::Value> = cookies
        .iter()
        .map(|c| {
            json!({
                "name": c.name,
                "value": c.value,
                "domain": c.domain,
                "path": c.path,
                "secure": c.secure,
                "httpOnly": c.http_only,
                "sameSite": c.same_site,
                "expires": c.expires,
            })
        })
        .collect();
    sb.execute_cdp_with_params("Network.setCookies", json!({ "cookies": cdp_cookies }))
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
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
