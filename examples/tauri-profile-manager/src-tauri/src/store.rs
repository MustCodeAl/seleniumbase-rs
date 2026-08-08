use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::json;
use tauri::AppHandle;
use tauri::Manager;
use tokio::sync::Mutex;
use uuid::Uuid;

use seleniumbase_rs::{BaseCase, BrowserConfig};

use crate::models::{BrowserCookie, Folder, Profile, SessionInfo, Tag};

pub struct AppState {
    pub profiles: Mutex<Vec<Profile>>,
    pub sessions: Mutex<HashMap<String, BaseCase>>,
    pub session_info: Mutex<HashMap<String, SessionInfo>>,
    pub tags: Mutex<Vec<Tag>>,
    pub folders: Mutex<Vec<Folder>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            profiles: Mutex::new(Vec::new()),
            sessions: Mutex::new(HashMap::new()),
            session_info: Mutex::new(HashMap::new()),
            tags: Mutex::new(Vec::new()),
            folders: Mutex::new(vec![Folder {
                id: "default".into(),
                name: "Default".into(),
            }]),
        }
    }
}

fn profile_path(app: &AppHandle) -> PathBuf {
    let dir = app.path().app_data_dir().expect("app data dir");
    std::fs::create_dir_all(&dir).ok();
    dir.join("profiles.json")
}

fn tags_path(app: &AppHandle) -> PathBuf {
    let dir = app.path().app_data_dir().expect("app data dir");
    dir.join("tags.json")
}

fn folders_path(app: &AppHandle) -> PathBuf {
    let dir = app.path().app_data_dir().expect("app data dir");
    dir.join("folders.json")
}

pub async fn load_all(app: &AppHandle, state: &AppState) {
    let profiles = load_json::<Vec<Profile>>(profile_path(app))
        .await
        .unwrap_or_else(|_| default_profiles());
    *state.profiles.lock().await = profiles;

    if let Ok(tags) = load_json::<Vec<Tag>>(tags_path(app)).await {
        *state.tags.lock().await = tags;
    }

    if let Ok(folders) = load_json::<Vec<Folder>>(folders_path(app)).await {
        *state.folders.lock().await = folders;
    }
}

async fn load_json<T: serde::de::DeserializeOwned>(path: PathBuf) -> Result<T, String> {
    if !path.exists() {
        return Err("file not found".into());
    }
    let data = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::from_str(&data).map_err(|e| e.to_string())
}

pub async fn save_profiles(app: &AppHandle, profiles: &[Profile]) -> Result<(), String> {
    let path = profile_path(app);
    let data = serde_json::to_string_pretty(profiles).map_err(|e| e.to_string())?;
    tokio::fs::write(&path, data)
        .await
        .map_err(|e| e.to_string())
}

#[allow(dead_code)]
pub async fn save_tags(app: &AppHandle, tags: &[Tag]) -> Result<(), String> {
    let path = tags_path(app);
    let data = serde_json::to_string_pretty(tags).map_err(|e| e.to_string())?;
    tokio::fs::write(&path, data)
        .await
        .map_err(|e| e.to_string())
}

#[allow(dead_code)]
pub async fn save_folders(app: &AppHandle, folders: &[Folder]) -> Result<(), String> {
    let path = folders_path(app);
    let data = serde_json::to_string_pretty(folders).map_err(|e| e.to_string())?;
    tokio::fs::write(&path, data)
        .await
        .map_err(|e| e.to_string())
}

fn default_profiles() -> Vec<Profile> {
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
