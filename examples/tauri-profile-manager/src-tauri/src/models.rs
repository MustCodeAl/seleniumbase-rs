use serde::{Deserialize, Serialize};

use seleniumbase_rs::profile_payloads::ProfileParams;
use seleniumbase_rs::{Browser, DriverMode};

/// Common API status wrapper.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiStatus {
    pub error_code: String,
    pub http_code: u16,
    pub message: String,
}

impl ApiStatus {
    pub fn ok(message: impl Into<String>) -> Self {
        Self {
            error_code: "".into(),
            http_code: 200,
            message: message.into(),
        }
    }

    pub fn err(code: impl Into<String>, msg: impl Into<String>) -> Self {
        Self {
            error_code: code.into(),
            http_code: 400,
            message: msg.into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    pub status: ApiStatus,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            data: Some(data),
            status: ApiStatus::ok(""),
        }
    }

    pub fn ok_msg(data: T, msg: impl Into<String>) -> Self {
        Self {
            data: Some(data),
            status: ApiStatus::ok(msg),
        }
    }

    pub fn err(status: ApiStatus) -> Self {
        Self { data: None, status }
    }
}

/// A saved browser profile that maps to one isolated container.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub container_url: String,
    #[serde(default)]
    pub browser: Browser,
    #[serde(default)]
    pub mode: DriverMode,
    pub user_agent: Option<String>,
    pub proxy: Option<String>,
    pub locale: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub accuracy: Option<f64>,
    #[serde(default)]
    pub headless: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub folder_id: String,
    #[serde(default)]
    pub cookies: Vec<BrowserCookie>,
    /// Full external profile parameters, when supplied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_profile: Option<ProfileParams>,
}

/// Input payload for creating a profile.
#[derive(Clone, Debug, Deserialize)]
pub struct NewProfile {
    pub name: String,
    pub container_url: String,
    #[serde(default)]
    pub browser: Browser,
    #[serde(default)]
    pub mode: DriverMode,
    pub user_agent: Option<String>,
    pub proxy: Option<String>,
    pub locale: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub accuracy: Option<f64>,
    #[serde(default)]
    pub headless: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub folder_id: String,
    /// Raw external profile parameters (flags, fingerprints, storage, proxy, ...).
    #[serde(default, rename = "parameters")]
    pub external_profile: Option<ProfileParams>,
}

/// Information returned after launching a profile.
#[derive(Clone, Debug, Serialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub profile_id: String,
    pub profile_name: String,
    pub container_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrowserCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires: Option<f64>,
    #[serde(default)]
    pub secure: bool,
    #[serde(default)]
    pub http_only: bool,
    #[serde(default)]
    pub same_site: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ProxyValidateRequest {
    #[serde(rename = "type")]
    pub proxy_type: String,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProxyValidateData {
    pub ip: String,
    pub country_code: String,
    pub latitude: f64,
    pub longitude: f64,
    pub timezone: String,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(dead_code)]
pub struct CookieImportRequest {
    pub profile_id: String,
    #[serde(default)]
    pub folder_id: String,
    #[serde(default)]
    pub import_advanced_cookies: bool,
    #[serde(default)]
    pub cookies: Vec<BrowserCookie>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CookieExportRequest {
    pub profile_id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(dead_code)]
pub struct StartProfileRequest {
    pub profile_id: String,
    #[serde(default)]
    pub automation: String,
    #[serde(default)]
    pub prefs: serde_json::Value,
}

#[derive(Clone, Debug, Serialize)]
pub struct StartProfileData {
    pub profile_id: String,
    pub session_id: String,
    pub port: u16,
    pub ws_endpoint: String,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CreateTagRequest {
    pub name: String,
    pub color: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub color: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CreateFolderRequest {
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RunScriptRequest {
    pub profile_ids: Vec<String>,
    pub script: String,
}
