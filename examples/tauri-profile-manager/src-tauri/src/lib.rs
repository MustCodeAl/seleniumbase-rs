use std::path::PathBuf;
use std::sync::Arc;

use seleniumbase_rs::init_tracing;
use tauri::{command, generate_context, generate_handler, Manager, State};
use tracing::{error, info};

mod api;
mod models;
mod passphrase;
mod session;
mod storage;
mod store;

use models::{NewProfile, Profile, RandomizeRequest, Randomized, SessionInfo, StorageStatus};
use seleniumbase_rs::OsType;
use session::Session;
use store::{next_api_port, AppState};

/// Whether the profile vault opened, for the window to explain when it did not.
#[command]
fn get_storage_status(state: State<'_, Arc<AppState>>) -> StorageStatus {
    state.storage_status()
}

#[command]
async fn list_profiles(state: State<'_, Arc<AppState>>) -> Result<Vec<Profile>, String> {
    state.ensure_storage().map_err(|e| e.to_string())?;
    let profiles = state.profiles.lock().await.clone();
    info!(count = profiles.len(), "listed profiles");
    Ok(profiles)
}

#[command]
async fn create_profile(
    state: State<'_, Arc<AppState>>,
    new: NewProfile,
) -> Result<Profile, String> {
    let profile = Profile::from_new(uuid::Uuid::new_v4().to_string(), new)?;
    state
        .add_profile(profile.clone())
        .await
        .map_err(|e| e.to_string())?;
    info!(profile_id = %profile.id, name = %profile.name, "created profile");
    Ok(profile)
}

#[command]
async fn delete_profile(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    state.delete_profile(&id).await.map_err(|e| e.to_string())?;
    info!(profile_id = %id, "deleted profile");
    Ok(())
}

/// Gives a profile a new randomized identity; see
/// [`AppState::randomize_fingerprint`].
#[command]
async fn randomize_fingerprint(
    state: State<'_, Arc<AppState>>,
    id: String,
    os: Option<OsType>,
    seed: Option<u64>,
) -> Result<Randomized, String> {
    let randomized = state
        .randomize_fingerprint(&id, RandomizeRequest { os, seed })
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Profile not found".to_string())?;
    info!(profile_id = %id, os = ?randomized.os, "randomized fingerprint");
    Ok(randomized)
}

#[command]
async fn launch_profile(
    state: State<'_, Arc<AppState>>,
    id: String,
    start_url: Option<String>,
) -> Result<SessionInfo, String> {
    let profile = {
        let profiles = state.profiles.lock().await;
        profiles
            .iter()
            .find(|p| p.id == id)
            .cloned()
            .ok_or_else(|| "Profile not found".to_string())?
    };

    let session = Session::launch(&profile, start_url.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    let session_id = store::make_session_id();
    let info = SessionInfo {
        session_id: session_id.clone(),
        profile_id: profile.id,
        profile_name: profile.name,
        container_url: profile.container_url,
        engine: profile.engine,
    };

    state
        .sessions
        .lock()
        .await
        .insert(session_id.clone(), session);
    state
        .session_info
        .lock()
        .await
        .insert(session_id, info.clone());
    info!(
        session_id = %info.session_id,
        profile_id = %info.profile_id,
        engine = ?info.engine,
        "launched profile"
    );
    Ok(info)
}

#[command]
async fn list_sessions(state: State<'_, Arc<AppState>>) -> Result<Vec<SessionInfo>, String> {
    Ok(state.session_info.lock().await.values().cloned().collect())
}

#[command]
async fn navigate_session(
    state: State<'_, Arc<AppState>>,
    session_id: String,
    url: String,
) -> Result<(), String> {
    let mut sessions = state.sessions.lock().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| "Session not found".to_string())?;
    session.open(&url).await
}

#[command]
async fn take_screenshot(
    state: State<'_, Arc<AppState>>,
    session_id: String,
) -> Result<PathBuf, String> {
    let mut sessions = state.sessions.lock().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| "Session not found".to_string())?;
    session.screenshot().await
}

#[command]
async fn set_session_geolocation(
    state: State<'_, Arc<AppState>>,
    session_id: String,
    latitude: f64,
    longitude: f64,
    accuracy: Option<f64>,
) -> Result<(), String> {
    let mut sessions = state.sessions.lock().await;
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| "Session not found".to_string())?;
    session
        .set_geolocation(latitude, longitude, accuracy.unwrap_or(100.0))
        .await
}

#[command]
async fn close_session(state: State<'_, Arc<AppState>>, session_id: String) -> Result<(), String> {
    // Take the session out first, so closing a slow browser does not hold up
    // every other session.
    let mut session = state
        .sessions
        .lock()
        .await
        .remove(&session_id)
        .ok_or_else(|| "Session not found".to_string())?;
    state.session_info.lock().await.remove(&session_id);
    session.quit().await?;
    info!(session_id = %session_id, "closed session");
    Ok(())
}

#[command]
async fn get_api_base() -> Result<String, String> {
    Ok(format!("http://127.0.0.1:{}", next_api_port()))
}

/// Hands this run's API token to the app window.
///
/// Reaching the token requires Tauri IPC, which is available only to the
/// app's own frontend. A web page can send requests to the loopback API but
/// has no way to obtain the token they must carry.
#[command]
async fn get_api_token(state: State<'_, Arc<AppState>>) -> Result<String, String> {
    Ok(state.api_token.clone())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Open the vault before anything can ask for a profile. This takes
            // a fraction of a second, because deriving the key is meant to be
            // slow.
            let data_dir = app.path().app_data_dir()?;
            let opened = tauri::async_runtime::block_on(storage::open_default(&data_dir));
            let state = Arc::new(AppState::from_opened(opened));
            app.manage(state.clone());

            std::thread::spawn(move || {
                actix_web::rt::System::new().block_on(async move {
                    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], next_api_port()));
                    if let Err(e) = api::start_server(state, addr).await {
                        error!(error = %e, "external profile API server stopped");
                    }
                });
            });
            Ok(())
        })
        .invoke_handler(generate_handler![
            get_storage_status,
            list_profiles,
            create_profile,
            delete_profile,
            randomize_fingerprint,
            launch_profile,
            list_sessions,
            navigate_session,
            take_screenshot,
            set_session_geolocation,
            close_session,
            get_api_base,
            get_api_token,
        ])
        .run(generate_context!())
        .expect("error while running tauri application");
}
