//! The extension that answers a proxy's request for a user name and password.
//!
//! Chrome ignores credentials written into `--proxy-server`, and a WebDriver
//! session has no way to answer the proxy's `407` itself, so SeleniumBase
//! loads a small extension that does. (The Pure CDP engine answers over the
//! protocol and needs none.) The proxy's address still goes in
//! `--proxy-server`; the extension only supplies the credentials.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::json;

use crate::error::SeleniumBaseError;

/// The extension's `manifest.json`.
///
/// It is a manifest v3 extension, as Chrome no longer runs version 2. The
/// `webRequestAuthProvider` permission is what lets a v3 extension answer an
/// authentication challenge. Nothing is asked for beyond that: the extension
/// does not change the proxy settings.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::utils::extensions::proxy_auth_manifest;
///
/// let manifest: serde_json::Value = serde_json::from_str(&proxy_auth_manifest())?;
/// assert_eq!(manifest["manifest_version"], 3);
/// # Ok::<(), serde_json::Error>(())
/// ```
#[must_use]
pub fn proxy_auth_manifest() -> String {
    let manifest = json!({
        "manifest_version": 3,
        "name": "Proxy Auth Extension",
        "version": "1.0.0",
        "minimum_chrome_version": "108",
        "permissions": ["webRequest", "webRequestAuthProvider"],
        "host_permissions": ["<all_urls>"],
        "background": { "service_worker": "background.js" },
    });
    serde_json::to_string_pretty(&manifest).expect("a JSON value serialises")
}

/// The extension's `background.js`, which answers with `username` and
/// `password`.
///
/// Both are written as JSON string literals, so quotes, backslashes and line
/// breaks in them stay part of the value and cannot end the string early or
/// add code.
///
/// The script answers only a challenge from the proxy. A web site that asks
/// for a password (Chrome reports it with `isProxy: false`) is left to ask the
/// person, so the proxy's credentials are never handed to a site. A request the
/// proxy refuses twice is cancelled rather than retried for ever.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::utils::extensions::proxy_auth_background_js;
///
/// let script = proxy_auth_background_js("user", "pa\"ss");
/// assert!(script.contains(r#"password: "pa\"ss""#));
/// ```
#[must_use]
pub fn proxy_auth_background_js(username: &str, password: &str) -> String {
    let username = serde_json::to_string(username).expect("a string serialises");
    let password = serde_json::to_string(password).expect("a string serialises");
    format!(
        r#"const credentials = {{ username: {username}, password: {password} }};
const answered = new Set();
const everything = {{ urls: ["<all_urls>"] }};

chrome.webRequest.onAuthRequired.addListener(
  function (details) {{
    // A site that asks for a password is not the proxy: do not give it ours.
    if (!details.isProxy) return {{}};
    // The proxy refused the credentials once already: stop, do not loop.
    if (answered.has(details.requestId)) return {{ cancel: true }};
    answered.add(details.requestId);
    return {{ authCredentials: credentials }};
  }},
  everything,
  ["blocking"]
);

function forget(details) {{
  answered.delete(details.requestId);
}}
chrome.webRequest.onCompleted.addListener(forget, everything);
chrome.webRequest.onErrorOccurred.addListener(forget, everything);
"#
    )
}

/// Writes the extension into `dir`, creating the folder if needed.
///
/// `dir` then holds `manifest.json` and `background.js`, and can be given to
/// Chrome with `--load-extension`. `background.js` holds the credentials in
/// plain text, so on Unix both files are readable by their owner only. Delete
/// the folder when the browser is done with it.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::Io`] if the folder or a file cannot be written.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::utils::extensions::write_proxy_auth_extension;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let dir = std::env::temp_dir().join(format!("sb-doc-proxy-auth-{}", std::process::id()));
/// write_proxy_auth_extension(&dir, "user", "secret")?;
/// assert!(dir.join("manifest.json").is_file());
/// assert!(dir.join("background.js").is_file());
/// # std::fs::remove_dir_all(&dir)?;
/// # Ok(())
/// # }
/// ```
pub fn write_proxy_auth_extension(
    dir: &Path,
    username: &str,
    password: &str,
) -> Result<(), SeleniumBaseError> {
    fs::create_dir_all(dir)?;
    write_private(&dir.join("manifest.json"), &proxy_auth_manifest())?;
    write_private(
        &dir.join("background.js"),
        &proxy_auth_background_js(username, password),
    )?;
    Ok(())
}

/// Makes a new folder under the system's temporary folder, writes the
/// extension into it and returns the folder.
///
/// Every call makes a different folder, so runs side by side do not overwrite
/// each other's credentials, and nothing is written into the working
/// directory, where a password could end up committed. The caller removes the
/// folder once the browser has exited; it is not removed automatically. See
/// [`write_proxy_auth_extension`] to choose the folder.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::Io`] if the folder or a file cannot be written.
pub fn get_proxy_auth_extension(
    username: &str,
    password: &str,
) -> Result<PathBuf, SeleniumBaseError> {
    let dir = tempfile::Builder::new()
        .prefix("sb-proxy-auth-")
        .tempdir()?
        .keep();
    if let Err(error) = write_proxy_auth_extension(&dir, username, password) {
        // Do not leave half an extension, or a password, behind.
        let _ = fs::remove_dir_all(&dir);
        return Err(error);
    }
    Ok(dir)
}

/// Writes `contents` to `path` for its owner alone (on Unix), replacing a file
/// that is already there.
fn write_private(path: &Path, contents: &str) -> std::io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    // `mode` applies only to a file this call creates; fix one that existed.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(contents.as_bytes())
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    #[test]
    fn the_manifest_is_valid_json_for_manifest_v3() {
        let manifest: Value =
            serde_json::from_str(&proxy_auth_manifest()).expect("manifest.json is JSON");
        assert_eq!(manifest["manifest_version"], 3);
        assert_eq!(manifest["background"]["service_worker"], "background.js");
        let permissions = manifest["permissions"].as_array().expect("permissions");
        // Without these two the listener in background.js never hears a challenge.
        assert!(permissions.contains(&json!("webRequest")));
        assert!(permissions.contains(&json!("webRequestAuthProvider")));
        // And the extension has no business changing the proxy.
        assert!(!permissions.contains(&json!("proxy")));
    }

    #[test]
    fn written_files_are_the_manifest_and_the_script() {
        let dir = tempfile::tempdir().expect("a temporary folder");
        let target = dir.path().join("nested").join("extension");
        write_proxy_auth_extension(&target, "user", "secret").expect("writes");
        assert_eq!(
            fs::read_to_string(target.join("manifest.json")).expect("manifest.json"),
            proxy_auth_manifest()
        );
        assert_eq!(
            fs::read_to_string(target.join("background.js")).expect("background.js"),
            proxy_auth_background_js("user", "secret")
        );
    }

    #[test]
    fn credentials_are_written_as_json_string_literals() {
        let hostile = "p\"w\\d\n</script>';alert(1);//\u{2028}";
        let script = proxy_auth_background_js("us\"er", hostile);
        let literal = serde_json::to_string(hostile).expect("a string serialises");
        assert!(script.contains(&format!("password: {literal}")), "{script}");
        assert!(script.contains(r#"username: "us\"er""#), "{script}");
        // The raw characters never reach the script unescaped.
        assert!(!script.contains("p\"w"), "{script}");
        assert!(!script.contains("\n</script>"), "{script}");
    }

    #[test]
    fn the_script_answers_only_the_proxy() {
        let script = proxy_auth_background_js("u", "p");
        assert!(script.contains("if (!details.isProxy) return {};"));
        assert!(script.contains("{ cancel: true }"));
    }

    #[cfg(unix)]
    #[test]
    fn the_files_are_readable_by_their_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("a temporary folder");
        // A file that is already there, and too open, is tightened.
        fs::write(dir.path().join("background.js"), "old").expect("an old file");
        fs::set_permissions(
            dir.path().join("background.js"),
            fs::Permissions::from_mode(0o644),
        )
        .expect("loosen it");
        write_proxy_auth_extension(dir.path(), "user", "secret").expect("writes");
        for file in ["manifest.json", "background.js"] {
            let mode = fs::metadata(dir.path().join(file))
                .expect("metadata")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600, "{file}");
        }
    }

    #[test]
    fn each_call_makes_a_new_folder_outside_the_working_directory() {
        let first = get_proxy_auth_extension("user", "secret").expect("first");
        let second = get_proxy_auth_extension("user", "secret").expect("second");
        assert_ne!(first, second);
        let cwd = std::env::current_dir().expect("a working directory");
        for dir in [&first, &second] {
            assert!(!dir.starts_with(&cwd), "{dir:?} is inside {cwd:?}");
            assert!(dir.join("manifest.json").is_file());
            assert!(dir.join("background.js").is_file());
        }
        fs::remove_dir_all(first).expect("cleanup");
        fs::remove_dir_all(second).expect("cleanup");
    }
}
