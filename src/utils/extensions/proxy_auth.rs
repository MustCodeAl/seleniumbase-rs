use std::path::PathBuf;

pub fn get_proxy_auth_extension(
    username: &str,
    password: &str,
) -> Result<PathBuf, crate::error::SeleniumBaseError> {
    let dir = PathBuf::from("proxy_auth_extension");
    std::fs::create_dir_all(&dir).map_err(|e| {
        crate::error::SeleniumBaseError::InvalidConfig(format!(
            "failed to create extension dir: {}",
            e
        ))
    })?;
    let manifest = dir.join("manifest.json");
    let content = r#"{
  "version": "1.0.0",
  "manifest_version": 2,
  "name": "Proxy Auth Extension",
  "permissions": ["proxy", "<all_urls>"],
  "background": {{
    "scripts": ["background.js"]
  }}
}}"#
    .to_string();
    std::fs::write(&manifest, content).map_err(|e| {
        crate::error::SeleniumBaseError::InvalidConfig(format!("failed to write manifest: {}", e))
    })?;

    let bg = dir.join("background.js");
    let js = format!(
        r#"chrome.webRequest.onAuthRequired.addListener(
  function(details) {{
    return {{ authCredentials: {{ username: "{}", password: "{}" }} }};
  }},
  {{ urls: ["<all_urls>"] }},
  ['blocking']
);"#,
        username, password
    );
    std::fs::write(&bg, js).map_err(|e| {
        crate::error::SeleniumBaseError::InvalidConfig(format!(
            "failed to write background.js: {}",
            e
        ))
    })?;
    Ok(dir)
}
