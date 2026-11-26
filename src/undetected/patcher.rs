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
//! use seleniumbase_rs::evasion::patcher::{ChromedriverPatcher, EnginePatch};
//!
//! ChromedriverPatcher::new("chromedriver")
//!     .patch(EnginePatch::all())?;
//! ```

use crate::error::SeleniumBaseError;
use rand::RngExt;
use regex::bytes::Regex;
use ring::digest::{digest, SHA256};
use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::fmt::Write as _;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// Named preset profiles for binary automation marker patching.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PatchPreset {
    /// Conservative set of patches (`cdc_` properties, prefixes, core `__webdriver` globals) with `.orig` backup.
    Balanced,
    /// Full standard set of driver and browser markers with `.orig` backup.
    #[default]
    All,
    /// Maximum scrubbing including embedded `sourceURL` comments, DOM automation flags, and all extended globals.
    Aggressive,
    /// Preset tuned for cloning and patching Chrome/Chromium binaries in cache without mutating source files.
    ChromeBinary,
}

impl PatchPreset {
    /// Canonical lowercase preset name used by CLI options.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Balanced => "balanced",
            Self::All => "all",
            Self::Aggressive => "aggressive",
            Self::ChromeBinary => "chrome-binary",
        }
    }

    /// Converts this preset into an [`EnginePatch`] configuration.
    #[must_use]
    pub fn to_patch(self) -> EnginePatch {
        match self {
            Self::Balanced => EnginePatch::balanced(),
            Self::All => EnginePatch::all(),
            Self::Aggressive => EnginePatch::aggressive(),
            Self::ChromeBinary => EnginePatch::chrome_binary(),
        }
    }
}

impl fmt::Display for PatchPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for PatchPreset {
    type Err = SeleniumBaseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "balanced" => Ok(Self::Balanced),
            "all" => Ok(Self::All),
            "aggressive" => Ok(Self::Aggressive),
            "chrome-binary" | "chrome_binary" | "chrome" => Ok(Self::ChromeBinary),
            other => Err(SeleniumBaseError::patcher(
                other,
                format!(
                    "unknown patch preset '{other}' (expected balanced, all, aggressive, or chrome-binary)"
                ),
            )),
        }
    }
}

/// Detected executable container format read from leading magic bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BinaryFormat {
    /// ELF executable or shared object (Linux / BSD).
    Elf,
    /// Mach-O single-architecture or fat binary (macOS).
    MachO,
    /// PE/COFF `.exe` or `.dll` image (Windows).
    Pe,
    /// Unrecognized or plain script/fixture byte sequence.
    #[default]
    Unknown,
}

impl BinaryFormat {
    /// Identifies a binary format from its header magic bytes.
    #[must_use]
    pub fn detect(bytes: &[u8]) -> Self {
        if bytes.starts_with(b"\x7fELF") {
            return Self::Elf;
        }
        if bytes.starts_with(b"MZ") {
            return Self::Pe;
        }
        if bytes.len() >= 4 {
            let magic = [bytes[0], bytes[1], bytes[2], bytes[3]];
            if matches!(
                magic,
                [0xfe, 0xed, 0xfa, 0xce]
                    | [0xce, 0xfa, 0xed, 0xfe]
                    | [0xfe, 0xed, 0xfa, 0xcf]
                    | [0xcf, 0xfa, 0xed, 0xfe]
                    | [0xca, 0xfe, 0xba, 0xbe]
                    | [0xbe, 0xba, 0xfe, 0xca]
            ) {
                return Self::MachO;
            }
        }
        Self::Unknown
    }

    /// Human-readable format label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Elf => "ELF",
            Self::MachO => "Mach-O",
            Self::Pe => "PE",
            Self::Unknown => "Unknown",
        }
    }
}

impl fmt::Display for BinaryFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Equal-length custom byte replacement rule for binary patching.
///
/// Replacements preserve exact byte lengths so Mach-O, ELF, and PE section
/// offsets and virtual addresses remain intact after patching.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BinaryPatchRule {
    needle: Vec<u8>,
    replacement: Vec<u8>,
}

impl BinaryPatchRule {
    /// Validates and constructs an equal-length byte replacement rule.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Patcher`] when `needle` is empty or its
    /// byte length differs from `replacement`.
    pub fn try_new(
        needle: impl Into<Vec<u8>>,
        replacement: impl Into<Vec<u8>>,
    ) -> Result<Self, SeleniumBaseError> {
        let needle = needle.into();
        let replacement = replacement.into();
        if needle.is_empty() {
            return Err(SeleniumBaseError::patcher(
                "<custom-rule>",
                "binary patch needle cannot be empty",
            ));
        }
        if needle.len() != replacement.len() {
            return Err(SeleniumBaseError::patcher(
                String::from_utf8_lossy(&needle).into_owned(),
                format!(
                    "binary patch replacement length ({}) must match needle length ({})",
                    replacement.len(),
                    needle.len()
                ),
            ));
        }
        Ok(Self {
            needle,
            replacement,
        })
    }

    /// Builds a rule that overwrites `needle` with ASCII space bytes of the same length.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Patcher`] when `needle` is empty.
    pub fn neutralize(needle: impl AsRef<[u8]>) -> Result<Self, SeleniumBaseError> {
        let needle = needle.as_ref().to_vec();
        let spaces = vec![b' '; needle.len()];
        Self::try_new(needle, spaces)
    }

    /// Pattern bytes matched literally in the binary.
    #[must_use]
    pub fn needle(&self) -> &[u8] {
        &self.needle
    }

    /// Replacement bytes of identical length.
    #[must_use]
    pub fn replacement(&self) -> &[u8] {
        &self.replacement
    }
}

/// Breakdown of known automation and CDP markers detected inside a binary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct MarkerCounts {
    /// `window.cdc_<22-char>_...` property assignments or checks.
    pub cdc_properties: usize,
    /// Quoted `$cdc_<22-char>_` prefix literals.
    pub cdc_prefixes: usize,
    /// Inline `{window.cdc...;}` injection blocks.
    pub cdc_blocks: usize,
    /// Core `__webdriver`, `__selenium`, and `__driver` tokens.
    pub webdriver_markers: usize,
    /// Extended driver globals (`__fxdriver`, `__nightmare`, `_phantom`, `calledSelenium`, `$wdc_`, Watir/Edge markers).
    pub extra_driver_globals: usize,
    /// `domAutomation` and `domAutomationController` globals.
    pub dom_automation: usize,
    /// Embedded `ChromeDriver/<version>` or `EdgeDriver/<version>` strings.
    pub chrome_driver_versions: usize,
    /// `HeadlessChrome` user-agent token occurrences.
    pub headless_user_agent: usize,
    /// Embedded `navigator.webdriver = !0 / true` assignments.
    pub navigator_webdriver: usize,
    /// `kAutomationControlled`, `enable-automation`, and `webdriver-active` strings.
    pub automation_flags: usize,
    /// CDP runtime evaluation markers (`__puppeteer_evaluation_script__`, `__playwright_evaluation_script__`).
    pub cdp_isolation_markers: usize,
    /// Automation `sourceURL=` annotations (`//# sourceURL=...webdriver/cdc_/pptr:`).
    pub source_urls: usize,
}

impl MarkerCounts {
    /// Total number of detected automation markers across every category.
    #[must_use]
    pub const fn total(&self) -> usize {
        self.cdc_properties
            + self.cdc_prefixes
            + self.cdc_blocks
            + self.webdriver_markers
            + self.extra_driver_globals
            + self.dom_automation
            + self.chrome_driver_versions
            + self.headless_user_agent
            + self.navigator_webdriver
            + self.automation_flags
            + self.cdp_isolation_markers
            + self.source_urls
    }
}

/// Diagnostic inspection report describing automation markers inside a driver or browser binary.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct BinaryMarkerReport {
    /// Inspected file path.
    pub path: PathBuf,
    /// Container format inferred from header bytes.
    pub format: BinaryFormat,
    /// File length in bytes.
    pub size_bytes: usize,
    /// Lower-case SHA-256 hex digest of the inspected file.
    pub sha256: String,
    /// Per-family marker counts.
    pub counts: MarkerCounts,
}

impl BinaryMarkerReport {
    /// Reads and inspects a driver or browser binary on disk for automation markers.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Patcher`] if the file cannot be read.
    pub fn inspect(path: impl AsRef<Path>) -> Result<Self, SeleniumBaseError> {
        let path = path.as_ref();
        let content = fs::read(path).map_err(|e| {
            SeleniumBaseError::patcher(
                path.display().to_string(),
                format!("failed to read binary for inspection: {e}"),
            )
        })?;
        Ok(Self::from_bytes(path, &content))
    }

    /// Inspects an in-memory binary buffer and produces a [`BinaryMarkerReport`].
    #[must_use]
    pub fn from_bytes(path: impl AsRef<Path>, content: &[u8]) -> Self {
        inspect_bytes(path.as_ref(), content)
    }

    /// Returns `true` when core chromedriver/webdriver automation markers remain unpatched.
    #[must_use]
    pub const fn needs_patch(&self) -> bool {
        self.counts.cdc_properties
            + self.counts.cdc_prefixes
            + self.counts.cdc_blocks
            + self.counts.webdriver_markers
            + self.counts.chrome_driver_versions
            + self.counts.navigator_webdriver
            > 0
    }

    /// Returns `true` when any automation marker of any category was detected.
    #[must_use]
    pub const fn has_any_markers(&self) -> bool {
        self.counts.total() > 0
    }
}

impl fmt::Display for BinaryMarkerReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}, {} bytes, sha256: {}): {} total markers [cdc={}/{}/{}, webdriver={}, extra={}, dom_auto={}, version={}, headless_ua={}, nav_wd={}, flags={}, cdp_iso={}, source_urls={}]",
            self.path.display(),
            self.format,
            self.size_bytes,
            self.sha256,
            self.counts.total(),
            self.counts.cdc_properties,
            self.counts.cdc_prefixes,
            self.counts.cdc_blocks,
            self.counts.webdriver_markers,
            self.counts.extra_driver_globals,
            self.counts.dom_automation,
            self.counts.chrome_driver_versions,
            self.counts.headless_user_agent,
            self.counts.navigator_webdriver,
            self.counts.automation_flags,
            self.counts.cdp_isolation_markers,
            self.counts.source_urls,
        )
    }
}

/// Paths produced when caching and patching a browser executable along with any companion library.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct PatchedBrowserBundle {
    /// Patched primary browser executable inside the cache directory.
    pub executable: PathBuf,
    /// Patched companion framework or DLL (`Google Chrome Framework` on macOS, `chrome.dll` on Windows), if found.
    pub companion: Option<PathBuf>,
}

/// Kinds of binary patches that can be applied to a Chromium driver or browser binary.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
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
    /// Replace `kAutomationControlled` Chromium feature flags.
    pub scrub_k_automation_controlled: bool,
    /// Replace `--enable-automation` command line checks.
    pub scrub_enable_automation: bool,
    /// Replace CDP evaluation isolation script markers.
    pub patch_cdp_runtime_isolate: bool,
    /// Replace `domAutomation` / `domAutomationController` embedded identifiers.
    pub scrub_dom_automation: bool,
    /// Replace extended driver/Watir/Phantom/Nightmare/EdgeDriver globals.
    pub scrub_extra_driver_globals: bool,
    /// Replace additional CDP/automation state tokens (`webdriver-active`, `AutomationControlled`).
    pub scrub_cdp_serialization_markers: bool,
    /// Scrub embedded `//# sourceURL=...` comments naming automation tools.
    pub neutralize_source_urls: bool,
    /// Additional user-supplied equal-length byte replacement rules.
    pub custom_rules: Vec<BinaryPatchRule>,
    /// Create a `.orig` backup before patching.
    pub backup: bool,
}

impl EnginePatch {
    /// Builds a patch configuration from a [`PatchPreset`].
    #[must_use]
    pub fn from_preset(preset: PatchPreset) -> Self {
        preset.to_patch()
    }

    /// A conservative set of patches with backup enabled.
    #[must_use]
    pub fn balanced() -> Self {
        Self {
            scrub_cdc_props: true,
            randomize_cdc_prefix: true,
            scrub_webdriver_markers: true,
            backup: true,
            ..Self::default()
        }
    }

    /// All standard driver and browser marker patches with backup enabled.
    #[must_use]
    pub fn all() -> Self {
        Self {
            scrub_cdc_props: true,
            randomize_cdc_prefix: true,
            scrub_webdriver_markers: true,
            replace_cdc_blocks: true,
            scrub_chrome_version: true,
            patch_navigator_webdriver: true,
            patch_headless_user_agent: true,
            scrub_k_automation_controlled: true,
            scrub_enable_automation: true,
            patch_cdp_runtime_isolate: true,
            scrub_dom_automation: true,
            scrub_extra_driver_globals: true,
            scrub_cdp_serialization_markers: true,
            neutralize_source_urls: false,
            custom_rules: Vec::new(),
            backup: true,
        }
    }

    /// Maximum scrubbing preset including `sourceURL` comment neutralization.
    #[must_use]
    pub fn aggressive() -> Self {
        Self {
            neutralize_source_urls: true,
            ..Self::all()
        }
    }

    /// No backup; useful in CI where the binary is ephemeral.
    #[must_use]
    pub fn no_backup() -> Self {
        Self {
            backup: false,
            ..Self::all()
        }
    }

    /// Patches suitable for the Chrome/Chromium browser binary itself.
    #[must_use]
    pub fn chrome_binary() -> Self {
        Self {
            scrub_cdc_props: false,
            randomize_cdc_prefix: false,
            replace_cdc_blocks: false,
            scrub_chrome_version: false,
            scrub_webdriver_markers: true,
            patch_navigator_webdriver: true,
            patch_headless_user_agent: true,
            scrub_k_automation_controlled: true,
            scrub_enable_automation: true,
            patch_cdp_runtime_isolate: true,
            scrub_dom_automation: true,
            scrub_extra_driver_globals: true,
            scrub_cdp_serialization_markers: true,
            neutralize_source_urls: false,
            custom_rules: Vec::new(),
            backup: false,
        }
    }

    /// Appends a custom equal-length byte replacement rule to run after built-in passes.
    #[must_use]
    pub fn custom_rule(mut self, rule: BinaryPatchRule) -> Self {
        self.custom_rules.push(rule);
        self
    }

    fn cache_fingerprint(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        // Bump format revision when built-in replacement tables change.
        3u8.hash(&mut hasher);
        self.scrub_cdc_props.hash(&mut hasher);
        self.randomize_cdc_prefix.hash(&mut hasher);
        self.scrub_webdriver_markers.hash(&mut hasher);
        self.replace_cdc_blocks.hash(&mut hasher);
        self.scrub_chrome_version.hash(&mut hasher);
        self.patch_navigator_webdriver.hash(&mut hasher);
        self.patch_headless_user_agent.hash(&mut hasher);
        self.scrub_k_automation_controlled.hash(&mut hasher);
        self.scrub_enable_automation.hash(&mut hasher);
        self.patch_cdp_runtime_isolate.hash(&mut hasher);
        self.scrub_dom_automation.hash(&mut hasher);
        self.scrub_extra_driver_globals.hash(&mut hasher);
        self.scrub_cdp_serialization_markers.hash(&mut hasher);
        self.neutralize_source_urls.hash(&mut hasher);
        self.custom_rules.hash(&mut hasher);
        hasher.finish()
    }
}

/// Replaces the file at `path` with `content` in one step.
///
/// The new bytes are written beside it and renamed over it, keeping the
/// original's permissions. Writing into the file itself fails while it is
/// running (`ETXTBSY` on Linux) and leaves a half-written binary if the write
/// is interrupted; a rename does neither.
fn replace_file(path: &Path, content: &[u8]) -> std::io::Result<()> {
    let mut temp = path.as_os_str().to_owned();
    temp.push(".patching");
    let temp = PathBuf::from(temp);
    let written = fs::write(&temp, content)
        .and_then(|()| fs::set_permissions(&temp, fs::metadata(path)?.permissions()))
        .and_then(|()| fs::rename(&temp, path));
    if written.is_err() {
        let _ = fs::remove_file(&temp);
    }
    written
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
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Patcher`] if reading, backing up, writing,
    /// or re-signing the binary fails.
    pub fn patch(&self, spec: EnginePatch) -> Result<(), SeleniumBaseError> {
        let path = self.path.as_ref();
        let path_str = path.display().to_string();
        // The first backup is the original. A later patch must not replace it
        // with a file that is already patched, or nothing is left to restore.
        if spec.backup && !self.backup_path().exists() {
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

        let content = fs::read(path).map_err(|e| {
            SeleniumBaseError::patcher(&path_str, format!("failed to read chromedriver: {e}"))
        })?;
        let patched = apply_binary_patches(content, &spec);

        replace_file(path, &patched).map_err(|e| {
            let err = SeleniumBaseError::patcher(
                &path_str,
                format!("failed to write patched chromedriver: {e}"),
            );
            err.log_in_context("ChromedriverPatcher::patch");
            err
        })?;
        // Changing the bytes of a signed binary breaks its signature, and
        // macOS kills a program whose signature does not match.
        if cfg!(target_os = "macos") {
            crate::browser::driver_access::make_runnable(
                path,
                crate::browser::driver_access::CodeSignature::Resign,
            )?;
        }
        Ok(())
    }

    /// Restores the binary from the `.orig` backup if it exists.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Patcher`] when no `.orig` backup exists or
    /// restoring the file fails.
    pub fn restore(&self) -> Result<(), SeleniumBaseError> {
        let path_str = self.path.as_ref().display().to_string();
        let backup = self.backup_path();
        if !backup.exists() {
            return Err(SeleniumBaseError::patcher(
                &path_str,
                "no .orig backup found to restore",
            ));
        }
        let original = fs::read(&backup).map_err(|e| {
            SeleniumBaseError::patcher(
                &path_str,
                format!("failed to read the backup {}: {}", backup.display(), e),
            )
        })?;
        replace_file(self.path.as_ref(), &original).map_err(|e| {
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

    /// Inspects the driver binary and returns counts for every known marker family.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Patcher`] if the binary cannot be read.
    pub fn inspect(&self) -> Result<BinaryMarkerReport, SeleniumBaseError> {
        BinaryMarkerReport::inspect(self.path.as_ref())
    }

    /// Returns `true` when known core automation markers are still present.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Patcher`] if reading the binary fails.
    pub fn needs_patch(&self) -> Result<bool, SeleniumBaseError> {
        let path_str = self.path.as_ref().display().to_string();
        let content = fs::read(self.path.as_ref()).map_err(|e| {
            let err =
                SeleniumBaseError::patcher(&path_str, format!("failed to read chromedriver: {e}"));
            err.log_in_context("ChromedriverPatcher::needs_patch");
            err
        })?;
        Ok(inspect_bytes(self.path.as_ref(), &content).needs_patch())
    }

    /// Returns `true` when core chromedriver automation markers are no longer present.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Patcher`] if reading the binary fails.
    pub fn verify_clean(&self) -> Result<bool, SeleniumBaseError> {
        Ok(!self.needs_patch()?)
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
    #[must_use]
    pub fn cache_dir(mut self, dir: impl AsRef<Path>) -> Self {
        self.cache_dir = Some(dir.as_ref().to_path_buf());
        self
    }

    /// Returns the source binary path.
    #[must_use]
    pub fn source(&self) -> &Path {
        &self.source
    }

    /// Computes the cached patched-binary path for [`EnginePatch::chrome_binary()`].
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Patcher`] when no cache directory is known
    /// or reading source binary metadata fails.
    pub fn patched_path(&self) -> Result<PathBuf, SeleniumBaseError> {
        self.patched_path_for(&EnginePatch::chrome_binary())
    }

    /// Computes the cached patched-binary path for a specific [`EnginePatch`].
    ///
    /// The hash incorporates the source path, modification time, and the full
    /// `spec` fingerprint (preset flags and custom rules), so different patch
    /// presets never collide in the cache.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Patcher`] when no cache directory is known
    /// or reading source binary metadata fails.
    pub fn patched_path_for(&self, spec: &EnginePatch) -> Result<PathBuf, SeleniumBaseError> {
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
        let spec_fp = spec.cache_fingerprint();
        let mut hasher = DefaultHasher::new();
        source_str.hash(&mut hasher);
        mtime.hash(&mut hasher);
        spec_fp.hash(&mut hasher);
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

    /// Inspects the source Chrome/Chromium binary without modifying it.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Patcher`] if reading the binary fails.
    pub fn inspect(&self) -> Result<BinaryMarkerReport, SeleniumBaseError> {
        BinaryMarkerReport::inspect(&self.source)
    }

    /// Locates the companion shared library alongside a browser executable, if present.
    ///
    /// Checks macOS `Contents/Frameworks/{Google Chrome,Chromium,Microsoft Edge} Framework.framework/...`
    /// as well as sibling or version-directory shared libraries (`chrome.dll`, `msedge.dll`,
    /// `libchrome.so`, `libchromium.so`).
    #[must_use]
    pub fn find_companion_binary(&self) -> Option<PathBuf> {
        find_companion_binary_for(&self.source)
    }

    /// Inspects both the source executable and any discovered companion library.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Patcher`] if reading either binary fails.
    pub fn inspect_bundle(&self) -> Result<Vec<BinaryMarkerReport>, SeleniumBaseError> {
        let mut reports = vec![self.inspect()?];
        if let Some(companion) = self.find_companion_binary() {
            reports.push(BinaryMarkerReport::inspect(companion)?);
        }
        Ok(reports)
    }

    /// Applies `spec` to a cached copy of the source binary and returns its path.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError`] when the binary is missing or reading/writing fails.
    pub fn patch(&self, spec: EnginePatch) -> Result<PathBuf, SeleniumBaseError> {
        let source_str = self.source.display().to_string();
        if !self.source.exists() {
            return Err(SeleniumBaseError::browser_launch(
                source_str,
                "chrome binary not found",
            ));
        }

        let out = self.patched_path_for(&spec)?;
        patch_to_cached_copy(&self.source, &out, &spec)?;
        Ok(out)
    }

    /// Applies `spec` to a cached copy of both the primary executable and any companion library.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError`] when reading, patching, or writing either file fails.
    pub fn patch_bundle(
        &self,
        spec: EnginePatch,
    ) -> Result<PatchedBrowserBundle, SeleniumBaseError> {
        let executable = self.patch(spec.clone())?;
        let companion = match (self.find_companion_binary(), executable.parent()) {
            (Some(comp_src), Some(cache_dir)) => {
                let file_name = comp_src
                    .file_name()
                    .unwrap_or_else(|| std::ffi::OsStr::new("companion.bin"));
                let comp_out = cache_dir.join(file_name);
                patch_to_cached_copy(&comp_src, &comp_out, &spec)?;
                Some(comp_out)
            }
            _ => None,
        };
        Ok(PatchedBrowserBundle {
            executable,
            companion,
        })
    }
}

fn patch_to_cached_copy(
    source: &Path,
    out: &Path,
    spec: &EnginePatch,
) -> Result<(), SeleniumBaseError> {
    let source_str = source.display().to_string();
    if out.exists() {
        tracing::debug!(path = %out.display(), "reusing cached patched binary");
        return Ok(());
    }

    tracing::info!(source = %source_str, dest = %out.display(), "patching browser binary copy");
    let content = fs::read(source).map_err(|e| {
        SeleniumBaseError::patcher(
            source_str.clone(),
            format!("failed to read chrome binary: {e}"),
        )
    })?;

    let patched = apply_binary_patches(content, spec);

    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            SeleniumBaseError::patcher(
                source_str.clone(),
                format!("failed to create patch cache dir: {e}"),
            )
        })?;
    }
    fs::write(out, patched).map_err(|e| {
        SeleniumBaseError::patcher(
            source_str.clone(),
            format!("failed to write patched chrome binary: {e}"),
        )
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(out)
            .map_err(|e| {
                SeleniumBaseError::patcher(
                    source_str.clone(),
                    format!("failed to read patched binary permissions: {e}"),
                )
            })?
            .permissions();
        perms.set_mode(perms.mode() | 0o111);
        fs::set_permissions(out, perms).map_err(|e| {
            SeleniumBaseError::patcher(
                source_str.clone(),
                format!("failed to make patched binary executable: {e}"),
            )
        })?;
    }

    #[cfg(target_os = "macos")]
    strip_binary_signature(out)?;

    tracing::info!(path = %out.display(), "patched binary ready");
    Ok(())
}

fn find_companion_binary_for(source: &Path) -> Option<PathBuf> {
    let parent = source.parent()?;
    if parent.file_name().and_then(|n| n.to_str()) == Some("MacOS") {
        if let Some(contents) = parent.parent() {
            let frameworks = contents.join("Frameworks");
            for framework_name in [
                "Google Chrome Framework",
                "Chromium Framework",
                "Microsoft Edge Framework",
            ] {
                let bundle = frameworks.join(format!("{framework_name}.framework"));
                let current = bundle.join("Versions").join("Current").join(framework_name);
                if current.exists() {
                    return Some(current);
                }
                if let Ok(entries) = fs::read_dir(bundle.join("Versions")) {
                    for entry in entries.flatten() {
                        let candidate = entry.path().join(framework_name);
                        if candidate.is_file() {
                            return Some(candidate);
                        }
                    }
                }
                let direct = bundle.join(framework_name);
                if direct.exists() {
                    return Some(direct);
                }
            }
        }
    }

    let lib_names = ["chrome.dll", "msedge.dll", "libchrome.so", "libchromium.so"];
    for name in lib_names {
        let sibling = parent.join(name);
        if sibling.is_file() {
            return Some(sibling);
        }
    }
    if let Ok(entries) = fs::read_dir(parent) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                for name in lib_names {
                    let nested = path.join(name);
                    if nested.is_file() {
                        return Some(nested);
                    }
                }
            }
        }
    }
    None
}

/// Attempts to locate a system Chrome/Chromium binary.
///
/// Calls [`find_system_chrome_with`] using [`std::env::var`].
#[must_use]
pub fn find_system_chrome() -> Option<PathBuf> {
    find_system_chrome_with(|name| std::env::var(name).ok())
}

/// Attempts to locate a system Chrome/Chromium binary, reading environment variables through `lookup`.
///
/// Checks, in order:
/// * The `CHROME_BIN` / `CHROMIUM_BIN` environment variables returned by `lookup`.
/// * Common well-known paths per platform.
/// * `which google-chrome` / `which chromium` / `which chromium-browser`.
#[must_use]
pub fn find_system_chrome_with(lookup: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    if let Some(path) = lookup("CHROME_BIN").filter(|s| !s.trim().is_empty()) {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }
    if let Some(path) = lookup("CHROMIUM_BIN").filter(|s| !s.trim().is_empty()) {
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
            let local = lookup("LOCALAPPDATA").unwrap_or_default();
            let pf = lookup("PROGRAMFILES").unwrap_or_default();
            let pf_x86 = lookup("PROGRAMFILES(X86)").unwrap_or_default();
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

struct MarkerPatterns {
    cdc_properties: Regex,
    cdc_prefixes: Regex,
    cdc_blocks: Regex,
    webdriver_markers: Regex,
    extra_driver_globals: Regex,
    dom_automation: Regex,
    chrome_driver_versions: Regex,
    headless_user_agent: Regex,
    navigator_webdriver: Regex,
    automation_flags: Regex,
    cdp_isolation_markers: Regex,
    source_urls: Regex,
}

fn marker_patterns() -> &'static MarkerPatterns {
    static PATTERNS: std::sync::OnceLock<MarkerPatterns> = std::sync::OnceLock::new();
    PATTERNS.get_or_init(|| MarkerPatterns {
        cdc_properties: Regex::new(r"window\.cdc_[a-zA-Z0-9]{22}_")
            .expect("valid static cdc_properties regex"),
        cdc_prefixes: Regex::new(r#"['\"]?\$cdc_[a-zA-Z0-9]{22}_['\"]?"#)
            .expect("valid static cdc_prefixes regex"),
        cdc_blocks: Regex::new(r"\{window\.cdc.*?;\}").expect("valid static cdc_blocks regex"),
        webdriver_markers: Regex::new(r"__webdriver|__selenium|__driver")
            .expect("valid static webdriver_markers regex"),
        extra_driver_globals: Regex::new(
            r"__fxdriver|__nightmare|_phantom|callPhantom|calledSelenium|\$wdc_|__lastWatir",
        )
        .expect("valid static extra_driver_globals regex"),
        dom_automation: Regex::new(r"domAutomationController|domAutomation")
            .expect("valid static dom_automation regex"),
        chrome_driver_versions: Regex::new(r"(?:Chrome|Edge)Driver/\d+\.\d+\.\d+\.\d+")
            .expect("valid static chrome_driver_versions regex"),
        headless_user_agent: Regex::new(r"HeadlessChrome")
            .expect("valid static headless_user_agent regex"),
        navigator_webdriver: Regex::new(r"navigator\.webdriver\s*=\s*(!0|true)")
            .expect("valid static navigator_webdriver regex"),
        automation_flags: Regex::new(
            r"kAutomationControlled|AutomationControlled|enable-automation|webdriver-active",
        )
        .expect("valid static automation_flags regex"),
        cdp_isolation_markers: Regex::new(
            r"__puppeteer_evaluation_script__|__playwright_evaluation_script__",
        )
        .expect("valid static cdp_isolation_markers regex"),
        source_urls: Regex::new(
            r"//[#@]\s*sourceURL\s*=[^\n\r\x00]*(?:webdriver|cdc_|pptr:|puppeteer|playwright)[^\n\r\x00]*",
        )
        .expect("valid static source_urls regex"),
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    let sum = digest(&SHA256, bytes);
    let mut out = String::with_capacity(sum.as_ref().len() * 2);
    for byte in sum.as_ref() {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn inspect_bytes(path: &Path, content: &[u8]) -> BinaryMarkerReport {
    let p = marker_patterns();
    let counts = MarkerCounts {
        cdc_properties: p.cdc_properties.find_iter(content).count(),
        cdc_prefixes: p.cdc_prefixes.find_iter(content).count(),
        cdc_blocks: p.cdc_blocks.find_iter(content).count(),
        webdriver_markers: p.webdriver_markers.find_iter(content).count(),
        extra_driver_globals: p.extra_driver_globals.find_iter(content).count(),
        dom_automation: p.dom_automation.find_iter(content).count(),
        chrome_driver_versions: p.chrome_driver_versions.find_iter(content).count(),
        headless_user_agent: p.headless_user_agent.find_iter(content).count(),
        navigator_webdriver: p.navigator_webdriver.find_iter(content).count(),
        automation_flags: p.automation_flags.find_iter(content).count(),
        cdp_isolation_markers: p.cdp_isolation_markers.find_iter(content).count(),
        source_urls: p.source_urls.find_iter(content).count(),
    };
    BinaryMarkerReport {
        path: path.to_path_buf(),
        format: BinaryFormat::detect(content),
        size_bytes: content.len(),
        sha256: sha256_hex(content),
        counts,
    }
}

fn apply_binary_patches(mut content: Vec<u8>, spec: &EnginePatch) -> Vec<u8> {
    if spec.scrub_cdc_props {
        content = scrub_cdc_property_assignments(content);
    }
    if spec.randomize_cdc_prefix {
        content = randomize_cdc_string_prefix(content);
    }
    if spec.replace_cdc_blocks {
        content = replace_cdc_blocks(content);
    }
    if spec.scrub_chrome_version {
        content = scrub_chrome_version_strings(content);
        content = replace_literal_pairs(
            content,
            &[
                (b"ChromeDriver", b"Chrome      "),
                (b"EdgeDriver", b"Edge      "),
            ],
        );
    }
    if spec.scrub_webdriver_markers {
        content = replace_literal_pairs(
            content,
            &[
                (b"__webdriver", b"__000000000"),
                (b"__selenium", b"__00000000"),
                (b"__driver", b"__000000"),
            ],
        );
    }
    if spec.scrub_extra_driver_globals {
        content = replace_literal_pairs(
            content,
            &[
                (b"__fxdriver", b"__00000000"),
                (b"__nightmare", b"__000000000"),
                (b"_phantom", b"_0000000"),
                (b"callPhantom", b"call0000000"),
                (b"calledSelenium", b"called00000000"),
                (b"$wdc_", b"$000_"),
                (b"__lastWatir", b"__last00000"),
            ],
        );
    }
    if spec.scrub_dom_automation {
        content = replace_literal_pairs(
            content,
            &[
                (b"domAutomationController", b"dom00000000000000000000"),
                (b"domAutomation", b"dom0000000000"),
            ],
        );
    }
    if spec.patch_headless_user_agent {
        content = replace_literal_equal_len(&content, b"HeadlessChrome", b"Chrome        ");
    }
    if spec.scrub_k_automation_controlled {
        content =
            replace_literal_equal_len(&content, b"kAutomationControlled", b"kNotControlled0000000");
    }
    if spec.scrub_enable_automation {
        content = replace_literal_equal_len(&content, b"enable-automation", b"x-not-automation0");
    }
    if spec.scrub_cdp_serialization_markers {
        content = replace_literal_pairs(
            content,
            &[
                (b"webdriver-active", b"browser-active00"),
                (b"AutomationControlled", b"NotControlled0000000"),
            ],
        );
    }
    if spec.patch_cdp_runtime_isolate {
        content = replace_literal_pairs(
            content,
            &[
                (
                    b"__puppeteer_evaluation_script__",
                    b"__user_client_script_executed__",
                ),
                (
                    b"__playwright_evaluation_script__",
                    b"__user_client_script_executed0__",
                ),
            ],
        );
    }
    if spec.patch_navigator_webdriver {
        content = patch_navigator_webdriver_assignments(content);
    }
    if spec.neutralize_source_urls {
        content = scrub_source_urls(content);
    }
    for rule in &spec.custom_rules {
        content = replace_literal_equal_len(&content, rule.needle(), rule.replacement());
    }
    content
}

fn replace_literal_pairs(mut content: Vec<u8>, pairs: &[(&[u8], &[u8])]) -> Vec<u8> {
    for &(needle, replacement) in pairs {
        content = replace_literal_equal_len(&content, needle, replacement);
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

fn scrub_cdc_property_assignments(mut content: Vec<u8>) -> Vec<u8> {
    static RE_ASSIGN: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    static RE_OR: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();

    let re_assign = RE_ASSIGN.get_or_init(|| {
        Regex::new(
            r"window\.cdc_[a-zA-Z0-9]{22}_(Array|Promise|Symbol|Object|Proxy|JSON|Window|Document)\s*=\s*window\.(Array|Promise|Symbol|Object|Proxy|JSON|Window|Document);",
        )
        .expect("valid static cdc_assign regex")
    });
    content = re_assign
        .replace_all(&content, |caps: &regex::bytes::Captures| {
            vec![b' '; caps[0].len()]
        })
        .into_owned();

    let re_or = RE_OR.get_or_init(|| {
        Regex::new(
            r"window\.cdc_[a-zA-Z0-9]{22}_(Array|Promise|Symbol|Object|Proxy|JSON|Window|Document)\s*\|\|",
        )
        .expect("valid static cdc_or regex")
    });
    content = re_or
        .replace_all(&content, |caps: &regex::bytes::Captures| {
            vec![b' '; caps[0].len()]
        })
        .into_owned();
    content
}

fn randomize_cdc_string_prefix(mut content: Vec<u8>) -> Vec<u8> {
    static RE_STMT: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    static RE_TOKEN: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();

    let re_stmt = RE_STMT.get_or_init(|| {
        Regex::new(r#"['\"]\$cdc_[a-zA-Z0-9]{22}_['\"];"#).expect("valid static cdc_stmt regex")
    });
    content = re_stmt
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

    let re_token = RE_TOKEN.get_or_init(|| {
        Regex::new(r"\$cdc_[a-zA-Z0-9]{22}_").expect("valid static cdc_token regex")
    });
    content = re_token
        .replace_all(&content, |caps: &regex::bytes::Captures| {
            let len = caps[0].len();
            let mut rng = rand::rng();
            let mut out = Vec::with_capacity(len);
            out.push(b'$');
            // Start with non-'c' letter so the replacement never re-matches `$cdc_`.
            out.push(rng.random_range(b'd'..=b'z'));
            for _ in 2..len.saturating_sub(1) {
                out.push(rng.random_range(b'a'..=b'z'));
            }
            if len > 2 {
                out.push(b'_');
            }
            out
        })
        .into_owned();
    content
}

fn replace_cdc_blocks(content: Vec<u8>) -> Vec<u8> {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE
        .get_or_init(|| Regex::new(r"\{window\.cdc.*?;\}").expect("valid static cdc_block regex"));
    re.replace_all(&content, |caps: &regex::bytes::Captures| {
        let mut out = b"{console.log(\"chromedriver is undetectable!\")}".to_vec();
        if out.len() < caps[0].len() {
            out.extend(vec![b' '; caps[0].len() - out.len()]);
        } else {
            out.truncate(caps[0].len());
        }
        out
    })
    .into_owned()
}

fn scrub_chrome_version_strings(content: Vec<u8>) -> Vec<u8> {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?:Chrome|Edge)Driver/\d+\.\d+\.\d+\.\d+")
            .expect("valid static driver_version regex")
    });
    re.replace_all(&content, |caps: &regex::bytes::Captures| {
        vec![b' '; caps[0].len()]
    })
    .into_owned()
}

fn patch_navigator_webdriver_assignments(content: Vec<u8>) -> Vec<u8> {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(navigator\.webdriver\s*=\s*)(!0|true)")
            .expect("valid static navigator.webdriver regex")
    });
    re.replace_all(&content, |caps: &regex::bytes::Captures| {
        let full_len = caps[0].len();
        let prefix = &caps[1];
        let mut out = Vec::with_capacity(full_len);
        out.extend_from_slice(prefix);
        out.extend_from_slice(b"!1");
        out.resize(full_len, b' ');
        out
    })
    .into_owned()
}

fn scrub_source_urls(content: Vec<u8>) -> Vec<u8> {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(
            r"//[#@]\s*sourceURL\s*=[^\n\r\x00]*(?:webdriver|cdc_|pptr:|puppeteer|playwright)[^\n\r\x00]*",
        )
        .expect("valid static sourceURL regex")
    });
    re.replace_all(&content, |caps: &regex::bytes::Captures| {
        vec![b' '; caps[0].len()]
    })
    .into_owned()
}

/// Convenience one-shot patcher preserving the old API.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::Patcher`] if patching `path` fails.
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

#[cfg(test)]
mod backup_tests {
    use super::*;

    fn driver_in(dir: &Path) -> PathBuf {
        let path = dir.join("chromedriver");
        fs::write(&path, b"head __webdriver_evaluate tail").unwrap();
        path
    }

    #[test]
    fn a_second_patch_keeps_the_first_backup_so_restore_returns_the_original() {
        let dir = tempfile::tempdir().unwrap();
        let path = driver_in(dir.path());
        let original = fs::read(&path).unwrap();
        let patcher = ChromedriverPatcher::new(&path);

        patcher.patch(EnginePatch::balanced()).unwrap();
        let patched = fs::read(&path).unwrap();
        assert_ne!(patched, original, "the patch changed nothing; test is void");
        assert_eq!(fs::read(patcher.backup_path()).unwrap(), original);

        patcher.patch(EnginePatch::balanced()).unwrap();
        assert_eq!(
            fs::read(patcher.backup_path()).unwrap(),
            original,
            "the second patch replaced the backup with a patched file"
        );

        patcher.restore().unwrap();
        assert_eq!(fs::read(&path).unwrap(), original);
    }

    #[cfg(unix)]
    #[test]
    fn patching_keeps_the_executable_bit_and_leaves_no_temp_file() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = driver_in(dir.path());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();

        ChromedriverPatcher::new(&path)
            .patch(EnginePatch::balanced())
            .unwrap();

        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o755
        );
        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".patching"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    #[test]
    fn find_system_chrome_with_lookup_reads_mocked_env() {
        let dir = tempfile::tempdir().unwrap();
        let fake_chrome = dir.path().join("fake_chrome");
        fs::write(&fake_chrome, b"binary").unwrap();

        let found = find_system_chrome_with(|name| match name {
            "CHROME_BIN" => Some(fake_chrome.to_str().unwrap().to_owned()),
            _ => None,
        });
        assert_eq!(found, Some(fake_chrome));

        let not_found = find_system_chrome_with(|name| match name {
            "CHROME_BIN" => Some("/nonexistent/chrome/path".to_owned()),
            "CHROMIUM_BIN" => Some("/nonexistent/chromium/path".to_owned()),
            _ => None,
        });
        // May find system chrome from well-known paths or which, or None
        if let Some(p) = not_found {
            assert!(p.exists());
            assert_ne!(p, PathBuf::from("/nonexistent/chrome/path"));
        }
    }
}

#[cfg(test)]
mod advanced_patcher_tests {
    use super::*;

    #[test]
    fn patch_preset_round_trips_and_rejects_unknown() {
        for (raw, expected) in [
            ("balanced", PatchPreset::Balanced),
            ("ALL", PatchPreset::All),
            ("aggressive", PatchPreset::Aggressive),
            ("chrome-binary", PatchPreset::ChromeBinary),
            ("chrome_binary", PatchPreset::ChromeBinary),
        ] {
            let parsed = raw.parse::<PatchPreset>().unwrap();
            assert_eq!(parsed, expected);
            assert_eq!(parsed.to_patch(), EnginePatch::from_preset(expected));
        }
        assert!("unknown-preset".parse::<PatchPreset>().is_err());
        assert_eq!(PatchPreset::Aggressive.to_string(), "aggressive");
    }

    #[test]
    fn detects_executable_container_formats() {
        assert_eq!(BinaryFormat::detect(b"\x7fELF\x02\x01"), BinaryFormat::Elf);
        assert_eq!(BinaryFormat::detect(b"MZ\x90\x00"), BinaryFormat::Pe);
        assert_eq!(
            BinaryFormat::detect(&[0xcf, 0xfa, 0xed, 0xfe, 0x07]),
            BinaryFormat::MachO
        );
        assert_eq!(
            BinaryFormat::detect(&[0xca, 0xfe, 0xba, 0xbe, 0x00]),
            BinaryFormat::MachO
        );
        assert_eq!(BinaryFormat::detect(b"plain-script"), BinaryFormat::Unknown);
    }

    #[test]
    fn binary_patch_rule_enforces_non_empty_equal_length() {
        assert!(BinaryPatchRule::try_new(b"".as_slice(), b"".as_slice()).is_err());
        assert!(BinaryPatchRule::try_new(b"abc".as_slice(), b"ab".as_slice()).is_err());
        assert!(BinaryPatchRule::neutralize(b"".as_slice()).is_err());

        let rule = BinaryPatchRule::try_new(b"botFlag", b"usrFlag").unwrap();
        assert_eq!(rule.needle(), b"botFlag");
        assert_eq!(rule.replacement(), b"usrFlag");

        let spaces = BinaryPatchRule::neutralize(b"botMarker").unwrap();
        assert_eq!(spaces.replacement(), b"         ");
    }

    #[test]
    fn inspect_and_aggressive_patch_preserve_lengths_and_clear_all_markers() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("chromedriver");
        let payload = concat!(
            "\x7fELF",
            "window.cdc_abcdef1234567890abcdef_Array = window.Array;\n",
            "\"$cdc_abcdef1234567890abcdef_\";\n",
            "raw_cstr=$cdc_1234567890abcdefghijkl_end\0",
            "{window.cdc_foo();}\n",
            "__webdriver_evaluate __selenium_id __driver_fn __fxdriver_unwrapped ",
            "__nightmare _phantom callPhantom calledSelenium $wdc_cache __lastWatirAlert ",
            "domAutomation domAutomationController ",
            "ChromeDriver/157.0.0.1 EdgeDriver/157.0.0.2 HeadlessChrome/157.0 ",
            "navigator.webdriver = true; navigator.webdriver=!0; ",
            "kAutomationControlled AutomationControlled enable-automation webdriver-active ",
            "__puppeteer_evaluation_script__ __playwright_evaluation_script__ ",
            "//# sourceURL=pptr:evaluate.js\n",
            "customBotToken\n"
        )
        .as_bytes()
        .to_vec();
        let original_len = payload.len();
        fs::write(&bin, &payload).unwrap();

        let patcher = ChromedriverPatcher::new(&bin);
        let before = patcher.inspect().unwrap();
        assert_eq!(before.format, BinaryFormat::Elf);
        assert_eq!(before.size_bytes, original_len);
        assert!(before.needs_patch());
        assert!(before.has_any_markers());
        assert_eq!(before.counts.cdc_properties, 1);
        assert_eq!(before.counts.cdc_prefixes, 2);
        assert_eq!(before.counts.cdc_blocks, 1);
        assert_eq!(before.counts.webdriver_markers, 3);
        assert_eq!(before.counts.extra_driver_globals, 7);
        assert_eq!(before.counts.dom_automation, 2);
        assert_eq!(before.counts.chrome_driver_versions, 2);
        assert_eq!(before.counts.headless_user_agent, 1);
        assert_eq!(before.counts.navigator_webdriver, 2);
        assert_eq!(before.counts.automation_flags, 4);
        assert_eq!(before.counts.cdp_isolation_markers, 2);
        assert_eq!(before.counts.source_urls, 1);

        let spec = EnginePatch::aggressive()
            .custom_rule(BinaryPatchRule::try_new(b"customBotToken", b"customAppToken").unwrap());
        patcher.patch(spec).unwrap();

        let patched_bytes = fs::read(&bin).unwrap();
        assert_eq!(
            patched_bytes.len(),
            original_len,
            "patching must preserve exact binary byte length"
        );
        let patched_text = String::from_utf8_lossy(&patched_bytes);
        assert!(patched_text.contains("navigator.webdriver = !1"));
        assert!(patched_text.contains("navigator.webdriver=!1"));
        assert!(patched_text.contains("customAppToken"));
        assert!(!patched_text.contains("domAutomation"));
        assert!(!patched_text.contains("playwright_evaluation_script"));
        assert!(!patched_text.contains("enable-automation"));

        let after = patcher.inspect().unwrap();
        assert_ne!(after.sha256, before.sha256);
        assert!(!after.needs_patch(), "core markers still present: {after}");
        assert!(
            !after.has_any_markers(),
            "unexpected residual markers after aggressive patch: {after}"
        );
        assert!(patcher.verify_clean().unwrap());
    }

    #[test]
    fn chrome_binary_patcher_separates_cache_by_spec_and_patches_companions() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        let app_contents = dir.path().join("Google Chrome.app").join("Contents");
        let exe = app_contents.join("MacOS").join("Google Chrome");
        let framework = app_contents
            .join("Frameworks")
            .join("Google Chrome Framework.framework")
            .join("Versions")
            .join("Current")
            .join("Google Chrome Framework");
        fs::create_dir_all(exe.parent().unwrap()).unwrap();
        fs::create_dir_all(framework.parent().unwrap()).unwrap();
        fs::write(
            &exe,
            b"HeadlessChrome __webdriver __selenium __fxdriver __driver enable-automation",
        )
        .unwrap();
        fs::write(
            &framework,
            b"__playwright_evaluation_script__ domAutomationController //# sourceURL=webdriver\n",
        )
        .unwrap();

        let patcher = ChromeBinaryPatcher::new(&exe).cache_dir(&cache);
        assert_eq!(patcher.find_companion_binary(), Some(framework.clone()));
        assert_eq!(patcher.inspect_bundle().unwrap().len(), 2);

        let default_path = patcher.patched_path().unwrap();
        let agg = EnginePatch::aggressive();
        let agg_path = patcher.patched_path_for(&agg).unwrap();
        assert_ne!(
            default_path, agg_path,
            "distinct presets must not collide in patch cache"
        );

        let bundle = patcher.patch_bundle(agg).unwrap();
        assert_eq!(bundle.executable, agg_path);
        assert!(bundle.executable.exists());
        let comp = bundle.companion.expect("macOS framework companion found");
        assert!(comp.exists());
        assert!(!BinaryMarkerReport::inspect(&comp)
            .unwrap()
            .has_any_markers());
        // Source files remain untouched.
        assert!(BinaryMarkerReport::inspect(&framework)
            .unwrap()
            .has_any_markers());

        // Windows versioned chrome.dll sibling layout is also discovered.
        let win_dir = dir.path().join("win");
        let win_exe = win_dir.join("chrome.exe");
        let win_dll = win_dir.join("157.0.0.1").join("chrome.dll");
        fs::create_dir_all(win_dll.parent().unwrap()).unwrap();
        fs::write(&win_exe, b"MZ\x90\x00HeadlessChrome").unwrap();
        fs::write(&win_dll, b"MZ\x90\x00__playwright_evaluation_script__").unwrap();
        let win_patcher = ChromeBinaryPatcher::new(&win_exe).cache_dir(&cache);
        assert_eq!(win_patcher.find_companion_binary(), Some(win_dll));
    }
}
