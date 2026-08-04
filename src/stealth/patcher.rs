//! Binary-level anti-detection patching for Chromium drivers and engines.
//!
//! Chromedriver leaks several identifying markers into the page (the `cdc_`
//! variables, `__webdriver` globals, etc.). The helpers here patch those
//! markers directly in the driver binary before it is launched, so the browser
//! never injects them in the first place.
//!
//! Typical usage:
//!
//! ```ignore
//! use seleniumbase_rs::stealth::patcher::{ChromedriverPatcher, EnginePatch};
//!
//! ChromedriverPatcher::new("chromedriver")
//!     .patch(EnginePatch::all())?;
//! ```

use crate::error::SeleniumBaseError;
use rand::RngExt;
use regex::bytes::Regex;
use std::fs;
use std::path::{Path, PathBuf};

/// Kinds of binary patches that can be applied to a Chromium driver.
#[derive(Clone, Debug, Default)]
pub struct EnginePatch {
    /// Replace `cdc_` property assignments with spaces.
    pub scrub_cdc_props: bool,
    /// Replace quoted `$cdc_` prefix strings with random ids.
    pub randomize_cdc_prefix: bool,
    /// Replace `__webdriver`, `__selenium`, `__driver` markers.
    pub scrub_webdriver_markers: bool,
    /// Replace `{window.cdc_...;}` blocks.
    pub replace_cdc_blocks: bool,
    /// Replace hard-coded `ChromeDriver/<version>` strings.
    pub scrub_chrome_version: bool,
    /// Replace `navigator.webdriver = !0` / `= true` assignments.
    pub patch_navigator_webdriver: bool,
    /// Replace `HeadlessChrome` user-agent markers with `Chrome`.
    pub patch_headless_user_agent: bool,
    /// Create a `.orig` backup before patching.
    pub backup: bool,
}

impl EnginePatch {
    /// A conservative set of patches with backup enabled.
    pub fn balanced() -> Self {
        Self {
            scrub_cdc_props: true,
            randomize_cdc_prefix: true,
            scrub_webdriver_markers: true,
            replace_cdc_blocks: false,
            scrub_chrome_version: false,
            patch_navigator_webdriver: false,
            patch_headless_user_agent: false,
            backup: true,
        }
    }

    /// All supported patches with backup enabled.
    pub fn all() -> Self {
        Self {
            scrub_cdc_props: true,
            randomize_cdc_prefix: true,
            scrub_webdriver_markers: true,
            replace_cdc_blocks: true,
            scrub_chrome_version: true,
            patch_navigator_webdriver: true,
            patch_headless_user_agent: true,
            backup: true,
        }
    }

    /// Aggressive set including version and navigator patches.
    pub fn aggressive() -> Self {
        Self {
            scrub_chrome_version: true,
            patch_navigator_webdriver: true,
            patch_headless_user_agent: true,
            ..Self::all()
        }
    }

    /// No backup; useful in CI where the binary is ephemeral.
    pub fn no_backup() -> Self {
        Self {
            backup: false,
            ..Self::all()
        }
    }

    /// Patches suitable for the Chrome/Chromium browser binary itself.
    pub fn chrome_binary() -> Self {
        Self {
            scrub_cdc_props: false,
            randomize_cdc_prefix: false,
            replace_cdc_blocks: false,
            scrub_chrome_version: false,
            scrub_webdriver_markers: true,
            patch_navigator_webdriver: true,
            patch_headless_user_agent: true,
            backup: false,
        }
    }
}

/// Fluent patcher for a chromedriver-style binary.
#[derive(Clone, Debug)]
pub struct ChromedriverPatcher<P: AsRef<Path>> {
    path: P,
}

impl<P: AsRef<Path>> ChromedriverPatcher<P> {
    /// Wraps the path to a chromedriver binary.
    pub fn new(path: P) -> Self {
        Self { path }
    }

    /// Path to the binary being patched.
    pub fn path(&self) -> &Path {
        self.path.as_ref()
    }

    /// Default backup path (`<binary>.orig`).
    pub fn backup_path(&self) -> PathBuf {
        self.path.as_ref().with_extension("orig")
    }

    /// Applies `spec` to the binary in-place.
    pub fn patch(&self, spec: EnginePatch) -> Result<(), SeleniumBaseError> {
        let path = self.path.as_ref();
        let path_str = path.display().to_string();
        if spec.backup {
            let backup = self.backup_path();
            fs::copy(path, &backup).map_err(|e| {
                SeleniumBaseError::patcher(
                    &path_str,
                    format!(
                        "failed to back up chromedriver to {}: {}",
                        backup.display(),
                        e
                    ),
                )
            })?;
        }

        let mut content = fs::read(path).map_err(|e| {
            SeleniumBaseError::patcher(&path_str, format!("failed to read chromedriver: {e}"))
        })?;

        if spec.scrub_cdc_props {
            content = scrub_cdc_property_assignments(content);
        }
        if spec.randomize_cdc_prefix {
            content = randomize_cdc_string_prefix(content);
        }
        if spec.scrub_webdriver_markers {
            content = scrub_automation_markers(content);
        }
        if spec.replace_cdc_blocks {
            content = replace_cdc_blocks(content);
        }
        if spec.scrub_chrome_version {
            content = scrub_chrome_version_strings(content);
        }
        if spec.patch_navigator_webdriver {
            content = patch_navigator_webdriver_assignments(content);
        }

        fs::write(path, content).map_err(|e| {
            let err = SeleniumBaseError::patcher(
                &path_str,
                format!("failed to write patched chromedriver: {e}"),
            );
            err.log_in_context("ChromedriverPatcher::patch");
            err
        })?;
        Ok(())
    }

    /// Restores the binary from the `.orig` backup if it exists.
    pub fn restore(&self) -> Result<(), SeleniumBaseError> {
        let path_str = self.path.as_ref().display().to_string();
        let backup = self.backup_path();
        if !backup.exists() {
            return Err(SeleniumBaseError::patcher(
                &path_str,
                "no .orig backup found to restore",
            ));
        }
        fs::copy(&backup, self.path.as_ref()).map_err(|e| {
            SeleniumBaseError::patcher(
                &path_str,
                format!(
                    "failed to restore chromedriver from {}: {}",
                    backup.display(),
                    e
                ),
            )
        })?;
        Ok(())
    }

    /// Returns true if known automation markers are still present.
    pub fn needs_patch(&self) -> Result<bool, SeleniumBaseError> {
        let path_str = self.path.as_ref().display().to_string();
        let content = fs::read(self.path.as_ref()).map_err(|e| {
            let err =
                SeleniumBaseError::patcher(&path_str, format!("failed to read chromedriver: {e}"));
            err.log_in_context("ChromedriverPatcher::needs_patch");
            err
        })?;
        Ok(has_automation_markers(&content))
    }
}

/// Patcher for the Chrome/Chromium browser binary itself.
///
/// Unlike [`ChromedriverPatcher`], this operates on a *copy* of the browser
/// executable. The original binary is never modified; the patched copy is
/// written to a cache directory and returned for use in launch capabilities.
/// This makes the spoofing invisible to page-side JS introspection because
/// there is no JavaScript override to detect — the browser executable itself
/// returns the fake value.
#[derive(Clone, Debug)]
pub struct ChromeBinaryPatcher {
    source: PathBuf,
    cache_dir: Option<PathBuf>,
}

impl ChromeBinaryPatcher {
    /// Wraps the path to a Chrome/Chromium binary.
    pub fn new(source: impl AsRef<Path>) -> Self {
        Self {
            source: source.as_ref().to_path_buf(),
            cache_dir: None,
        }
    }

    /// Use a specific cache directory for the patched copy. If unset, the
    /// platform cache directory is used.
    pub fn with_cache_dir(mut self, dir: impl AsRef<Path>) -> Self {
        self.cache_dir = Some(dir.as_ref().to_path_buf());
        self
    }

    /// Returns the source binary path.
    pub fn source(&self) -> &Path {
        &self.source
    }

    /// Computes the cached patched-binary path based on source metadata and
    /// the applied patch set.
    pub fn patched_path(&self) -> Result<PathBuf, SeleniumBaseError> {
        let base = self
            .cache_dir
            .clone()
            .or_else(dirs::cache_dir)
            .ok_or_else(|| {
                SeleniumBaseError::patcher(
                    self.source.display().to_string(),
                    "no cache directory available for patched binary",
                )
            })?;
        let meta = fs::metadata(&self.source).map_err(|e| {
            SeleniumBaseError::patcher(
                self.source.display().to_string(),
                format!("failed to read chrome binary metadata: {e}"),
            )
        })?;
        let mtime = meta
            .modified()
            .map(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            })
            .unwrap_or(0);
        let source_str = self.source.display().to_string();
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        format!("{source_str}:{mtime}:2").hash(&mut hasher);
        let hash = format!("{:x}", hasher.finish());
        Ok(base
            .join("seleniumbase-rs")
            .join("chrome-patches")
            .join(hash)
            .join(if cfg!(windows) {
                "chrome.exe"
            } else {
                "chrome"
            }))
    }

    /// Applies `spec` to a copy of the source binary and returns the path to
    /// the patched copy.
    pub fn patch(&self, spec: EnginePatch) -> Result<PathBuf, SeleniumBaseError> {
        let source_str = self.source.display().to_string();
        if !self.source.exists() {
            return Err(SeleniumBaseError::browser_launch(
                source_str,
                "chrome binary not found",
            ));
        }

        let out = self.patched_path()?;
        if out.exists() {
            tracing::debug!(path = %out.display(), "reusing cached patched chrome binary");
            return Ok(out);
        }

        tracing::info!(source = %source_str, dest = %out.display(), "patching chrome binary");
        let content = fs::read(&self.source).map_err(|e| {
            SeleniumBaseError::patcher(
                source_str.clone(),
                format!("failed to read chrome binary: {e}"),
            )
        })?;

        let patched = apply_chrome_binary_patches(content, spec);

        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                SeleniumBaseError::patcher(
                    source_str.clone(),
                    format!("failed to create patch cache dir: {e}"),
                )
            })?;
        }
        fs::write(&out, patched).map_err(|e| {
            SeleniumBaseError::patcher(
                source_str.clone(),
                format!("failed to write patched chrome binary: {e}"),
            )
        })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&out)
                .map_err(|e| {
                    SeleniumBaseError::patcher(
                        source_str.clone(),
                        format!("failed to read patched binary permissions: {e}"),
                    )
                })?
                .permissions();
            perms.set_mode(perms.mode() | 0o111);
            fs::set_permissions(&out, perms).map_err(|e| {
                SeleniumBaseError::patcher(
                    source_str.clone(),
                    format!("failed to make patched binary executable: {e}"),
                )
            })?;
        }

        #[cfg(target_os = "macos")]
        strip_binary_signature(&out)?;

        tracing::info!(path = %out.display(), "patched chrome binary ready");
        Ok(out)
    }
}

/// Attempts to locate a system Chrome/Chromium binary.
///
/// Checks, in order:
/// * The `CHROME_BIN` / `CHROMIUM_BIN` environment variables.
/// * Common well-known paths per platform.
/// * `which google-chrome` / `which chromium` / `which chromium-browser`.
pub fn find_system_chrome() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("CHROME_BIN") {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(path) = std::env::var("CHROMIUM_BIN") {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }

    let candidates: Vec<PathBuf> = {
        #[cfg(target_os = "macos")]
        {
            vec![
                "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".into(),
                "/Applications/Chromium.app/Contents/MacOS/Chromium".into(),
                "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary".into(),
                "~/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".into(),
            ]
        }
        #[cfg(target_os = "windows")]
        {
            let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
            let pf = std::env::var("PROGRAMFILES").unwrap_or_default();
            let pf_x86 = std::env::var("PROGRAMFILES(X86)").unwrap_or_default();
            vec![
                format!(r"{local}\Google\Chrome\Application\chrome.exe").into(),
                format!(r"{pf}\Google\Chrome\Application\chrome.exe").into(),
                format!(r"{pf_x86}\Google\Chrome\Application\chrome.exe").into(),
                r"C:\Program Files\Google\Chrome\Application\chrome.exe".into(),
                r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe".into(),
            ]
        }
        #[cfg(target_os = "linux")]
        {
            vec![
                "/usr/bin/google-chrome".into(),
                "/usr/bin/google-chrome-stable".into(),
                "/usr/bin/chromium".into(),
                "/usr/bin/chromium-browser".into(),
                "/snap/bin/chromium".into(),
            ]
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            vec![]
        }
    };

    for candidate in candidates {
        let expanded = expand_tilde(candidate);
        if expanded.exists() {
            return Some(expanded);
        }
    }

    for name in [
        "google-chrome-stable",
        "google-chrome",
        "chromium",
        "chromium-browser",
    ] {
        if let Ok(p) = which::which(name) {
            return Some(p);
        }
    }

    None
}

#[cfg(target_os = "macos")]
fn strip_binary_signature(path: &Path) -> Result<(), SeleniumBaseError> {
    if which::which("codesign").is_err() {
        tracing::warn!("codesign not found; patched macOS binary may fail to launch");
        return Ok(());
    }
    let status = std::process::Command::new("codesign")
        .arg("--remove-signature")
        .arg(path)
        .status()
        .map_err(|e| {
            SeleniumBaseError::patcher(
                path.display().to_string(),
                format!("failed to invoke codesign: {e}"),
            )
        })?;
    if !status.success() {
        tracing::warn!(path = %path.display(), "codesign --remove-signature failed; patched binary may be rejected by macOS");
    } else {
        tracing::debug!(path = %path.display(), "removed code signature from patched binary");
    }
    Ok(())
}

fn apply_chrome_binary_patches(mut content: Vec<u8>, spec: EnginePatch) -> Vec<u8> {
    if spec.patch_headless_user_agent {
        content = replace_literal_equal_len(&content, b"HeadlessChrome", b"Chrome        ");
    }
    if spec.scrub_webdriver_markers {
        let markers: Vec<(&[u8], &[u8])> = vec![
            (b"__webdriver", b"__0000000"),
            (b"__selenium", b"__0000000"),
            (b"__fxdriver", b"__0000000"),
            (b"__driver", b"__0000"),
            (b"ChromeDriver", b"Chrome      "),
        ];
        for (needle, repl) in markers {
            content = replace_literal_equal_len(&content, needle, repl);
        }
    }
    if spec.patch_navigator_webdriver {
        let re = Regex::new(r"navigator\.webdriver\s*=\s*(!0|true)").unwrap();
        content = re
            .replace_all(&content, |caps: &regex::bytes::Captures| {
                vec![b' '; caps[0].len()]
            })
            .into_owned();
    }
    content
}

fn replace_literal_equal_len(content: &[u8], needle: &[u8], replacement: &[u8]) -> Vec<u8> {
    assert_eq!(
        needle.len(),
        replacement.len(),
        "replacement must match needle length"
    );
    let mut out = content.to_vec();
    let mut i = 0;
    while i + needle.len() <= out.len() {
        if &out[i..i + needle.len()] == needle {
            out[i..i + needle.len()].copy_from_slice(replacement);
            i += needle.len();
        } else {
            i += 1;
        }
    }
    out
}

fn expand_tilde(path: PathBuf) -> PathBuf {
    let s = path.display().to_string();
    if let Some(rest) = s.strip_prefix("~/") {
        dirs::home_dir().map(|home| home.join(rest)).unwrap_or(path)
    } else {
        path
    }
}

fn has_automation_markers(content: &[u8]) -> bool {
    static PATTERNS: std::sync::OnceLock<Vec<Regex>> = std::sync::OnceLock::new();
    let patterns = PATTERNS.get_or_init(|| {
        vec![
            Regex::new(r"window\.cdc_[a-zA-Z0-9]{22}_").unwrap(),
            Regex::new(r#"['\"]?\$cdc_[a-zA-Z0-9]{22}_['\"]?"#).unwrap(),
            Regex::new(r"__webdriver|__selenium|__driver").unwrap(),
            Regex::new(r"\{window\.cdc.*?;\}").unwrap(),
            Regex::new(r"ChromeDriver/\d+\.\d+\.\d+\.\d+").unwrap(),
            Regex::new(r"navigator\.webdriver\s*=\s*(!0|true)").unwrap(),
        ]
    });
    patterns.iter().any(|re| re.is_match(content))
}

fn scrub_cdc_property_assignments(mut content: Vec<u8>) -> Vec<u8> {
    let re = Regex::new(
        r"window\.cdc_[a-zA-Z0-9]{22}_(Array|Promise|Symbol|Object|Proxy|JSON|Window)\s*=\s*window\.(Array|Promise|Symbol|Object|Proxy|JSON|Window);",
    )
    .unwrap();
    content = re
        .replace_all(&content, |caps: &regex::bytes::Captures| {
            vec![b' '; caps[0].len()]
        })
        .into_owned();

    let re = Regex::new(
        r"window\.cdc_[a-zA-Z0-9]{22}_(Array|Promise|Symbol|Object|Proxy|JSON|Window)\s*\|\|",
    )
    .unwrap();
    content = re
        .replace_all(&content, |caps: &regex::bytes::Captures| {
            vec![b' '; caps[0].len()]
        })
        .into_owned();
    content
}

fn randomize_cdc_string_prefix(mut content: Vec<u8>) -> Vec<u8> {
    let re = Regex::new(r#"['\"]\$cdc_[a-zA-Z0-9]{22}_['\"];"#).unwrap();
    content = re
        .replace_all(&content, |caps: &regex::bytes::Captures| {
            let full = &caps[0];
            let quote = full[0];
            let inner_len = full.len().saturating_sub(3); // two quotes + semicolon
            let mut rng = rand::rng();
            let ran_len = rng.random_range(6..=inner_len.max(6));
            let chars: Vec<u8> = (0..ran_len)
                .map(|_| rng.random_range(b'a'..=b'z'))
                .collect();

            let mut out = Vec::with_capacity(full.len());
            out.push(quote);
            out.extend(chars);
            out.push(quote);
            out.push(b';');
            out.extend(std::iter::repeat_n(
                b'\n',
                inner_len.saturating_sub(ran_len),
            ));
            out
        })
        .into_owned();
    content
}

fn scrub_automation_markers(mut content: Vec<u8>) -> Vec<u8> {
    let re = Regex::new(r"__webdriver|__selenium|__driver").unwrap();
    content = re
        .replace_all(&content, |caps: &regex::bytes::Captures| {
            vec![b' '; caps[0].len()]
        })
        .into_owned();
    content
}

fn replace_cdc_blocks(mut content: Vec<u8>) -> Vec<u8> {
    let re = Regex::new(r"\{window\.cdc.*?;\}").unwrap();
    content = re
        .replace_all(&content, |caps: &regex::bytes::Captures| {
            let mut out = b"{console.log(\"chromedriver is undetectable!\")}".to_vec();
            if out.len() < caps[0].len() {
                out.extend(vec![b' '; caps[0].len() - out.len()]);
            } else {
                out.truncate(caps[0].len());
            }
            out
        })
        .into_owned();
    content
}

fn scrub_chrome_version_strings(mut content: Vec<u8>) -> Vec<u8> {
    let re = Regex::new(r"ChromeDriver/\d+\.\d+\.\d+\.\d+").unwrap();
    content = re
        .replace_all(&content, |caps: &regex::bytes::Captures| {
            vec![b' '; caps[0].len()]
        })
        .into_owned();
    content
}

fn patch_navigator_webdriver_assignments(mut content: Vec<u8>) -> Vec<u8> {
    let re = Regex::new(r"navigator\.webdriver\s*=\s*(!0|true)").unwrap();
    content = re
        .replace_all(&content, |caps: &regex::bytes::Captures| {
            let matched = &caps[0];
            let prefix_len = matched
                .windows(2)
                .position(|w| w == b"= ")
                .map(|i| i + 2)
                .unwrap_or(matched.len().saturating_sub(4));
            let mut out = Vec::with_capacity(matched.len());
            out.extend_from_slice(&matched[..prefix_len]);
            out.extend_from_slice(b"false");
            out.extend(vec![b' '; matched.len().saturating_sub(out.len())]);
            out
        })
        .into_owned();
    content
}

/// Convenience one-shot patcher preserving the old API.
pub fn patch_chromedriver<P: AsRef<Path>>(path: P) -> Result<(), SeleniumBaseError> {
    ChromedriverPatcher::new(path).patch(EnginePatch::all())
}

/// Returns additional Chromium command-line flags that reduce engine-level
/// automation fingerprints. These complement the JS evasions applied at
/// runtime.
pub fn engine_spoofing_args() -> Vec<String> {
    vec![
        "--disable-blink-features=AutomationControlled".to_owned(),
        "--disable-features=IsolateOrigins,site-per-process,PrivacySandboxSettings4,InterestFeedContentSuggestions,FedCm,WebRtcHideLocalIpsWithMdns".to_owned(),
        "--disable-component-extensions-with-background-pages".to_owned(),
        "--disable-background-networking".to_owned(),
        "--disable-background-timer-throttling".to_owned(),
        "--disable-backgrounding-occluded-windows".to_owned(),
        "--disable-renderer-backgrounding".to_owned(),
        "--disable-client-side-phishing-detection".to_owned(),
        "--disable-default-apps".to_owned(),
        "--disable-hang-monitor".to_owned(),
        "--disable-popup-blocking".to_owned(),
        "--disable-prompt-on-repost".to_owned(),
        "--disable-sync".to_owned(),
        "--disable-translate".to_owned(),
        "--metrics-recording-only".to_owned(),
        "--no-first-run".to_owned(),
        "--no-default-browser-check".to_owned(),
        "--no-pings".to_owned(),
        "--password-store=basic".to_owned(),
        "--use-mock-keychain".to_owned(),
        "--force-webrtc-ip-handling-policy=disable_non_proxied_udp".to_owned(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patches_cdc_marker() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("chromedriver");
        let marker = b"window.cdc_abcdef1234567890abcdef_Array = window.Array;";
        fs::write(&path, marker).unwrap();

        patch_chromedriver(&path).unwrap();

        let patched = fs::read(&path).unwrap();
        let patched_str = String::from_utf8_lossy(&patched);
        assert!(!patched_str.contains("cdc_abcdef1234567890abcdef_Array"));
    }

    #[test]
    fn patcher_detects_markers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("chromedriver");
        fs::write(
            &path,
            b"window.cdc_abcdef1234567890abcdef_Array = window.Array;",
        )
        .unwrap();

        let patcher = ChromedriverPatcher::new(&path);
        assert!(patcher.needs_patch().unwrap());

        patcher.patch(EnginePatch::balanced()).unwrap();
        assert!(!patcher.needs_patch().unwrap());
    }

    #[test]
    fn patcher_creates_backup_and_restores() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("chromedriver");
        let original = b"window.cdc_abcdef1234567890abcdef_Array = window.Array;";
        fs::write(&path, original).unwrap();

        let patcher = ChromedriverPatcher::new(&path);
        patcher.patch(EnginePatch::all()).unwrap();
        assert!(patcher.backup_path().exists());

        patcher.restore().unwrap();
        let restored = fs::read(&path).unwrap();
        assert_eq!(&restored[..], original);
    }

    #[test]
    fn engine_args_include_automation_switch() {
        let args = engine_spoofing_args();
        assert!(args.contains(&"--disable-blink-features=AutomationControlled".to_owned()));
        assert!(args.iter().any(|a| a.contains("PrivacySandboxSettings4")));
    }
}
