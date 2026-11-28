//! User plugin lifecycle management for `sbrs plugin`: `install`, `uninstall`,
//! `enable`, `disable`, `upgrade`, `search`, `list`, and `directory`.
//!
//! Stores user-installed plugin metadata and enabled/disabled overrides inside a
//! per-user plugin directory (`SB_PLUGINS_DIR`, or the platform configuration
//! directory `sbrs/plugins`). All operations are pure Rust and tested against
//! temporary directories without touching `$HOME` or requiring network access
//! when using local plugin paths.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Environment variable overriding the user plugin store directory.
pub const PLUGINS_DIR_ENV: &str = "SB_PLUGINS_DIR";

/// File name of the plugin store registry inside the plugin directory.
const REGISTRY_FILE: &str = "registry.json";

/// Subfolder inside the plugin directory holding installed plugin checkouts/copies.
const INSTALLED_SUBDIR: &str = "installed";

/// Default official community/companion plugin repository.
const OFFICIAL_PLUGINS_REPO: &str = "https://github.com/MustCodeAl/seleniumbase-rs-plugins.git";

/// Errors returned by [`crate::cli::plugin_manager`] operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PluginManagerError {
    /// Invalid plugin name or path traversal attempt.
    #[error("invalid plugin name {name:?}: {reason}")]
    InvalidName {
        /// Supplied name.
        name: String,
        /// Explanation.
        reason: String,
    },
    /// Plugin source cannot be resolved or installed.
    #[error("invalid plugin source {source_str:?}: {reason}")]
    InvalidSource {
        /// Supplied source string.
        source_str: String,
        /// Explanation.
        reason: String,
    },
    /// Plugin is already installed and `--force` was not passed.
    #[error(
        "plugin `{0}` is already installed; pass --force to reinstall or use `sbrs plugin upgrade`"
    )]
    AlreadyInstalled(String),
    /// Named plugin is not installed in the user plugin directory.
    #[error("plugin `{0}` is not installed")]
    NotInstalled(String),
    /// External `git` command failed during remote install or upgrade.
    #[error("git {operation} failed: {details}")]
    GitFailed {
        /// Git subcommand/operation.
        operation: String,
        /// Combined output/message.
        details: String,
    },
    /// Registry JSON is unreadable or malformed.
    #[error("invalid plugin registry at {}: {source}", .path.display())]
    InvalidRegistry {
        /// Registry path.
        path: PathBuf,
        /// Underlying JSON error.
        #[source]
        source: serde_json::Error,
    },
    /// Filesystem I/O failure.
    #[error("I/O error at {}: {source}", .path.display())]
    Io {
        /// Failing path.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
}

/// Origin from which a user plugin was installed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum PluginInstallOrigin {
    /// Installed from a local directory path.
    Local {
        /// Canonical or supplied source directory path.
        path: PathBuf,
    },
    /// Installed from a remote git URL or official catalog entry.
    Remote {
        /// Remote repository URL.
        url: String,
        /// Optional subdirectory inside a monorepo (such as `crates/plugin-owasp`).
        subpath: Option<String>,
    },
}

/// Record stored in `registry.json` for a user-installed plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledPlugin {
    /// Normalized plugin name.
    pub name: String,
    /// Optional version parsed from `Cargo.toml` or `sbrs-plugin.toml`.
    pub version: String,
    /// Short summary parsed from the plugin manifest or catalog.
    pub summary: String,
    /// Whether the plugin is currently enabled for user runs.
    pub enabled: bool,
    /// Where it was installed from.
    pub origin: PluginInstallOrigin,
    /// Location of the installed files inside `<plugins_dir>/installed/<name>`.
    pub installed_path: PathBuf,
}

/// Persisted plugin store registry (`registry.json`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginRegistry {
    /// Installed plugins keyed by normalized plugin name.
    pub plugins: BTreeMap<String, InstalledPlugin>,
    /// Explicit enabled/disabled overrides for built-in plugins (`name -> enabled`).
    #[serde(default)]
    pub builtin_overrides: BTreeMap<String, bool>,
}

/// Built-in or curated remote plugin catalog entry returned by [`search_plugins`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogPlugin {
    /// Plugin identifier.
    pub name: &'static str,
    /// Category tag.
    pub category: &'static str,
    /// Short summary.
    pub summary: &'static str,
    /// Source kind (`built-in` or `community`).
    pub kind: &'static str,
    /// Subpath inside the official plugins repository if remote.
    pub repo_subpath: Option<&'static str>,
}

const CATALOG: &[CatalogPlugin] = &[
    CatalogPlugin {
        name: "shield",
        category: "privacy",
        summary: "Strip tracking parameters, block privacy-invasive hosts, and apply Tor routing.",
        kind: "built-in",
        repo_subpath: None,
    },
    CatalogPlugin {
        name: "block-resources",
        category: "core",
        summary: "Block matching URLs or resource types before the browser fetches them.",
        kind: "built-in",
        repo_subpath: None,
    },
    CatalogPlugin {
        name: "crx",
        category: "extension",
        summary: "Load unpacked or packed browser extensions via LaunchSettings.",
        kind: "built-in",
        repo_subpath: None,
    },
    CatalogPlugin {
        name: "fetcher",
        category: "core",
        summary: "Fetch rules, response capture, and scripted HTTP tasks.",
        kind: "built-in",
        repo_subpath: None,
    },
    CatalogPlugin {
        name: "extractor",
        category: "core",
        summary: "Find and extract structured page data as JSON lines.",
        kind: "built-in",
        repo_subpath: None,
    },
    CatalogPlugin {
        name: "batcher",
        category: "environment",
        summary: "Run concurrent batches with retries and resume state.",
        kind: "built-in",
        repo_subpath: None,
    },
    CatalogPlugin {
        name: "crawler",
        category: "testing",
        summary: "Follow site links and report broken pages and console errors.",
        kind: "built-in",
        repo_subpath: None,
    },
    CatalogPlugin {
        name: "browser",
        category: "extension",
        summary: "Extension loader and persistent containerized browser profiles.",
        kind: "built-in",
        repo_subpath: None,
    },
    CatalogPlugin {
        name: "imap",
        category: "testing",
        summary: "Read verification codes or links from an IMAP mailbox.",
        kind: "built-in",
        repo_subpath: None,
    },
    CatalogPlugin {
        name: "tor",
        category: "privacy",
        summary: "Per-context Tor SOCKS routing.",
        kind: "built-in",
        repo_subpath: None,
    },
    CatalogPlugin {
        name: "antibot-detector",
        category: "community",
        summary: "Detect challenge pages and protection vendor signatures.",
        kind: "community",
        repo_subpath: Some("plugins/antibot-detector"),
    },
    CatalogPlugin {
        name: "behavior-dataset",
        category: "community",
        summary: "Record and replay interaction telemetry datasets.",
        kind: "community",
        repo_subpath: Some("plugins/behavior-dataset"),
    },
    CatalogPlugin {
        name: "cloud-logging",
        category: "community",
        summary: "Forward run logs and artifacts to cloud storage buckets.",
        kind: "community",
        repo_subpath: Some("plugins/cloud-logging"),
    },
    CatalogPlugin {
        name: "langchain",
        category: "community",
        summary: "Bridge browser extraction tasks into LangChain workflows.",
        kind: "community",
        repo_subpath: Some("plugins/langchain"),
    },
    CatalogPlugin {
        name: "ocr",
        category: "community",
        summary: "On-device OCR text recognition for page regions and captchas.",
        kind: "community",
        repo_subpath: Some("plugins/ocr"),
    },
    CatalogPlugin {
        name: "owasp",
        category: "community",
        summary: "Security header checks and OWASP ZAP bridge hooks.",
        kind: "community",
        repo_subpath: Some("plugins/owasp"),
    },
];

/// Summary returned after installing a plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallOutcome {
    /// Installed plugin record.
    pub plugin: InstalledPlugin,
    /// Human-readable status text.
    pub message: String,
}

/// Summary returned after upgrading a plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradeOutcome {
    /// Plugin name.
    pub name: String,
    /// New or refreshed version.
    pub version: String,
    /// Human-readable status detail.
    pub detail: String,
}

/// Search hit returned by [`search_plugins`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    /// Plugin name.
    pub name: String,
    /// Category or origin (`built-in`, `community`, `installed`).
    pub source_label: String,
    /// Short summary.
    pub summary: String,
    /// Whether it is currently installed locally in the plugin store.
    pub installed: bool,
}

/// Inspection of the active user plugin store directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryInfo {
    /// Root plugin directory (`<plugins_dir>`).
    pub root: PathBuf,
    /// Subdirectory storing installed plugins (`<plugins_dir>/installed`).
    pub installed_dir: PathBuf,
    /// Path to `registry.json`.
    pub registry_path: PathBuf,
    /// Whether `root` already exists on disk.
    pub exists: bool,
    /// Number of installed plugins recorded in `registry.json`.
    pub installed_count: usize,
}

/// Resolves the user plugin store directory using [`PLUGINS_DIR_ENV`], a caller
/// override, or the platform user config path (`~/.config/sbrs/plugins` /
/// `%APPDATA%\sbrs\plugins`).
#[must_use]
pub fn resolve_plugins_dir(custom: Option<&Path>) -> PathBuf {
    if let Some(dir) = custom {
        return dir.to_path_buf();
    }
    if let Some(env_dir) = std::env::var_os(PLUGINS_DIR_ENV).filter(|v| !v.is_empty()) {
        return PathBuf::from(env_dir);
    }
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(xdg).join("sbrs").join("plugins");
    }
    if let Some(appdata) = std::env::var_os("APPDATA").filter(|v| !v.is_empty()) {
        return PathBuf::from(appdata).join("sbrs").join("plugins");
    }
    if let Some(home) = std::env::var_os("HOME").filter(|v| !v.is_empty()) {
        #[cfg(target_os = "macos")]
        {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("sbrs")
                .join("plugins");
        }
        #[cfg(not(target_os = "macos"))]
        {
            return PathBuf::from(home)
                .join(".config")
                .join("sbrs")
                .join("plugins");
        }
    }
    PathBuf::from(".sbrs").join("plugins")
}

/// Validates and normalizes a plugin name.
///
/// # Errors
///
/// Returns [`PluginManagerError::InvalidName`] when empty or unsafe.
pub fn validate_name(raw: &str) -> Result<String, PluginManagerError> {
    let clean = raw.trim().to_ascii_lowercase().replace(' ', "-");
    if clean.is_empty() {
        return Err(PluginManagerError::InvalidName {
            name: raw.to_owned(),
            reason: "plugin name cannot be empty".to_owned(),
        });
    }
    if clean.contains("..")
        || clean.contains('/')
        || clean.contains('\\')
        || clean.starts_with('.')
        || clean.starts_with('-')
    {
        return Err(PluginManagerError::InvalidName {
            name: raw.to_owned(),
            reason: "must not contain path separators, '..', or start with '.' or '-'".to_owned(),
        });
    }
    Ok(clean)
}

/// Loads `registry.json` from `store_dir`, returning an empty registry if none exists yet.
///
/// # Errors
///
/// Returns [`PluginManagerError::InvalidRegistry`] if the file is corrupted or
/// [`PluginManagerError::Io`] on read failure.
pub fn load_registry(store_dir: &Path) -> Result<PluginRegistry, PluginManagerError> {
    let path = store_dir.join(REGISTRY_FILE);
    if !path.is_file() {
        return Ok(PluginRegistry::default());
    }
    let text = fs::read_to_string(&path).map_err(|source| PluginManagerError::Io {
        path: path.clone(),
        source,
    })?;
    serde_json::from_str(&text)
        .map_err(|source| PluginManagerError::InvalidRegistry { path, source })
}

fn save_registry(store_dir: &Path, registry: &PluginRegistry) -> Result<(), PluginManagerError> {
    fs::create_dir_all(store_dir).map_err(|source| PluginManagerError::Io {
        path: store_dir.to_path_buf(),
        source,
    })?;
    let path = store_dir.join(REGISTRY_FILE);
    let json = serde_json::to_string_pretty(registry).map_err(|source| {
        PluginManagerError::InvalidRegistry {
            path: path.clone(),
            source,
        }
    })?;
    fs::write(&path, format!("{json}\n")).map_err(|source| PluginManagerError::Io { path, source })
}

/// Inspects `store_dir` and optionally initializes `installed/` and `registry.json`.
///
/// # Errors
///
/// Returns [`PluginManagerError::Io`] or [`PluginManagerError::InvalidRegistry`].
pub fn directory_info(store_dir: &Path, ensure: bool) -> Result<DirectoryInfo, PluginManagerError> {
    let installed_dir = store_dir.join(INSTALLED_SUBDIR);
    let registry_path = store_dir.join(REGISTRY_FILE);
    if ensure {
        fs::create_dir_all(&installed_dir).map_err(|source| PluginManagerError::Io {
            path: installed_dir.clone(),
            source,
        })?;
        if !registry_path.is_file() {
            save_registry(store_dir, &PluginRegistry::default())?;
        }
    }
    let registry = load_registry(store_dir)?;
    Ok(DirectoryInfo {
        root: store_dir.to_path_buf(),
        installed_dir,
        registry_path,
        exists: store_dir.is_dir(),
        installed_count: registry.plugins.len(),
    })
}

/// Installs a plugin from a local directory, git/HTTPS URL, `owner/repo` shorthand,
/// or official community catalog name into `store_dir`.
///
/// # Errors
///
/// Returns [`PluginManagerError`] if the source is missing, the name is invalid,
/// the plugin is already installed (without `force`), or `git` fails.
pub fn install_plugin(
    store_dir: &Path,
    source: &str,
    name_override: Option<&str>,
    force: bool,
) -> Result<InstallOutcome, PluginManagerError> {
    let trimmed = source.trim();
    if trimmed.is_empty() {
        return Err(PluginManagerError::InvalidSource {
            source_str: source.to_owned(),
            reason: "source cannot be empty".to_owned(),
        });
    }

    let mut registry = load_registry(store_dir)?;

    if let Some(local_path) = resolve_local_source(trimmed) {
        if !local_path.is_dir() {
            return Err(PluginManagerError::InvalidSource {
                source_str: source.to_owned(),
                reason: format!("local path `{}` is not a directory", local_path.display()),
            });
        }
        let manifest = read_plugin_manifest(&local_path);
        let raw_name = name_override
            .map(str::to_owned)
            .or(manifest.name)
            .or_else(|| {
                local_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(str::to_owned)
            })
            .ok_or_else(|| PluginManagerError::InvalidSource {
                source_str: source.to_owned(),
                reason: "could not determine plugin name from local path".to_owned(),
            })?;
        let name = validate_name(&raw_name)?;
        if registry.plugins.contains_key(&name) && !force {
            return Err(PluginManagerError::AlreadyInstalled(name));
        }

        let dest = store_dir.join(INSTALLED_SUBDIR).join(&name);
        if dest.exists() {
            fs::remove_dir_all(&dest).map_err(|e| PluginManagerError::Io {
                path: dest.clone(),
                source: e,
            })?;
        }
        copy_plugin_dir(&local_path, &dest)?;

        let canonical_src = local_path.canonicalize().unwrap_or(local_path);
        let record = InstalledPlugin {
            name: name.clone(),
            version: manifest.version.unwrap_or_else(|| "0.1.0".to_owned()),
            summary: manifest.summary.unwrap_or_else(|| {
                format!("Local plugin installed from {}", canonical_src.display())
            }),
            enabled: true,
            origin: PluginInstallOrigin::Local {
                path: canonical_src.clone(),
            },
            installed_path: dest,
        };
        registry.plugins.insert(name.clone(), record.clone());
        save_registry(store_dir, &registry)?;
        return Ok(InstallOutcome {
            message: format!(
                "Installed local plugin `{}` ({}) from {} into {}",
                record.name,
                record.version,
                canonical_src.display(),
                record.installed_path.display()
            ),
            plugin: record,
        });
    }

    let (remote_url, subpath, default_name) = resolve_remote_source(trimmed)?;
    let raw_name = name_override.unwrap_or(&default_name);
    let name = validate_name(raw_name)?;
    if registry.plugins.contains_key(&name) && !force {
        return Err(PluginManagerError::AlreadyInstalled(name));
    }

    let dest = store_dir.join(INSTALLED_SUBDIR).join(&name);
    if dest.exists() {
        fs::remove_dir_all(&dest).map_err(|e| PluginManagerError::Io {
            path: dest.clone(),
            source: e,
        })?;
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| PluginManagerError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }

    git_clone(&remote_url, &dest)?;
    let manifest_dir = match &subpath {
        Some(sub) if dest.join(sub).is_dir() => dest.join(sub),
        _ => dest.clone(),
    };
    let manifest = read_plugin_manifest(&manifest_dir);
    let catalog_summary = CATALOG
        .iter()
        .find(|entry| entry.name == name)
        .map(|entry| entry.summary.to_owned());

    let record = InstalledPlugin {
        name: name.clone(),
        version: manifest.version.unwrap_or_else(|| "git".to_owned()),
        summary: manifest
            .summary
            .or(catalog_summary)
            .unwrap_or_else(|| format!("Remote plugin from {remote_url}")),
        enabled: true,
        origin: PluginInstallOrigin::Remote {
            url: remote_url.clone(),
            subpath,
        },
        installed_path: dest,
    };
    registry.plugins.insert(name.clone(), record.clone());
    save_registry(store_dir, &registry)?;

    Ok(InstallOutcome {
        message: format!(
            "Installed remote plugin `{}` ({}) from {} into {}",
            record.name,
            record.version,
            remote_url,
            record.installed_path.display()
        ),
        plugin: record,
    })
}

/// Uninstalls an installed user plugin from `store_dir`.
///
/// # Errors
///
/// Returns [`PluginManagerError::NotInstalled`] when absent or [`PluginManagerError::Io`] on removal failure.
pub fn uninstall_plugin(store_dir: &Path, raw_name: &str) -> Result<PathBuf, PluginManagerError> {
    let name = validate_name(raw_name)?;
    let mut registry = load_registry(store_dir)?;
    let Some(removed) = registry.plugins.remove(&name) else {
        return Err(PluginManagerError::NotInstalled(name));
    };
    if removed.installed_path.exists() {
        fs::remove_dir_all(&removed.installed_path).map_err(|source| PluginManagerError::Io {
            path: removed.installed_path.clone(),
            source,
        })?;
    }
    save_registry(store_dir, &registry)?;
    Ok(removed.installed_path)
}

/// Enables or disables a user-installed plugin (or sets an override for a built-in catalog plugin).
///
/// # Errors
///
/// Returns [`PluginManagerError::NotInstalled`] if the name is neither installed nor in [`CATALOG`].
pub fn set_plugin_enabled(
    store_dir: &Path,
    raw_name: &str,
    enabled: bool,
) -> Result<String, PluginManagerError> {
    let name = validate_name(raw_name)?;
    let mut registry = load_registry(store_dir)?;
    if let Some(entry) = registry.plugins.get_mut(&name) {
        entry.enabled = enabled;
        save_registry(store_dir, &registry)?;
        return Ok(format!(
            "Plugin `{name}` {}.",
            if enabled { "enabled" } else { "disabled" }
        ));
    }
    if CATALOG.iter().any(|item| item.name == name) {
        registry.builtin_overrides.insert(name.clone(), enabled);
        save_registry(store_dir, &registry)?;
        return Ok(format!(
            "Recorded default override for built-in plugin `{name}`: {}.",
            if enabled { "enabled" } else { "disabled" }
        ));
    }
    Err(PluginManagerError::NotInstalled(name))
}

/// Upgrades one named installed plugin or all installed plugins.
///
/// # Errors
///
/// Returns [`PluginManagerError`] if the named plugin is missing or upgrade fails.
pub fn upgrade_plugins(
    store_dir: &Path,
    target_name: Option<&str>,
) -> Result<Vec<UpgradeOutcome>, PluginManagerError> {
    let mut registry = load_registry(store_dir)?;
    let names: Vec<String> = match target_name {
        Some(raw) => {
            let name = validate_name(raw)?;
            if !registry.plugins.contains_key(&name) {
                return Err(PluginManagerError::NotInstalled(name));
            }
            vec![name]
        }
        None => registry.plugins.keys().cloned().collect(),
    };

    let mut outcomes = Vec::new();
    for name in names {
        let Some(existing) = registry.plugins.get(&name).cloned() else {
            continue;
        };
        match &existing.origin {
            PluginInstallOrigin::Local { path } => {
                if !path.is_dir() {
                    return Err(PluginManagerError::InvalidSource {
                        source_str: path.display().to_string(),
                        reason: "original local source directory no longer exists".to_owned(),
                    });
                }
                if existing.installed_path.exists() {
                    fs::remove_dir_all(&existing.installed_path).map_err(|source| {
                        PluginManagerError::Io {
                            path: existing.installed_path.clone(),
                            source,
                        }
                    })?;
                }
                copy_plugin_dir(path, &existing.installed_path)?;
                let manifest = read_plugin_manifest(path);
                let version = manifest.version.unwrap_or_else(|| existing.version.clone());
                if let Some(entry) = registry.plugins.get_mut(&name) {
                    entry.version = version.clone();
                    if let Some(summary) = manifest.summary {
                        entry.summary = summary;
                    }
                }
                outcomes.push(UpgradeOutcome {
                    name,
                    version,
                    detail: format!("re-synced from local source {}", path.display()),
                });
            }
            PluginInstallOrigin::Remote { url, subpath } => {
                if existing.installed_path.join(".git").exists() {
                    git_pull(&existing.installed_path)?;
                } else {
                    if existing.installed_path.exists() {
                        fs::remove_dir_all(&existing.installed_path).map_err(|source| {
                            PluginManagerError::Io {
                                path: existing.installed_path.clone(),
                                source,
                            }
                        })?;
                    }
                    git_clone(url, &existing.installed_path)?;
                }
                let manifest_dir = match subpath {
                    Some(sub) if existing.installed_path.join(sub).is_dir() => {
                        existing.installed_path.join(sub)
                    }
                    _ => existing.installed_path.clone(),
                };
                let manifest = read_plugin_manifest(&manifest_dir);
                let version = manifest.version.unwrap_or_else(|| existing.version.clone());
                if let Some(entry) = registry.plugins.get_mut(&name) {
                    entry.version = version.clone();
                }
                outcomes.push(UpgradeOutcome {
                    name,
                    version,
                    detail: format!("updated from {url}"),
                });
            }
        }
    }
    save_registry(store_dir, &registry)?;
    Ok(outcomes)
}

/// Searches built-in plugins, community catalog entries, and installed user plugins.
///
/// # Errors
///
/// Returns [`PluginManagerError`] if reading the local registry fails.
pub fn search_plugins(
    store_dir: &Path,
    query: Option<&str>,
) -> Result<Vec<SearchHit>, PluginManagerError> {
    let registry = load_registry(store_dir)?;
    let needle = query.unwrap_or("").trim().to_ascii_lowercase();
    let mut hits = Vec::new();

    for entry in CATALOG {
        let installed = registry.plugins.contains_key(entry.name);
        let matches = needle.is_empty()
            || entry.name.to_ascii_lowercase().contains(&needle)
            || entry.category.to_ascii_lowercase().contains(&needle)
            || entry.summary.to_ascii_lowercase().contains(&needle);
        if matches {
            hits.push(SearchHit {
                name: entry.name.to_owned(),
                source_label: format!("{} ({})", entry.kind, entry.category),
                summary: entry.summary.to_owned(),
                installed,
            });
        }
    }

    for (name, installed) in &registry.plugins {
        if CATALOG.iter().any(|c| c.name == name) {
            continue;
        }
        let matches = needle.is_empty()
            || name.to_ascii_lowercase().contains(&needle)
            || installed.summary.to_ascii_lowercase().contains(&needle);
        if matches {
            hits.push(SearchHit {
                name: name.clone(),
                source_label: "installed".to_owned(),
                summary: installed.summary.clone(),
                installed: true,
            });
        }
    }

    Ok(hits)
}

/// Formats installed user plugins for appending to `sbrs plugin list` output.
///
/// Returns `Ok(None)` when no user plugins are installed so the default output of
/// `sbrs plugin list` remains clean when the user store is empty.
///
/// # Errors
///
/// Returns [`PluginManagerError`] if the registry cannot be loaded.
pub fn format_installed_section(store_dir: &Path) -> Result<Option<String>, PluginManagerError> {
    let registry = load_registry(store_dir)?;
    if registry.plugins.is_empty() {
        return Ok(None);
    }
    let mut out = format!("\nInstalled plugins ({}):\n", store_dir.display());
    let name_width = registry
        .plugins
        .keys()
        .map(String::len)
        .max()
        .unwrap_or(4)
        .max(4);
    for plugin in registry.plugins.values() {
        let state = if plugin.enabled { "yes" } else { "no" };
        let origin_label = match &plugin.origin {
            PluginInstallOrigin::Local { .. } => "local",
            PluginInstallOrigin::Remote { .. } => "remote",
        };
        out.push_str(&format!(
            "  {:<name_width$}  {:<8}  {:<7}  {} ({})\n",
            plugin.name, plugin.version, state, plugin.summary, origin_label
        ));
    }
    Ok(Some(out))
}

/// Validates a local plugin crate or directory for `sbrs-dev plugin check`.
///
/// # Errors
///
/// Returns [`PluginManagerError::InvalidSource`] when required files are missing.
pub fn check_plugin_project(dir: &Path) -> Result<String, PluginManagerError> {
    if !dir.is_dir() {
        return Err(PluginManagerError::InvalidSource {
            source_str: dir.display().to_string(),
            reason: "directory does not exist".to_owned(),
        });
    }
    let cargo_toml = dir.join("Cargo.toml");
    let lib_rs = dir.join("src").join("lib.rs");
    if !cargo_toml.is_file() {
        return Err(PluginManagerError::InvalidSource {
            source_str: dir.display().to_string(),
            reason: "missing Cargo.toml".to_owned(),
        });
    }
    if !lib_rs.is_file() {
        return Err(PluginManagerError::InvalidSource {
            source_str: dir.display().to_string(),
            reason: "missing src/lib.rs".to_owned(),
        });
    }
    let manifest = read_plugin_manifest(dir);
    let name = manifest.name.unwrap_or_else(|| {
        dir.file_name()
            .and_then(OsStr::to_str)
            .unwrap_or("plugin")
            .to_owned()
    });
    let version = manifest.version.unwrap_or_else(|| "0.1.0".to_owned());
    Ok(format!(
        "ok: plugin crate `{name}` v{version} at {} (Cargo.toml and src/lib.rs present)",
        dir.display()
    ))
}

fn resolve_local_source(raw: &str) -> Option<PathBuf> {
    if let Some(stripped) = raw.strip_prefix("file://") {
        return Some(PathBuf::from(stripped));
    }
    let candidate = Path::new(raw);
    if raw.starts_with('.') || raw.starts_with('/') || raw.contains('\\') || candidate.exists() {
        return Some(candidate.to_path_buf());
    }
    None
}

fn resolve_remote_source(
    raw: &str,
) -> Result<(String, Option<String>, String), PluginManagerError> {
    if let Some(entry) = CATALOG.iter().find(|item| item.name == raw) {
        if let Some(sub) = entry.repo_subpath {
            return Ok((
                OFFICIAL_PLUGINS_REPO.to_owned(),
                Some(sub.to_owned()),
                entry.name.to_owned(),
            ));
        }
        return Err(PluginManagerError::InvalidSource {
            source_str: raw.to_owned(),
            reason: format!(
                "`{raw}` is already a built-in plugin; turn it on with `--plugin {raw}`"
            ),
        });
    }
    if raw.starts_with("https://")
        || raw.starts_with("http://")
        || raw.starts_with("ssh://")
        || raw.starts_with("git@")
    {
        let last = raw
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or("plugin")
            .trim_end_matches(".git");
        return Ok((raw.to_owned(), None, last.to_owned()));
    }
    if let Some((owner, repo)) = raw.split_once('/')
        && !owner.is_empty()
        && !repo.is_empty()
        && !repo.contains('/')
    {
        let clean_repo = repo.trim_end_matches(".git");
        return Ok((
            format!("https://github.com/{owner}/{clean_repo}.git"),
            None,
            clean_repo.to_owned(),
        ));
    }
    Err(PluginManagerError::InvalidSource {
        source_str: raw.to_owned(),
        reason:
            "expected an existing local folder, a git URL, owner/repo, or community plugin name"
                .to_owned(),
    })
}

#[derive(Default)]
struct ManifestInfo {
    name: Option<String>,
    version: Option<String>,
    summary: Option<String>,
}

fn read_plugin_manifest(dir: &Path) -> ManifestInfo {
    let cargo_path = dir.join("Cargo.toml");
    let Ok(text) = fs::read_to_string(cargo_path) else {
        return ManifestInfo::default();
    };
    let mut in_package = false;
    let mut info = ManifestInfo::default();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("name")
            && let Some(v) = quoted_value(rest)
        {
            info.name = Some(v);
        } else if let Some(rest) = trimmed.strip_prefix("version")
            && let Some(v) = quoted_value(rest)
        {
            info.version = Some(v);
        } else if let Some(rest) = trimmed.strip_prefix("description")
            && let Some(v) = quoted_value(rest)
        {
            info.summary = Some(v);
        }
    }
    info
}

fn quoted_value(after_key: &str) -> Option<String> {
    let rest = after_key.trim_start().strip_prefix('=')?.trim();
    let val = rest.strip_prefix('"')?.split('"').next()?;
    if val.is_empty() {
        None
    } else {
        Some(val.to_owned())
    }
}

fn copy_plugin_dir(src: &Path, dst: &Path) -> Result<(), PluginManagerError> {
    fs::create_dir_all(dst).map_err(|source| PluginManagerError::Io {
        path: dst.to_path_buf(),
        source,
    })?;
    let entries = fs::read_dir(src).map_err(|source| PluginManagerError::Io {
        path: src.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| PluginManagerError::Io {
            path: src.to_path_buf(),
            source,
        })?;
        let name = entry.file_name();
        if name == "target" || name == ".git" {
            continue;
        }
        let from = entry.path();
        let to = dst.join(name);
        if from.is_dir() {
            copy_plugin_dir(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(|source| PluginManagerError::Io { path: to, source })?;
        }
    }
    Ok(())
}

fn git_clone(url: &str, dest: &Path) -> Result<(), PluginManagerError> {
    let output = Command::new("git")
        .arg("clone")
        .arg("--depth")
        .arg("1")
        .arg(url)
        .arg(dest)
        .output()
        .map_err(|e| PluginManagerError::GitFailed {
            operation: "clone".to_owned(),
            details: e.to_string(),
        })?;
    if !output.status.success() {
        return Err(PluginManagerError::GitFailed {
            operation: "clone".to_owned(),
            details: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(())
}

fn git_pull(dest: &Path) -> Result<(), PluginManagerError> {
    let output = Command::new("git")
        .current_dir(dest)
        .args(["pull", "--ff-only"])
        .output()
        .map_err(|e| PluginManagerError::GitFailed {
            operation: "pull".to_owned(),
            details: e.to_string(),
        })?;
    if !output.status.success() {
        return Err(PluginManagerError::GitFailed {
            operation: "pull".to_owned(),
            details: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_sample_local_plugin(dir: &Path, name: &str, version: &str) {
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(
            dir.join("Cargo.toml"),
            format!(
                "[package]\nname = \"{name}\"\nversion = \"{version}\"\ndescription = \"Sample local plugin\"\n"
            ),
        )
        .unwrap();
        fs::write(
            dir.join("src/lib.rs"),
            "pub fn ping() -> &'static str { \"ok\" }\n",
        )
        .unwrap();
    }

    #[test]
    fn install_disable_enable_upgrade_and_uninstall_local_plugin() {
        let store = tempfile::tempdir().unwrap();
        let src_dir = tempfile::tempdir().unwrap();
        let plugin_src = src_dir.path().join("demo-plug");
        create_sample_local_plugin(&plugin_src, "demo-plug", "0.1.0");

        let info_before = directory_info(store.path(), true).unwrap();
        assert!(info_before.exists);
        assert_eq!(info_before.installed_count, 0);

        let installed =
            install_plugin(store.path(), &plugin_src.display().to_string(), None, false).unwrap();
        assert_eq!(installed.plugin.name, "demo-plug");
        assert!(installed.plugin.enabled);
        assert!(installed.plugin.installed_path.join("src/lib.rs").is_file());

        set_plugin_enabled(store.path(), "demo-plug", false).unwrap();
        let reg = load_registry(store.path()).unwrap();
        assert!(!reg.plugins["demo-plug"].enabled);

        set_plugin_enabled(store.path(), "demo-plug", true).unwrap();
        let reg = load_registry(store.path()).unwrap();
        assert!(reg.plugins["demo-plug"].enabled);

        create_sample_local_plugin(&plugin_src, "demo-plug", "0.2.0");
        let upgraded = upgrade_plugins(store.path(), Some("demo-plug")).unwrap();
        assert_eq!(upgraded.len(), 1);
        assert_eq!(upgraded[0].version, "0.2.0");

        let section = format_installed_section(store.path()).unwrap().unwrap();
        assert!(section.contains("demo-plug"));
        assert!(section.contains("0.2.0"));

        let removed_path = uninstall_plugin(store.path(), "demo-plug").unwrap();
        assert!(!removed_path.exists());
        assert!(load_registry(store.path()).unwrap().plugins.is_empty());
    }

    #[test]
    fn search_finds_catalog_and_installed_plugins() {
        let store = tempfile::tempdir().unwrap();
        let hits = search_plugins(store.path(), Some("owasp")).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "owasp");
        assert!(!hits[0].installed);
    }

    #[test]
    fn validate_name_rejects_traversal() {
        assert!(validate_name("../bad").is_err());
        assert!(validate_name("-bad").is_err());
        assert!(validate_name("good-plugin").is_ok());
    }
}
