use std::path::PathBuf;
use std::sync::Arc;

use serde_json::json;
use seleniumbase_rs::init_tracing;
use tauri::{command, generate_context, generate_handler, AppHandle, Manager, State};
use tracing::info;

mod api;
mod models;
mod store;

use models::{NewProfile, Profile, SessionInfo};
use store::{
    apply_profile_overrides, build_config, load_all, next_api_port, save_profiles, AppState,
};

#[command]
async fn list_profiles(state: State<'_, Arc<AppState>>) -> Result<Vec<Profile>, String> {
    let profiles = state.profiles.lock().await.clone();
    info!(count = profiles.len(), "listed profiles");
    Ok(profiles)
}

#[command]
async fn create_profile(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    new: NewProfile,
) -> Result<Profile, String> {
    let profile = Profile {
        id: uuid::Uuid::new_v4().to_string(),
        name: new.name,
        container_url: new.container_url,
        browser: new.browser,
        mode: new.mode,
        user_agent: new.user_agent,
        proxy: new.proxy,
        locale: new.locale,
        latitude: new.latitude,
        longitude: new.longitude,
        accuracy: new.accuracy,
        headless: new.headless,
        tags: new.tags,
        folder_id: if new.folder_id.is_empty() {
            "default".into()
        } else {
            new.folder_id
        },
        cookies: vec![],
        external_profile: new.external_profile,
    };
    {
        let mut profiles = state.profiles.lock().await;
        profiles.push(profile.clone());
    }
    let profiles = state.profiles.lock().await.clone();
    save_profiles(&app, &profiles).await?;
    info!(profile_id = %profile.id, name = %profile.name, "created profile");
    Ok(profile)
}

#[command]
async fn delete_profile(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    {
        let mut profiles = state.profiles.lock().await;
        profiles.retain(|p| p.id != id);
    }
    let profiles = state.profiles.lock().await.clone();
    save_profiles(&app, &profiles).await?;
    info!(profile_id = %id, "deleted profile");
    Ok(())
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

    let config = build_config(&profile);
    let mut sb = seleniumbase_rs::BaseCase::new(config)
        .await
        .map_err(|e| e.to_string())?;
    apply_profile_overrides(&mut sb, &profile).await?;

    if let Some(url) = start_url {
        sb.open(&url).await.map_err(|e| e.to_string())?;
    }

    let session_id = store::make_session_id();
    let info = SessionInfo {
        session_id: session_id.clone(),
        profile_id: profile.id,
        profile_name: profile.name,
        container_url: profile.container_url,
    };

    state.sessions.lock().await.insert(session_id.clone(), sb);
    state
        .session_info
        .lock()
        .await
        .insert(session_id, info.clone());
    info!(session_id = %info.session_id, profile_id = %info.profile_id, "launched profile");
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
    let sb = sessions
        .get_mut(&session_id)
        .ok_or_else(|| "Session not found".to_string())?;
    sb.open(&url).await.map_err(|e| e.to_string())
}

#[command]
async fn take_screenshot(
    state: State<'_, Arc<AppState>>,
    session_id: String,
) -> Result<PathBuf, String> {
    let mut sessions = state.sessions.lock().await;
    let sb = sessions
        .get_mut(&session_id)
        .ok_or_else(|| "Session not found".to_string())?;
    sb.save_screenshot_to_logs()
        .await
        .map_err(|e| e.to_string())
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
    let sb = sessions
        .get_mut(&session_id)
        .ok_or_else(|| "Session not found".to_string())?;
    let params = json!({
        "latitude": latitude,
        "longitude": longitude,
        "accuracy": accuracy.unwrap_or(100.0),
    });
    sb.execute_cdp_with_params("Emulation.setGeolocationOverride", params)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[command]
async fn close_session(state: State<'_, Arc<AppState>>, session_id: String) -> Result<(), String> {
    let mut sessions = state.sessions.lock().await;
    let sb = sessions
        .remove(&session_id)
        .ok_or_else(|| "Session not found".to_string())?;
    sb.quit().await.map_err(|e| e.to_string())?;
    state.session_info.lock().await.remove(&session_id);
    info!(session_id = %session_id, "closed session");
    Ok(())
}

#[command]
async fn get_api_base() -> Result<String, String> {
    Ok(format!("http://127.0.0.1:{}", next_api_port()))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let state = Arc::new(AppState::new());
            app.manage(state.clone());

            std::thread::spawn(move || {
                actix_web::rt::System::new().block_on(async move {
                    load_all(&handle, &state).await;
                    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], next_api_port()));
                    if let Err(e) = api::start_server(state.clone(), addr).await {
                        eprintln!("External profile API server error: {e}");
                    }
                });
            });
            Ok(())
        })
        .invoke_handler(generate_handler![
            list_profiles,
            create_profile,
            delete_profile,
            launch_profile,
            list_sessions,
            navigate_session,
            take_screenshot,
            set_session_geolocation,
            close_session,
            get_api_base,
        ])
        .run(generate_context!())
        .expect("error while running tauri application");
}
