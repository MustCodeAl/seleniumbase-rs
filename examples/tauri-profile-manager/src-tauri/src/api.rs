#![allow(clippy::result_large_err)]

use std::collections::HashMap;
use std::sync::Arc;

use actix_cors::Cors;
use actix_web::{
    delete, get, http::StatusCode, post, web, App, HttpResponse, HttpServer, ResponseError,
};
use serde::Serialize;
use serde_json::{json, Value};
use tracing::info;
use uuid::Uuid;

use seleniumbase_rs::profile_payloads::ProfileParams;
use seleniumbase_rs::BaseCase;

use crate::models::*;
use crate::store::{apply_profile_overrides, build_config, make_session_id, set_cookies, AppState};

#[derive(Debug)]
pub struct ApiErrorResponse {
    status: StatusCode,
    body: ApiResponse<Value>,
}

impl std::fmt::Display for ApiErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            serde_json::to_string(&self.body).unwrap_or_default()
        )
    }
}

impl ResponseError for ApiErrorResponse {
    fn status_code(&self) -> StatusCode {
        self.status
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status).json(&self.body)
    }
}

pub type ApiResult = Result<HttpResponse, ApiErrorResponse>;

fn err(code: u16, msg: impl Into<String>) -> ApiResult {
    let status = ApiStatus {
        error_code: "ERROR".into(),
        http_code: code,
        message: msg.into(),
    };
    Err(ApiErrorResponse {
        status: StatusCode::from_u16(code).unwrap_or(StatusCode::BAD_REQUEST),
        body: ApiResponse::err(status),
    })
}

fn ok<T: Serialize>(data: T) -> ApiResult {
    Ok(HttpResponse::Ok().json(ApiResponse::ok(data)))
}

fn ok_msg<T: Serialize>(data: T, msg: impl Into<String>) -> ApiResult {
    Ok(HttpResponse::Ok().json(ApiResponse::ok_msg(data, msg)))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(version)
        .service(status_all)
        .service(profile_status)
        .service(profile_search)
        .service(profile_create)
        .service(profile_get)
        .service(profile_update)
        .service(profile_delete)
        .service(profile_start)
        .service(profile_stop)
        .service(profile_clone)
        .service(profile_export)
        .service(profile_import)
        .service(cookie_import)
        .service(cookie_export)
        .service(proxy_validate)
        .service(tag_list)
        .service(tag_create)
        .service(tag_update)
        .service(tag_delete)
        .service(folder_list)
        .service(folder_create)
        .service(folder_update)
        .service(folder_delete)
        .service(screen_resolution)
        .service(script_runner_start)
        .service(script_runner_stop)
        .service(browser_core_list)
        .service(load_browser_core)
        .service(delete_browser_core)
        .service(stop_all)
        .service(workspaces)
        .service(user_signin)
        .service(user_refresh_token)
        .service(bookmarks_export)
        .service(bookmarks_import)
        .service(twofa_setup)
        .service(twofa_enable);
}

pub async fn start_server(state: Arc<AppState>, addr: std::net::SocketAddr) -> std::io::Result<()> {
    info!(%addr, "starting external profile api server");
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .wrap(Cors::permissive())
            .configure(configure)
    })
    .bind(addr)?
    .run()
    .await
}

#[get("/api/v1/version")]
async fn version() -> ApiResult {
    ok_msg(
        json!({ "version": "0.1.0", "launcher": "seleniumbase-rs" }),
        "",
    )
}

#[get("/api/v1/status")]
async fn status_all(state: web::Data<Arc<AppState>>) -> ApiResult {
    let sessions = state.session_info.lock().await.clone();
    ok_msg(json!({ "sessions": sessions }), "")
}

#[get("/api/v1/profile_status")]
async fn profile_status(
    state: web::Data<Arc<AppState>>,
    query: web::Query<HashMap<String, String>>,
) -> ApiResult {
    let id = query.get("profile_id").cloned().unwrap_or_default();
    let active = state
        .session_info
        .lock()
        .await
        .values()
        .any(|s| s.profile_id == id);
    ok_msg(json!({ "profile_id": id, "active": active }), "")
}

#[get("/api/v1/profiles")]
async fn profile_search(state: web::Data<Arc<AppState>>) -> ApiResult {
    let profiles = state.profiles.lock().await.clone();
    ok(profiles)
}

#[post("/api/v1/profiles")]
async fn profile_create(
    state: web::Data<Arc<AppState>>,
    payload: web::Json<NewProfile>,
) -> ApiResult {
    let payload = payload.into_inner();
    let profile = Profile {
        id: Uuid::new_v4().to_string(),
        name: payload.name,
        container_url: payload.container_url,
        browser: payload.browser,
        mode: payload.mode,
        user_agent: payload.user_agent,
        proxy: payload.proxy,
        locale: payload.locale,
        latitude: payload.latitude,
        longitude: payload.longitude,
        accuracy: payload.accuracy,
        headless: payload.headless,
        tags: payload.tags,
        folder_id: if payload.folder_id.is_empty() {
            "default".into()
        } else {
            payload.folder_id
        },
        cookies: vec![],
        external_profile: payload.external_profile,
        fingerprint: payload.fingerprint,
    };
    state.profiles.lock().await.push(profile.clone());
    info!(profile_id = %profile.id, name = %profile.name, "created profile via api");
    ok_msg(profile, "Profile created")
}

#[get("/api/v1/profiles/{id}")]
async fn profile_get(state: web::Data<Arc<AppState>>, path: web::Path<String>) -> ApiResult {
    let id = path.into_inner();
    let profiles = state.profiles.lock().await;
    match profiles.iter().find(|p| p.id == id).cloned() {
        Some(p) => ok(p),
        None => err(404, "Profile not found"),
    }
}

#[post("/api/v1/profiles/{id}")]
async fn profile_update(
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    payload: web::Json<Value>,
) -> ApiResult {
    let id = path.into_inner();
    let mut profiles = state.profiles.lock().await;
    let Some(idx) = profiles.iter().position(|p| p.id == id) else {
        return err(404, "Profile not found");
    };
    let payload = payload.into_inner();
    if let Some(v) = payload.get("name").and_then(|v| v.as_str()) {
        profiles[idx].name = v.to_owned();
    }
    if let Some(v) = payload.get("container_url").and_then(|v| v.as_str()) {
        profiles[idx].container_url = v.to_owned();
    }
    if let Some(v) = payload.get("browser").and_then(|v| v.as_str()) {
        if let Ok(browser) = serde_json::from_value::<seleniumbase_rs::Browser>(json!(v)) {
            profiles[idx].browser = browser;
        }
    }
    if let Some(v) = payload.get("mode").and_then(|v| v.as_str()) {
        if let Ok(mode) = serde_json::from_value::<seleniumbase_rs::DriverMode>(json!(v)) {
            profiles[idx].mode = mode;
        }
    }
    if let Some(v) = payload.get("folder_id").and_then(|v| v.as_str()) {
        profiles[idx].folder_id = v.to_owned();
    }
    if let Some(v) = payload.get("user_agent").and_then(|v| v.as_str()) {
        profiles[idx].user_agent = Some(v.to_owned());
    }
    if let Some(v) = payload.get("proxy").and_then(|v| v.as_str()) {
        profiles[idx].proxy = Some(v.to_owned());
    }
    if let Some(v) = payload.get("locale").and_then(|v| v.as_str()) {
        profiles[idx].locale = Some(v.to_owned());
    }
    if let Some(v) = payload.get("latitude").and_then(|v| v.as_f64()) {
        profiles[idx].latitude = Some(v);
    }
    if let Some(v) = payload.get("longitude").and_then(|v| v.as_f64()) {
        profiles[idx].longitude = Some(v);
    }
    if let Some(v) = payload.get("accuracy").and_then(|v| v.as_f64()) {
        profiles[idx].accuracy = Some(v);
    }
    if let Some(v) = payload.get("headless").and_then(|v| v.as_bool()) {
        profiles[idx].headless = v;
    }
    if let Some(v) = payload.get("tags").and_then(|v| v.as_array()) {
        profiles[idx].tags = v
            .iter()
            .filter_map(|x| x.as_str().map(String::from))
            .collect();
    }
    if let Some(v) = payload.get("fingerprint") {
        if v.is_null() {
            profiles[idx].fingerprint = None;
        } else {
            match serde_json::from_value::<seleniumbase_rs::Fingerprint>(v.clone()) {
                Ok(fp) => profiles[idx].fingerprint = Some(fp),
                Err(e) => {
                    return err(400, format!("Invalid fingerprint payload: {e}"));
                }
            }
        }
    }
    if payload.get("parameters").is_some() {
        match serde_json::from_value::<seleniumbase_rs::profile_payloads::ProfileParams>(
            payload.get("parameters").cloned().unwrap_or_default(),
        ) {
            Ok(params) => profiles[idx].external_profile = Some(params),
            Err(e) => return err(400, format!("Invalid parameters: {e}")),
        }
    }
    let p = profiles[idx].clone();
    ok(p)
}

#[delete("/api/v1/profiles/{id}")]
async fn profile_delete(state: web::Data<Arc<AppState>>, path: web::Path<String>) -> ApiResult {
    let id = path.into_inner();
    let mut profiles = state.profiles.lock().await;
    let before = profiles.len();
    profiles.retain(|p| p.id != id);
    if profiles.len() == before {
        return err(404, "Profile not found");
    }
    ok_msg(json!({ "deleted": true }), "Profile removed")
}

#[get("/api/v1/profiles/{id}/start")]
async fn profile_start(
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    query: web::Query<HashMap<String, String>>,
) -> ApiResult {
    let id = path.into_inner();
    let profile = {
        let profiles = state.profiles.lock().await;
        profiles.iter().find(|p| p.id == id).cloned()
    };
    let Some(profile) = profile else {
        return err(404, "Profile not found");
    };

    let config = build_config(&profile);
    let mut sb = BaseCase::new(config).await.map_err(|e| ApiErrorResponse {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        body: ApiResponse::err(ApiStatus::err("LAUNCH_FAILED", e.to_string())),
    })?;
    apply_profile_overrides(&mut sb, &profile)
        .await
        .map_err(|e| ApiErrorResponse {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            body: ApiResponse::err(ApiStatus::err("OVERRIDE_FAILED", e)),
        })?;
    if !profile.cookies.is_empty() {
        set_cookies(&mut sb, &profile.cookies)
            .await
            .map_err(|e| ApiErrorResponse {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                body: ApiResponse::err(ApiStatus::err("COOKIE_FAILED", e)),
            })?;
    }
    if let Some(url) = query.get("url") {
        sb.open(url).await.map_err(|e| ApiErrorResponse {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            body: ApiResponse::err(ApiStatus::err("OPEN_FAILED", e.to_string())),
        })?;
    }

    let session_id = make_session_id();
    let info = SessionInfo {
        session_id: session_id.clone(),
        profile_id: profile.id.clone(),
        profile_name: profile.name.clone(),
        container_url: profile.container_url.clone(),
    };
    state.sessions.lock().await.insert(session_id.clone(), sb);
    state
        .session_info
        .lock()
        .await
        .insert(session_id.clone(), info);
    info!(session_id = %session_id, profile_id = %profile.id, "started profile via api");

    let port: u16 = profile
        .container_url
        .rsplit(':')
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4444);

    ok_msg(
        StartProfileData {
            profile_id: profile.id,
            session_id,
            port,
            ws_endpoint: profile.container_url.clone(),
            message: "Profile started".into(),
        },
        "Profile started",
    )
}

#[get("/api/v1/profiles/{id}/stop")]
async fn profile_stop(state: web::Data<Arc<AppState>>, path: web::Path<String>) -> ApiResult {
    let id = path.into_inner();
    let session_id = {
        let infos = state.session_info.lock().await;
        infos
            .values()
            .find(|s| s.profile_id == id)
            .map(|s| s.session_id.clone())
    };
    let Some(session_id) = session_id else {
        return err(404, "No active session for profile");
    };
    let mut sessions = state.sessions.lock().await;
    let mut sb = sessions
        .remove(&session_id)
        .ok_or_else(|| ApiErrorResponse {
            status: StatusCode::NOT_FOUND,
            body: ApiResponse::err(ApiStatus::err("NOT_FOUND", "Session not found")),
        })?;
    sb.quit().await.map_err(|e| ApiErrorResponse {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        body: ApiResponse::err(ApiStatus::err("QUIT_FAILED", e.to_string())),
    })?;
    state.session_info.lock().await.remove(&session_id);
    info!(session_id = %session_id, "stopped profile via api");
    ok_msg(json!({ "stopped": true }), "Profile stopped")
}

#[post("/api/v1/profiles/{id}/clone")]
async fn profile_clone(state: web::Data<Arc<AppState>>, path: web::Path<String>) -> ApiResult {
    let id = path.into_inner();
    let mut profiles = state.profiles.lock().await;
    let Some(source) = profiles.iter().find(|p| p.id == id).cloned() else {
        return err(404, "Profile not found");
    };
    let mut clone = source;
    clone.id = Uuid::new_v4().to_string();
    clone.name = format!("{} (clone)", clone.name);
    profiles.push(clone.clone());
    info!(source_id = %id, clone_id = %clone.id, "cloned profile via api");
    ok_msg(clone, "Profile cloned")
}

#[get("/api/v1/profiles/{id}/export")]
async fn profile_export(state: web::Data<Arc<AppState>>, path: web::Path<String>) -> ApiResult {
    let id = path.into_inner();
    let profiles = state.profiles.lock().await;
    match profiles.iter().find(|p| p.id == id).cloned() {
        Some(p) => ok_msg(
            serde_json::to_value(p).unwrap_or_default(),
            "Profile exported",
        ),
        None => err(404, "Profile not found"),
    }
}

#[post("/api/v1/profiles/import")]
async fn profile_import(state: web::Data<Arc<AppState>>, payload: web::Json<Value>) -> ApiResult {
    let value = payload.into_inner();
    let profile = if value.get("container_url").is_some() {
        serde_json::from_value::<Profile>(value).map_err(|e| ApiErrorResponse {
            status: StatusCode::BAD_REQUEST,
            body: ApiResponse::err(ApiStatus::err("BAD_REQUEST", e.to_string())),
        })?
    } else if value.get("parameters").is_some() {
        let params: ProfileParams =
            serde_json::from_value(value).map_err(|e| ApiErrorResponse {
                status: StatusCode::BAD_REQUEST,
                body: ApiResponse::err(ApiStatus::err("BAD_REQUEST", e.to_string())),
            })?;
        let geo = params.parameters.fingerprint.geolocation.as_ref();
        Profile {
            id: Uuid::new_v4().to_string(),
            name: params.name.clone(),
            container_url: "http://localhost:4444".into(),
            browser: params.browser(),
            mode: if matches!(params.browser_type.as_str(), "firefox" | "stealthfox") {
                seleniumbase_rs::DriverMode::WebDriver
            } else {
                seleniumbase_rs::DriverMode::Uc
            },
            user_agent: params.user_agent(),
            proxy: params.proxy_string(),
            locale: params.locale(),
            latitude: geo.map(|g| g.latitude),
            longitude: geo.map(|g| g.longitude),
            accuracy: geo.map(|g| g.accuracy),
            headless: false,
            tags: params.tags.clone(),
            folder_id: if params.folder_id.is_empty() {
                "default".into()
            } else {
                params.folder_id.clone()
            },
            cookies: vec![],
            external_profile: Some(params),
            fingerprint: None,
        }
    } else {
        return err(
            400,
            "Unrecognized profile JSON: expected container_url or parameters",
        );
    };
    let mut profiles = state.profiles.lock().await;
    profiles.push(profile.clone());
    ok_msg(profile, "Profile imported")
}

#[post("/api/v1/cookie_import")]
async fn cookie_import(
    state: web::Data<Arc<AppState>>,
    payload: web::Json<CookieImportRequest>,
) -> ApiResult {
    let payload = payload.into_inner();
    let mut profiles = state.profiles.lock().await;
    let Some(idx) = profiles.iter().position(|p| p.id == payload.profile_id) else {
        return err(404, "Profile not found");
    };
    profiles[idx].cookies = payload.cookies.clone();

    // Apply to an active session for this profile, if any.
    let session_id = {
        state
            .session_info
            .lock()
            .await
            .values()
            .find(|s| s.profile_id == payload.profile_id)
            .map(|s| s.session_id.clone())
    };
    if let Some(session_id) = session_id {
        let mut sessions = state.sessions.lock().await;
        if let Some(sb) = sessions.get_mut(&session_id) {
            set_cookies(sb, &payload.cookies)
                .await
                .map_err(|e| ApiErrorResponse {
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                    body: ApiResponse::err(ApiStatus::err("COOKIE_FAILED", e)),
                })?;
        }
    }

    ok_msg(
        json!({ "imported": payload.cookies.len() }),
        "Cookies successfully imported",
    )
}

#[post("/api/v1/cookie_export")]
async fn cookie_export(
    state: web::Data<Arc<AppState>>,
    payload: web::Json<CookieExportRequest>,
) -> ApiResult {
    let payload = payload.into_inner();
    let profiles = state.profiles.lock().await;
    let cookies = profiles
        .iter()
        .find(|p| p.id == payload.profile_id)
        .map(|p| p.cookies.clone())
        .unwrap_or_default();
    ok_msg(json!({ "cookies": cookies }), "Cookies exported")
}

fn bad_request(msg: impl Into<String>) -> ApiErrorResponse {
    ApiErrorResponse {
        status: StatusCode::BAD_REQUEST,
        body: ApiResponse::err(ApiStatus::err("BAD_REQUEST", msg)),
    }
}

fn internal_error(msg: impl Into<String>) -> ApiErrorResponse {
    ApiErrorResponse {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        body: ApiResponse::err(ApiStatus::err("INTERNAL_ERROR", msg)),
    }
}

fn bad_gateway(msg: impl Into<String>) -> ApiErrorResponse {
    ApiErrorResponse {
        status: StatusCode::BAD_GATEWAY,
        body: ApiResponse::err(ApiStatus::err("BAD_GATEWAY", msg)),
    }
}

#[post("/api/v1/proxy/validate")]
async fn proxy_validate(payload: web::Json<ProxyValidateRequest>) -> ApiResult {
    let payload = payload.into_inner();
    let proxy_url = if let (Some(u), Some(p)) = (payload.username, payload.password) {
        format!(
            "{}://{}:{}@{}:{}",
            payload.proxy_type, u, p, payload.host, payload.port
        )
    } else {
        format!("{}://{}:{}", payload.proxy_type, payload.host, payload.port)
    };

    let proxy =
        reqwest::Proxy::all(&proxy_url).map_err(|e| bad_request(format!("Invalid proxy: {e}")))?;

    let client = reqwest::Client::builder()
        .proxy(proxy)
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| internal_error(e.to_string()))?;

    let resp = client
        .get("https://ipinfo.io/json")
        .send()
        .await
        .map_err(|e| bad_gateway(e.to_string()))?;

    let data: Value = resp.json().await.map_err(|e| bad_gateway(e.to_string()))?;

    let loc = data.get("loc").and_then(|v| v.as_str()).unwrap_or("0,0");
    let mut parts = loc.split(',');
    let lat = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let lon = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);

    info!(host = %payload.host, port = %payload.port, "validated proxy");
    ok_msg(
        ProxyValidateData {
            ip: data.get("ip").and_then(|v| v.as_str()).unwrap_or("").into(),
            country_code: data
                .get("country")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .into(),
            latitude: lat,
            longitude: lon,
            timezone: data
                .get("timezone")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .into(),
        },
        "",
    )
}

#[get("/api/v1/tags")]
async fn tag_list(state: web::Data<Arc<AppState>>) -> ApiResult {
    ok(state.tags.lock().await.clone())
}

#[post("/api/v1/tags")]
async fn tag_create(
    state: web::Data<Arc<AppState>>,
    payload: web::Json<CreateTagRequest>,
) -> ApiResult {
    let payload = payload.into_inner();
    let tag = Tag {
        id: Uuid::new_v4().to_string(),
        name: payload.name,
        color: payload.color.unwrap_or_else(|| "#396cd8".into()),
    };
    state.tags.lock().await.push(tag.clone());
    ok_msg(tag, "Tag created")
}

#[post("/api/v1/tags/{id}")]
async fn tag_update(
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    payload: web::Json<Value>,
) -> ApiResult {
    let id = path.into_inner();
    let mut tags = state.tags.lock().await;
    let Some(tag) = tags.iter_mut().find(|t| t.id == id) else {
        return err(404, "Tag not found");
    };
    let payload = payload.into_inner();
    if let Some(v) = payload.get("name").and_then(|v| v.as_str()) {
        tag.name = v.to_owned();
    }
    if let Some(v) = payload.get("color").and_then(|v| v.as_str()) {
        tag.color = v.to_owned();
    }
    ok(tag.clone())
}

#[delete("/api/v1/tags/{id}")]
async fn tag_delete(state: web::Data<Arc<AppState>>, path: web::Path<String>) -> ApiResult {
    let id = path.into_inner();
    let mut tags = state.tags.lock().await;
    let before = tags.len();
    tags.retain(|t| t.id != id);
    if tags.len() == before {
        return err(404, "Tag not found");
    }
    ok_msg(json!({ "deleted": true }), "Tag removed")
}

#[get("/api/v1/folders")]
async fn folder_list(state: web::Data<Arc<AppState>>) -> ApiResult {
    ok(state.folders.lock().await.clone())
}

#[post("/api/v1/folders")]
async fn folder_create(
    state: web::Data<Arc<AppState>>,
    payload: web::Json<CreateFolderRequest>,
) -> ApiResult {
    let payload = payload.into_inner();
    let folder = Folder {
        id: Uuid::new_v4().to_string(),
        name: payload.name,
    };
    state.folders.lock().await.push(folder.clone());
    ok_msg(folder, "Folder created")
}

#[post("/api/v1/folders/{id}")]
async fn folder_update(
    state: web::Data<Arc<AppState>>,
    path: web::Path<String>,
    payload: web::Json<Value>,
) -> ApiResult {
    let id = path.into_inner();
    let mut folders = state.folders.lock().await;
    let Some(folder) = folders.iter_mut().find(|f| f.id == id) else {
        return err(404, "Folder not found");
    };
    if let Some(v) = payload.into_inner().get("name").and_then(|v| v.as_str()) {
        folder.name = v.to_owned();
    }
    ok(folder.clone())
}

#[delete("/api/v1/folders/{id}")]
async fn folder_delete(state: web::Data<Arc<AppState>>, path: web::Path<String>) -> ApiResult {
    let id = path.into_inner();
    let mut folders = state.folders.lock().await;
    let before = folders.len();
    folders.retain(|f| f.id != id);
    if folders.len() == before {
        return err(404, "Folder not found");
    }
    ok_msg(json!({ "deleted": true }), "Folder removed")
}

#[get("/api/v1/screen_resolution")]
async fn screen_resolution() -> ApiResult {
    ok_msg(
        json!({ "resolutions": ["1920x1080", "1366x768", "1440x900", "1536x864", "1280x720"] }),
        "",
    )
}

#[post("/api/v1/script_runner/start")]
async fn script_runner_start(
    state: web::Data<Arc<AppState>>,
    payload: web::Json<RunScriptRequest>,
) -> ApiResult {
    let payload = payload.into_inner();
    let mut results = Vec::new();
    for profile_id in &payload.profile_ids {
        let profile = {
            let profiles = state.profiles.lock().await;
            profiles.iter().find(|p| p.id == *profile_id).cloned()
        };
        let Some(profile) = profile else { continue };
        let config = build_config(&profile);
        let mut sb = match BaseCase::new(config).await {
            Ok(sb) => sb,
            Err(e) => {
                results.push(json!({ "profile_id": profile_id, "error": e.to_string() }));
                continue;
            }
        };
        let result = sb
            .execute_script(&payload.script)
            .await
            .map(|v| v.to_string())
            .unwrap_or_else(|e| e.to_string());
        results.push(json!({ "profile_id": profile_id, "result": result }));
        let _ = sb.quit().await;
    }
    ok_msg(json!({ "results": results }), "Script runner started")
}

#[post("/api/v1/script_runner/stop")]
async fn script_runner_stop() -> ApiResult {
    ok_msg(json!({ "stopped": true }), "Script runner stopped")
}

#[get("/api/v1/browser_cores")]
async fn browser_core_list() -> ApiResult {
    ok_msg(
        json!({ "cores": ["chrome-120", "chrome-121", "chrome-122"] }),
        "",
    )
}

#[post("/api/v1/load_browser_core")]
async fn load_browser_core() -> ApiResult {
    ok_msg(json!({ "message": "Download started" }), "")
}

#[delete("/api/v1/delete_browser_core")]
async fn delete_browser_core() -> ApiResult {
    ok_msg(json!({ "message": "" }), "")
}

#[get("/api/v1/stop_all")]
async fn stop_all(state: web::Data<Arc<AppState>>) -> ApiResult {
    let ids: Vec<String> = state.sessions.lock().await.keys().cloned().collect();
    for id in ids {
        if let Some(mut sb) = state.sessions.lock().await.remove(&id) {
            let _ = sb.quit().await;
        }
        state.session_info.lock().await.remove(&id);
    }
    ok_msg(json!({ "stopped_all": true }), "All profiles stopped")
}

#[get("/api/v1/workspaces")]
async fn workspaces() -> ApiResult {
    ok_msg(
        json!({ "workspaces": [{ "id": "default", "name": "Default Workspace" }] }),
        "",
    )
}

#[post("/api/v1/user/signin")]
async fn user_signin() -> ApiResult {
    ok_msg(json!({ "token": "dummy-token", "expires_in": 1800 }), "")
}

#[post("/api/v1/user/refresh_token")]
async fn user_refresh_token() -> ApiResult {
    ok_msg(json!({ "token": "dummy-token", "expires_in": 1800 }), "")
}

#[post("/api/v1/bookmarks/export")]
async fn bookmarks_export() -> ApiResult {
    ok_msg(json!({ "bookmarks": [] }), "")
}

#[post("/api/v1/bookmarks/import")]
async fn bookmarks_import() -> ApiResult {
    ok_msg(json!({ "imported": 0 }), "")
}

#[post("/api/v1/2fa/setup")]
async fn twofa_setup() -> ApiResult {
    ok_msg(json!({ "secret": "DUMMYSECRET", "qr": "" }), "")
}

#[post("/api/v1/2fa/enable")]
async fn twofa_enable() -> ApiResult {
    ok_msg(json!({ "enabled": true }), "")
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{http::StatusCode, test, App};

    fn test_state() -> web::Data<Arc<AppState>> {
        web::Data::new(Arc::new(AppState::new()))
    }

    #[actix_web::test]
    async fn version_endpoint() {
        let app = test::init_service(App::new().app_data(test_state()).configure(configure)).await;
        let req = test::TestRequest::get().uri("/api/v1/version").to_request();
        let res = test::call_service(&app, req).await;
        assert_eq!(res.status(), StatusCode::OK);
        let body: Value = test::read_body_json(res).await;
        assert_eq!(body["status"]["error_code"].as_str(), Some(""));
    }

    #[actix_web::test]
    async fn profile_crud() {
        let app = test::init_service(App::new().app_data(test_state()).configure(configure)).await;

        let create = test::TestRequest::post()
            .uri("/api/v1/profiles")
            .insert_header(("content-type", "application/json"))
            .set_payload(
                r#"{"name":"Test","container_url":"http://localhost:4444","tags":["tag1"]}"#,
            )
            .to_request();
        let create_res = test::call_service(&app, create).await;
        assert_eq!(create_res.status(), StatusCode::OK);
        let body: Value = test::read_body_json(create_res).await;
        let id = body["data"]["id"].as_str().unwrap().to_string();

        let list = test::TestRequest::get()
            .uri("/api/v1/profiles")
            .to_request();
        let list_res = test::call_service(&app, list).await;
        let list_body: Value = test::read_body_json(list_res).await;
        assert_eq!(list_body["data"].as_array().unwrap().len(), 1);

        let get = test::TestRequest::get()
            .uri(&format!("/api/v1/profiles/{id}"))
            .to_request();
        assert_eq!(test::call_service(&app, get).await.status(), StatusCode::OK);

        let update = test::TestRequest::post()
            .uri(&format!("/api/v1/profiles/{id}"))
            .insert_header(("content-type", "application/json"))
            .set_payload(r#"{"name":"Updated"}"#)
            .to_request();
        assert_eq!(
            test::call_service(&app, update).await.status(),
            StatusCode::OK
        );

        let delete = test::TestRequest::delete()
            .uri(&format!("/api/v1/profiles/{id}"))
            .to_request();
        assert_eq!(
            test::call_service(&app, delete).await.status(),
            StatusCode::OK
        );
    }

    #[actix_web::test]
    async fn tags_and_folders() {
        let app = test::init_service(App::new().app_data(test_state()).configure(configure)).await;

        let tag = test::TestRequest::post()
            .uri("/api/v1/tags")
            .insert_header(("content-type", "application/json"))
            .set_payload(r##"{"name":"Work","color":"#ff0000"}"##)
            .to_request();
        assert_eq!(test::call_service(&app, tag).await.status(), StatusCode::OK);

        let folder = test::TestRequest::post()
            .uri("/api/v1/folders")
            .insert_header(("content-type", "application/json"))
            .set_payload(r#"{"name":"Clients"}"#)
            .to_request();
        assert_eq!(
            test::call_service(&app, folder).await.status(),
            StatusCode::OK
        );

        let list = test::TestRequest::get().uri("/api/v1/tags").to_request();
        let list_res = test::call_service(&app, list).await;
        let body: Value = test::read_body_json(list_res).await;
        assert!(!body["data"].as_array().unwrap().is_empty());
    }
}
