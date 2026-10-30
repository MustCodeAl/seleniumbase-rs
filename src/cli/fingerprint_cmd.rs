//! `sbrs fingerprint`: capture, export, check and diff fingerprint files.
//!
//! The command-line crate declares the arguments and starts the browser for
//! `capture`; this module holds what needs no parser and, apart from the summary of a
//! capture, no browser: the presets, the identities `export` writes, and the text of
//! `check`, `diff` and `presets`. Every file it reads or writes is the one file
//! format, the v2 [`Fingerprint`] JSON.

use std::fmt::Write as _;
use std::path::Path;

use crate::error::SeleniumBaseError;
use crate::evasion::capture::CapturedFingerprint;
use crate::evasion::fingerprint::{Fingerprint, OsType};
use crate::evasion::humanize::Rng;

/// The names `export --preset` takes, in the order `presets` lists them. `safari` is
/// also read, as the name of the `ios` preset.
pub const PRESET_NAMES: [&str; 6] = ["windows", "edge", "macos", "linux", "android", "ios"];

/// The preset called `name`, if there is one. The name is read in any case.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::cli::fingerprint_cmd::preset;
///
/// assert!(preset("Windows").is_some());
/// assert_eq!(preset("safari"), preset("ios"));
/// assert!(preset("beos").is_none());
/// ```
#[must_use]
pub fn preset(name: &str) -> Option<Fingerprint> {
    Some(match name.to_ascii_lowercase().as_str() {
        "windows" => Fingerprint::windows_desktop(),
        "edge" => Fingerprint::edge_desktop(),
        "macos" => Fingerprint::macos_desktop(),
        "linux" => Fingerprint::linux_desktop(),
        "android" => Fingerprint::android_mobile(),
        "ios" | "safari" => Fingerprint::ios_mobile_safari(),
        _ => return None,
    })
}

/// The operating system this program runs on, as far as a persona goes: macOS and
/// Windows, and Linux for everything else.
#[must_use]
pub const fn host_os() -> OsType {
    if cfg!(target_os = "macos") {
        OsType::Macos
    } else if cfg!(windows) {
        OsType::Windows
    } else {
        OsType::Linux
    }
}

/// Where `export` takes its fingerprint from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportSource {
    /// A preset, by name (see [`PRESET_NAMES`]).
    Preset(String),
    /// A random identity: the operating system it claims (this machine's when `None`)
    /// and the seed that makes it repeatable (a fresh one when `None`, which the
    /// fingerprint keeps).
    Random {
        /// The operating system the identity claims.
        os: Option<OsType>,
        /// The seed of the draw.
        seed: Option<u64>,
    },
}

/// The fingerprint `export` writes.
///
/// # Errors
///
/// [`SeleniumBaseError::InvalidConfig`] naming the known presets when a preset name is
/// not one.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::cli::fingerprint_cmd::{export, ExportSource};
/// use seleniumbase_rs::OsType;
///
/// let drawn = ExportSource::Random { os: Some(OsType::Macos), seed: Some(7) };
/// assert_eq!(export(&drawn)?, export(&drawn)?, "a seed repeats");
/// assert!(export(&ExportSource::Preset("windows".to_owned()))?.platform().is_value());
/// assert!(export(&ExportSource::Preset("beos".to_owned())).is_err());
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
pub fn export(source: &ExportSource) -> Result<Fingerprint, SeleniumBaseError> {
    match source {
        ExportSource::Preset(name) => preset(name).ok_or_else(|| {
            SeleniumBaseError::invalid_config(format!(
                "there is no preset called {name:?}; the presets are {}",
                PRESET_NAMES.join(", ")
            ))
        }),
        ExportSource::Random { os, seed } => {
            let seed = seed.unwrap_or_else(|| Rng::from_entropy().next_u64());
            Ok(Fingerprint::randomized(os.unwrap_or(host_os()), seed))
        }
    }
}

/// What a browser, by its user agent, is called: `Edge 156`, `Chrome 155`, `Firefox 130`,
/// `Safari 26.0`, or the user agent itself when it is none of them.
fn browser_label(user_agent: &str) -> String {
    let token = |name: &str| {
        let rest = user_agent.split(name).nth(1)?;
        let version = rest
            .split(|c: char| c.is_whitespace() || c == ';' || c == ')')
            .next()?;
        Some(version.split('.').next().unwrap_or(version).to_owned())
    };
    if let Some(version) = token("Edg/") {
        format!("Edge {version}")
    } else if let Some(version) = token("Firefox/") {
        format!("Firefox {version}")
    } else if let Some(version) = token("Chrome/") {
        format!("Chrome {version}")
    } else if let Some(version) = token("Version/") {
        format!("Safari {version}")
    } else {
        user_agent.to_owned()
    }
}

/// A line that says what a fingerprint claims: its browser, system, screen and time zone.
fn describe(fingerprint: &Fingerprint) -> String {
    let mut parts = Vec::new();
    if let Some(user_agent) = fingerprint.user_agent().as_str() {
        parts.push(browser_label(user_agent));
    }
    parts.push(format!("{:?}", fingerprint.os_type()));
    if let Some(screen) = fingerprint.screen().value() {
        parts.push(format!("{}x{}", screen.width, screen.height));
    }
    if let Some(zone) = fingerprint.timezone().as_str() {
        parts.push(zone.to_owned());
    }
    parts.join(", ")
}

/// The table `sbrs fingerprint presets` prints: each name and what the preset claims.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::cli::fingerprint_cmd::presets_table;
///
/// let table = presets_table();
/// assert!(table.lines().any(|line| line.starts_with("windows") && line.contains("1920x1080")));
/// ```
#[must_use]
pub fn presets_table() -> String {
    let mut table = String::new();
    for name in PRESET_NAMES {
        if let Some(fingerprint) = preset(name) {
            // Writing to a `String` cannot fail.
            let _ = writeln!(table, "{name:<8} {}", describe(&fingerprint));
        }
    }
    table
}

/// What `check` found in a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    /// The report to print: how many settings the file gives, each error and warning,
    /// and the verdict.
    pub text: String,
    /// Whether the fingerprint has no error. A warning does not make it incoherent.
    pub is_coherent: bool,
}

/// Loads `path` and reports its [`validate`](Fingerprint::validate): the settings it gives,
/// the errors, the warnings and the verdict.
///
/// # Errors
///
/// As [`Fingerprint::load`]: the file cannot be read, is too large, or is not a
/// fingerprint.
pub fn check(path: &Path) -> Result<Check, SeleniumBaseError> {
    let fingerprint = Fingerprint::load(path)?;
    Ok(check_report(&path.display().to_string(), &fingerprint))
}

/// The report of [`check`] for a fingerprint that is loaded, under the name `name`.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::cli::fingerprint_cmd::check_report;
/// use seleniumbase_rs::Fingerprint;
///
/// let report = check_report("windows.json", &Fingerprint::windows_desktop());
/// assert!(report.is_coherent);
/// assert!(report.text.starts_with("windows.json: "));
/// assert!(report.text.trim_end().ends_with("coherent"));
/// ```
#[must_use]
pub fn check_report(name: &str, fingerprint: &Fingerprint) -> Check {
    let report = fingerprint.validate();
    let given = fingerprint.claimed_settings().len();
    let mut text = format!(
        "{name}: {given} settings given ({})\n",
        describe(fingerprint)
    );
    for error in &report.errors {
        let _ = writeln!(text, "error: {error}");
    }
    for warning in &report.warnings {
        let _ = writeln!(text, "warning: {warning}");
    }
    let verdict = match (report.errors.len(), report.warnings.len()) {
        (0, 0) => "coherent".to_owned(),
        (0, warnings) => format!("coherent, with {warnings} warning(s)"),
        (errors, _) => format!("not coherent: {errors} error(s)"),
    };
    text.push_str(&verdict);
    text.push('\n');
    Check {
        text,
        is_coherent: report.is_coherent(),
    }
}

/// What `diff` found between two files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Difference {
    /// The report to print: a line for each setting that differs, then the count.
    pub text: String,
    /// Whether any setting differs.
    pub differs: bool,
}

/// Loads two files and compares them with [`Fingerprint::diff`]: `expected` is what is
/// expected and `found` what is found, so a setting `expected` leaves natural is not
/// compared.
///
/// # Errors
///
/// As [`Fingerprint::load`], for either file.
pub fn diff(expected: &Path, found: &Path) -> Result<Difference, SeleniumBaseError> {
    Ok(diff_report(
        &expected.display().to_string(),
        &found.display().to_string(),
        &Fingerprint::load(expected)?,
        &Fingerprint::load(found)?,
    ))
}

/// The report of [`diff`] for two fingerprints that are loaded, under their names.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::cli::fingerprint_cmd::diff_report;
/// use seleniumbase_rs::Fingerprint;
///
/// let windows = Fingerprint::windows_desktop();
/// let tokyo = windows.to_builder().timezone("Asia/Tokyo").build()?;
///
/// let same = diff_report("a.json", "b.json", &windows, &windows);
/// assert!(!same.differs);
///
/// let other = diff_report("a.json", "b.json", &tokyo, &windows);
/// assert!(other.differs);
/// assert!(other.text.contains(r#"timezone: expected "Asia/Tokyo", found "America/New_York""#));
/// # Ok::<(), seleniumbase_rs::FingerprintError>(())
/// ```
#[must_use]
pub fn diff_report(
    expected_name: &str,
    found_name: &str,
    expected: &Fingerprint,
    found: &Fingerprint,
) -> Difference {
    let differences = expected.diff(found);
    if differences.is_empty() {
        let compared = expected.claimed_settings().len();
        return Difference {
            text: format!(
                "no difference: {found_name} agrees with the {compared} settings {expected_name} gives\n"
            ),
            differs: false,
        };
    }
    let mut text = String::new();
    for difference in &differences {
        let _ = writeln!(text, "{difference}");
    }
    let _ = writeln!(
        text,
        "{} setting(s) differ: {expected_name} is what is expected, {found_name} what is found",
        differences.len()
    );
    Difference {
        text,
        differs: true,
    }
}

/// The lines `capture` prints to standard error: what was captured and where, the settings
/// the page could not answer, the ones no page can read, and a warning when the browser
/// showed the markers of an automated one.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::cli::fingerprint_cmd::capture_summary;
/// use seleniumbase_rs::evasion::capture::CapturedFingerprint;
/// use serde_json::json;
///
/// let readings = json!({
///     "version": 1,
///     "url": "about:blank",
///     "navigator": { "userAgent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36" },
///     "headless": ["navigator_webdriver_true"],
/// });
/// let captured = CapturedFingerprint::from_probe(&readings, "2026-10-18T12:00:00Z")?;
///
/// let summary = capture_summary(&captured);
/// assert!(summary.starts_with("Captured Chrome 155.0.0.0 from about:blank"));
/// assert!(summary.contains("navigator_webdriver_true"));
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
#[must_use]
pub fn capture_summary(captured: &CapturedFingerprint) -> String {
    let fingerprint = &captured.fingerprint;
    let browser = captured
        .probe
        .browser_version
        .as_deref()
        .map(|version| {
            let name = fingerprint
                .user_agent()
                .as_str()
                .map_or_else(String::new, browser_label);
            let name = name.split(' ').next().unwrap_or_default().to_owned();
            if name.is_empty() {
                version.to_owned()
            } else {
                format!("{name} {version}")
            }
        })
        .unwrap_or_else(|| "a browser".to_owned());
    let mut text = format!(
        "Captured {browser} from {} at {}: {} settings read\n",
        captured.probe.url,
        captured.probe.captured_at,
        fingerprint.claimed_settings().len()
    );
    let unanswered = captured.unanswered();
    if !unanswered.is_empty() {
        let list: Vec<String> = unanswered
            .iter()
            .map(|(setting, reason)| format!("{setting} ({reason})"))
            .collect();
        let _ = writeln!(text, "  not answered by this page: {}", list.join(", "));
    }
    text.push_str("  not readable by any page: geolocation, fonts, proxy, webrtc\n");
    let markers: Vec<&str> = captured
        .unmapped
        .get("headless_markers")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .collect();
    if !markers.is_empty() {
        let _ = writeln!(
            text,
            "  warning: this browser shows the markers of an automated one ({}); capture a \
             browser a person uses for a fingerprint to present",
            markers.join(", ")
        );
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evasion::fingerprint::BrowserType;
    use serde_json::json;

    fn written(fingerprint: &Fingerprint) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fp.json");
        fingerprint.save(&path).unwrap();
        (dir, path)
    }

    #[test]
    fn every_listed_preset_exists_and_is_coherent() {
        for name in PRESET_NAMES {
            let fingerprint = preset(name).unwrap_or_else(|| panic!("{name} is not a preset"));

            assert!(fingerprint.validate().is_coherent(), "{name}");
        }
        assert_eq!(preset("SAFARI"), preset("ios"));
        assert!(preset("").is_none() && preset("beos").is_none());
    }

    #[test]
    fn the_presets_are_the_ones_the_macro_names() {
        assert_eq!(preset("windows"), Some(Fingerprint::windows_desktop()));
        assert_eq!(preset("edge"), Some(Fingerprint::edge_desktop()));
        assert_eq!(preset("macos"), Some(Fingerprint::macos_desktop()));
        assert_eq!(preset("linux"), Some(Fingerprint::linux_desktop()));
        assert_eq!(preset("android"), Some(Fingerprint::android_mobile()));
        assert_eq!(preset("ios"), Some(Fingerprint::ios_mobile_safari()));
    }

    #[test]
    fn an_unknown_preset_names_the_known_ones() {
        let error = export(&ExportSource::Preset("beos".to_owned())).unwrap_err();

        assert_eq!(error.exit_code(), 2);
        let message = error.to_string();
        assert!(message.contains("beos"), "{message}");
        for name in PRESET_NAMES {
            assert!(message.contains(name), "{message}");
        }
    }

    #[test]
    fn a_random_identity_follows_its_seed_and_its_system() {
        let seeded = |os, seed| ExportSource::Random { os, seed };

        let first = export(&seeded(Some(OsType::Linux), Some(11))).unwrap();
        let again = export(&seeded(Some(OsType::Linux), Some(11))).unwrap();
        let other = export(&seeded(Some(OsType::Linux), Some(12))).unwrap();
        let mac = export(&seeded(Some(OsType::Macos), Some(11))).unwrap();

        assert_eq!(first, again);
        assert_ne!(first, other);
        assert_eq!(first.os_type(), OsType::Linux);
        assert_eq!(mac.os_type(), OsType::Macos);
        assert_eq!(first.seed(), 11);
        assert!(first.validate().is_coherent());
    }

    #[test]
    fn a_random_identity_without_a_seed_is_new_every_time_and_keeps_its_seed() {
        let source = ExportSource::Random {
            os: None,
            seed: None,
        };

        let first = export(&source).unwrap();
        let second = export(&source).unwrap();

        assert_ne!(first, second);
        assert_eq!(
            first.os_type(),
            host_os(),
            "this machine's system by default"
        );
        // The seed is in the file, so the same file is the same identity.
        let reread = Fingerprint::from_json(&first.to_json_pretty().unwrap()).unwrap();
        assert_eq!(reread, first);
    }

    #[test]
    fn the_table_lists_every_preset_with_what_it_claims() {
        let table = presets_table();

        assert_eq!(table.lines().count(), PRESET_NAMES.len());
        let windows = table.lines().find(|l| l.starts_with("windows")).unwrap();
        assert!(
            windows.contains("Chrome") && windows.contains("1920x1080"),
            "{windows}"
        );
        assert!(windows.contains("America/New_York"), "{windows}");
        let edge = table.lines().find(|l| l.starts_with("edge")).unwrap();
        assert!(edge.contains("Edge "), "{edge}");
        let ios = table.lines().find(|l| l.starts_with("ios")).unwrap();
        assert!(ios.contains("Safari") && ios.contains("390x844"), "{ios}");
    }

    #[test]
    fn a_coherent_file_checks_out_with_its_count() {
        let (_dir, path) = written(&Fingerprint::macos_desktop());

        let check = check(&path).unwrap();

        assert!(check.is_coherent);
        assert!(check.text.contains(" settings given ("), "{}", check.text);
        assert!(
            check.text.trim_end().ends_with("coherent"),
            "{}",
            check.text
        );
        assert!(!check.text.contains("error:") && !check.text.contains("warning:"));
    }

    #[test]
    fn an_incoherent_file_lists_its_errors_and_is_not_coherent() {
        let wrong = Fingerprint::windows_desktop()
            .to_builder()
            .platform("MacIntel")
            .build()
            .unwrap();
        let (_dir, path) = written(&wrong);

        let check = check(&path).unwrap();

        assert!(!check.is_coherent);
        assert!(
            check.text.contains("error: navigator.platform 'MacIntel'"),
            "{}",
            check.text
        );
        assert!(
            check.text.trim_end().ends_with("not coherent: 1 error(s)"),
            "{}",
            check.text
        );
    }

    #[test]
    fn a_warning_is_reported_and_does_not_make_a_file_incoherent() {
        let odd = Fingerprint::windows_desktop()
            .to_builder()
            .locale("de-DE")
            .build()
            .unwrap();

        let check = check_report("odd.json", &odd);

        assert!(check.is_coherent, "{}", check.text);
        assert!(check.text.contains("warning: "), "{}", check.text);
        assert!(
            check
                .text
                .trim_end()
                .ends_with("coherent, with 2 warning(s)"),
            "{}",
            check.text
        );
    }

    #[test]
    fn a_file_that_does_not_load_is_an_error_not_a_report() {
        let dir = tempfile::tempdir().unwrap();
        let missing = check(&dir.path().join("missing.json")).unwrap_err();
        std::fs::write(dir.path().join("bad.json"), "{ \"timezone\": 5 }").unwrap();
        let bad = check(&dir.path().join("bad.json")).unwrap_err();

        assert_eq!(missing.exit_code(), 6);
        assert_eq!(bad.exit_code(), 2);
    }

    #[test]
    fn equal_files_do_not_differ_and_the_report_says_how_much_was_compared() {
        let windows = Fingerprint::windows_desktop();
        let (_a, a) = written(&windows);
        let (_b, b) = written(&windows);

        let difference = diff(&a, &b).unwrap();

        assert!(!difference.differs);
        assert!(
            difference.text.starts_with("no difference: "),
            "{}",
            difference.text
        );
        assert!(
            difference
                .text
                .contains(&windows.claimed_settings().len().to_string()),
            "{}",
            difference.text
        );
    }

    #[test]
    fn files_that_differ_say_which_setting_and_how_and_the_direction() {
        let windows = Fingerprint::windows_desktop();
        let mac = Fingerprint::macos_desktop();

        let difference = diff_report("preset.json", "real.json", &windows, &mac);

        assert!(difference.differs);
        let lines: Vec<&str> = difference.text.lines().collect();
        assert!(lines
            .iter()
            .any(|l| l.starts_with("platform: expected \"Win32\", found \"MacIntel\"")));
        assert!(lines.iter().any(|l| l.starts_with("timezone: ")));
        let last = lines.last().unwrap();
        assert!(
            last.contains("preset.json is what is expected, real.json what is found"),
            "{last}"
        );
        assert!(
            last.starts_with(&format!("{} setting(s) differ", lines.len() - 1)),
            "{last}"
        );
    }

    #[test]
    fn what_the_expected_side_leaves_natural_is_never_a_difference() {
        let bare = Fingerprint::builder()
            .timezone("America/New_York")
            .build()
            .unwrap();
        let windows = Fingerprint::windows_desktop();

        assert!(!diff_report("a", "b", &bare, &windows).differs);
        assert!(diff_report("a", "b", &windows, &bare).differs);
    }

    #[test]
    fn the_summary_names_the_browser_the_page_and_the_count_and_warns_about_automation() {
        let raw = json!({
            "version": 1,
            "url": "https://example.test/",
            "secureContext": false,
            "navigator": {
                "userAgent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/155.0.0.0 Safari/537.36",
                "platform": "MacIntel"
            },
            "userAgentData": { "unknown": "the API needs a secure context" },
            "headless": ["user_agent_says_headless", "navigator_webdriver_true"],
        });
        let captured = CapturedFingerprint::from_probe(&raw, "2026-10-18T12:00:00Z").unwrap();

        let summary = capture_summary(&captured);

        assert!(
            summary.starts_with(
                "Captured Chrome 155.0.0.0 from https://example.test/ at 2026-10-18T12:00:00Z: "
            ),
            "{summary}"
        );
        assert!(
            summary.contains("client_hints (the API needs a secure context)"),
            "{summary}"
        );
        // A reading the script left out altogether is not answered either, and says so.
        assert!(
            summary.contains("battery (the capture script did not answer)"),
            "{summary}"
        );
        assert!(summary.contains("not readable by any page: geolocation, fonts, proxy, webrtc"));
        assert!(summary.contains("warning: this browser shows the markers of an automated one"));
        assert!(summary.contains("user_agent_says_headless, navigator_webdriver_true"));
        assert_eq!(captured.fingerprint.browser_type(), BrowserType::Chromium);
    }

    #[test]
    fn a_browser_is_labelled_by_its_user_agent() {
        for (user_agent, label) in [
            ("Mozilla/5.0 (Windows NT 10.0) AppleWebKit/537.36 Chrome/156.0.0.0 Safari/537.36 Edg/156.0.0.0", "Edge 156"),
            ("Mozilla/5.0 (X11; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0", "Firefox 130"),
            ("Mozilla/5.0 (Macintosh) AppleWebKit/537.36 Chrome/155.0.0.0 Safari/537.36", "Chrome 155"),
            ("Mozilla/5.0 (iPhone) AppleWebKit/605.1.15 Version/26.0 Mobile/15E148 Safari/604.1", "Safari 26"),
            ("curl/8.0", "curl/8.0"),
        ] {
            assert_eq!(browser_label(user_agent), label);
        }
    }
}
