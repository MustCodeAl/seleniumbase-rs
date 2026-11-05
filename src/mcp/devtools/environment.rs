//! Checking the machine with no browser: Chrome and chromedriver and whether
//! they fit, the driver cache, the output directory, the build and the `SB_*`
//! settings.

use std::time::Duration;

use serde_json::{json, Map, Value};

use super::SESSION_WAIT;
use crate::mcp::failure::shorten;
use crate::mcp::support::json_output;
use crate::mcp::{Args, Ctx, Output, ToolError, OUTPUT_DIR_VAR};

/// Whether `path` is a folder that can be written to, by its permissions.
fn is_writable_folder(path: &std::path::Path) -> bool {
    std::fs::metadata(path).is_ok_and(|meta| meta.is_dir() && !meta.permissions().readonly())
}

/// The nearest folder at or above `path` that exists: where a folder that is
/// not there yet would be made.
fn nearest_existing_folder(path: &std::path::Path) -> Option<std::path::PathBuf> {
    let absolute = std::path::absolute(path).ok()?;
    absolute
        .ancestors()
        .find(|folder| folder.is_dir())
        .map(std::path::Path::to_path_buf)
}

/// Whether the output directory exists, whether it is writable and how much
/// disk is free there; a problem is appended for each thing that will hurt.
///
/// A directory that is not there yet is made by the first tool that writes
/// into it, so then it is the nearest folder above it that has to be writable.
async fn check_output_dir(
    output_dir: &std::path::Path,
    problems: &mut Vec<String>,
) -> (bool, Option<bool>, Option<u64>) {
    let metadata = std::fs::metadata(output_dir).ok();
    let writable = if metadata.is_some() {
        Some(is_writable_folder(output_dir))
    } else {
        nearest_existing_folder(output_dir).map(|folder| is_writable_folder(&folder))
    };
    if writable == Some(false) {
        problems.push(if metadata.is_some() {
            format!(
                "The output directory {} is not a writable directory; set {OUTPUT_DIR_VAR} to one \
                 that is.",
                output_dir.display()
            )
        } else {
            format!(
                "The output directory {} does not exist yet, and the folder it would be made in \
                 is not writable; set {OUTPUT_DIR_VAR} to a folder that is.",
                output_dir.display()
            )
        });
    }
    let free = free_disk_bytes(output_dir).await;
    if free.is_some_and(|bytes| bytes < LOW_DISK) {
        problems.push(format!(
            "Less than 1 GiB is free where the output directory is ({}); screenshots, \
             downloads and the browser profile may fail to save.",
            output_dir.display()
        ));
    }
    (metadata.is_some(), writable, free)
}

pub(super) async fn diagnose_environment<S: Send + 'static>(
    ctx: Ctx<S>,
    _args: Args,
) -> Result<Output, ToolError> {
    use crate::browser::driver_version::binary_major;
    use crate::resources::assets::{lookup_chromedriver, DriverLookup};

    let mut problems: Vec<String> = Vec::new();
    let session = match tokio::time::timeout(SESSION_WAIT, ctx.try_session()).await {
        Ok(Some(_)) => "running",
        Ok(None) => "none",
        Err(_) => "busy: another call is using it",
    };

    let chrome = crate::evasion::patcher::find_system_chrome();
    let chrome_major = match &chrome {
        Some(path) => binary_major(path).await,
        None => None,
    };
    if chrome.is_none() {
        problems.push(
            "No Chrome or Chromium was found. Install Google Chrome, or set SB_CHROME_BIN to \
             its executable before starting the server (the Pure CDP server reads it)."
                .to_owned(),
        );
    }
    let driver = lookup_chromedriver();
    let driver_major = match driver.found() {
        Some(path) => binary_major(path).await,
        None => None,
    };
    match &driver {
        DriverLookup::MissingConfigured(path) => problems.push(format!(
            "CHROMEDRIVER_PATH names {}, which is not a file. Fix or unset it, or run `sbrs \
             install`.",
            path.display()
        )),
        DriverLookup::NotFound => problems.push(
            "No chromedriver was found. The WebDriver servers (sb, driver) need one: run `sbrs \
             install`. The Pure CDP server does not."
                .to_owned(),
        ),
        DriverLookup::Found(_) => {}
    }
    let (versions, version_problem) = version_check(driver_major, chrome_major);
    problems.extend(version_problem);

    let output_dir = ctx.settings().output_dir().to_path_buf();
    let (exists, writable, free) = check_output_dir(&output_dir, &mut problems).await;
    let variable =
        |name: &str| std::env::var_os(name).map(|value| value.to_string_lossy().into_owned());
    let settings = sb_settings(std::env::vars_os().map(|(name, value)| {
        (
            name.to_string_lossy().into_owned(),
            value.to_string_lossy().into_owned(),
        )
    }));

    json_output(json!({
        "seleniumbase_rs": env!("CARGO_PKG_VERSION"),
        "platform": {
            "os": std::env::consts::OS,
            "family": std::env::consts::FAMILY,
            "arch": std::env::consts::ARCH,
        },
        "browser_session": session,
        "chrome": chrome.as_ref().map_or(json!({ "found": false }), |path| json!({
            "path": path.display().to_string(),
            "major_version": chrome_major,
        })),
        "chromedriver": match &driver {
            DriverLookup::Found(path) => json!({
                "path": path.display().to_string(),
                "major_version": driver_major,
            }),
            DriverLookup::MissingConfigured(path) => json!({
                "found": false,
                "configured": path.display().to_string(),
            }),
            DriverLookup::NotFound => json!({ "found": false }),
        },
        "versions": versions,
        "driver_cache": driver_cache(std::path::Path::new(
            crate::browser::binaries::DOWNLOADS_DIR,
        )),
        "output_dir": {
            "path": output_dir.display().to_string(),
            "exists": exists,
            "writable": writable,
            "free_disk_bytes": free,
        },
        "build": build_info(),
        "chromedriver_path_variable": variable("CHROMEDRIVER_PATH"),
        "sb_settings": settings,
        "problems": problems,
    }))
}

/// Below this much free space, saving evidence is likely to fail.
const LOW_DISK: u64 = 1024 * 1024 * 1024;

/// The cargo features that change what this library can do, with whether this
/// build has each. A feature that is off makes the calls that need it fail
/// with an error that names it.
const FEATURES: [(&str, bool); 24] = [
    ("mcp-server", cfg!(feature = "mcp-server")),
    ("test-util", cfg!(feature = "test-util")),
    ("plugins", cfg!(feature = "plugins")),
    ("plugin-shield", cfg!(feature = "plugin-shield")),
    (
        "plugin-block-resources",
        cfg!(feature = "plugin-block-resources"),
    ),
    ("plugin-crx", cfg!(feature = "plugin-crx")),
    ("plugin-fetcher", cfg!(feature = "plugin-fetcher")),
    ("plugin-extractor", cfg!(feature = "plugin-extractor")),
    ("plugin-batcher", cfg!(feature = "plugin-batcher")),
    ("plugin-crawler", cfg!(feature = "plugin-crawler")),
    ("plugin-browser", cfg!(feature = "plugin-browser")),
    ("plugin-imap", cfg!(feature = "plugin-imap")),
    ("plugin-tor", cfg!(feature = "plugin-tor")),
    ("plugins-builtin", cfg!(feature = "plugins-builtin")),
    ("pdf", cfg!(feature = "pdf")),
    ("visual", cfg!(feature = "visual")),
    ("ocr", cfg!(feature = "ocr")),
    ("turso", cfg!(feature = "turso")),
    ("tui", cfg!(feature = "tui")),
    ("gui", cfg!(feature = "gui")),
    ("s3", cfg!(feature = "s3")),
    ("azure", cfg!(feature = "azure")),
    ("gcp", cfg!(feature = "gcp")),
    ("error-backtrace", cfg!(feature = "error-backtrace")),
];

/// What this build is: the library version and the features it has and lacks.
fn build_info() -> Value {
    let names = |wanted: bool| -> Vec<&str> {
        FEATURES
            .iter()
            .filter(|(_, on)| *on == wanted)
            .map(|(name, _)| *name)
            .collect()
    };
    json!({
        "version": env!("CARGO_PKG_VERSION"),
        "features": names(true),
        "features_off": names(false),
        "note": "A call that needs a feature this build lacks fails with an error that names it; \
                 rebuild with --features NAME to get it.",
    })
}

/// The folder the drivers `sbrs install` fetches go to: where it is, whether
/// it is there, and what is in it.
fn driver_cache(dir: &std::path::Path) -> Value {
    let absolute = std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf());
    let entries = std::fs::read_dir(dir).ok().map(|entries| {
        let mut names: Vec<String> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names.truncate(50);
        names
    });
    json!({
        "path": absolute.display().to_string(),
        "exists": entries.is_some(),
        "entries": entries.unwrap_or_default(),
        "note": "Relative to the folder the server runs in. The library looks here first for a \
                 chromedriver, geckodriver or msedgedriver; install_driver and `sbrs install` \
                 fill it.",
    })
}

/// Words in a setting's name that mark its value as secret.
const SECRET_NAMES: [&str; 7] = [
    "PASS",
    "TOKEN",
    "SECRET",
    "KEY",
    "AUTH",
    "CREDENTIAL",
    "COOKIE",
];

/// Whether chromedriver and Chrome fit, as `match`, `mismatch` or `unknown`,
/// and for a mismatch the problem with the command that fixes it.
fn version_check(driver: Option<u32>, browser: Option<u32>) -> (Value, Option<String>) {
    use crate::browser::driver_version::{fit, Fit};

    let (verdict, problem) = match fit(driver, browser) {
        Fit::Matches => ("match", None),
        Fit::Mismatch { driver, browser } => (
            "mismatch",
            Some(format!(
                "chromedriver {driver} does not match Chrome {browser}; a driver only drives its \
                 own browser version. Run `sbrs install` to fetch chromedriver {browser}, then \
                 `sbrs doctor` to check both."
            )),
        ),
        Fit::Unknown => ("unknown", None),
    };
    (
        json!({ "fit": verdict, "chromedriver": driver, "chrome": browser }),
        problem,
    )
}

/// The `SB_*` settings among `vars`, by name, with secret-looking values
/// redacted (see [`redact_setting`]).
fn sb_settings(vars: impl IntoIterator<Item = (String, String)>) -> Map<String, Value> {
    let mut found: Vec<(String, String)> = vars
        .into_iter()
        .filter(|(name, _)| name.starts_with("SB_"))
        .collect();
    found.sort();
    found
        .into_iter()
        .map(|(name, value)| {
            let shown = shorten(&Value::from(redact_setting(&name, &value)), 300);
            (name, shown)
        })
        .collect()
}

/// `value` of the setting `name`, safe to show: the whole value when the
/// name says it is secret, and the user and password of a URL or proxy.
pub(in crate::mcp) fn redact_setting(name: &str, value: &str) -> String {
    let upper = name.to_ascii_uppercase();
    if SECRET_NAMES.iter().any(|word| upper.contains(word)) {
        return "[redacted]".to_owned();
    }
    // `scheme://user:password@host` or `user:password@host`.
    let start = value.find("://").map_or(0, |at| at + 3);
    match value[start..].rfind('@') {
        Some(at) if at > 0 => format!("{}[redacted]{}", &value[..start], &value[start + at..]),
        _ => value.to_owned(),
    }
}

/// Free bytes on the file system that holds `dir` (or its nearest existing
/// parent), from `df -P -k`; `None` where that cannot be asked.
async fn free_disk_bytes(dir: &std::path::Path) -> Option<u64> {
    let existing = dir.ancestors().find(|path| path.exists())?;
    let output = tokio::time::timeout(
        Duration::from_secs(5),
        tokio::process::Command::new("df")
            .args(["-P", "-k"])
            .arg(existing)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .ok()?
    .ok()?;
    parse_df(&String::from_utf8_lossy(&output.stdout))
}

/// The available space in the output of `df -P -k`, in bytes.
fn parse_df(output: &str) -> Option<u64> {
    let row = output.lines().nth(1)?;
    let kilobytes: u64 = row.split_whitespace().nth(3)?.parse().ok()?;
    kilobytes.checked_mul(1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_environment_check_names_the_fix_for_a_version_mismatch() {
        let (fits, problem) = version_check(Some(155), Some(155));
        assert_eq!(fits["fit"], "match");
        assert_eq!(problem, None);

        let (fits, problem) = version_check(Some(154), Some(155));
        assert_eq!(fits["fit"], "mismatch");
        assert_eq!(
            (fits["chromedriver"].clone(), fits["chrome"].clone()),
            (json!(154), json!(155))
        );
        let problem = problem.unwrap();
        assert!(
            problem.contains("`sbrs install`") && problem.contains("`sbrs doctor`"),
            "{problem}"
        );

        let (fits, problem) = version_check(None, Some(155));
        assert_eq!(fits["fit"], "unknown");
        assert_eq!(problem, None);
    }

    #[test]
    fn the_environment_check_hides_secrets_in_settings() {
        assert_eq!(
            redact_setting("SB_PROXY", "user:pw@proxy.test:8080"),
            "[redacted]@proxy.test:8080"
        );
        assert_eq!(
            redact_setting("SB_PROXY", "http://user:pw@proxy.test:8080"),
            "http://[redacted]@proxy.test:8080"
        );
        assert_eq!(
            redact_setting("SB_PROXY", "proxy.test:8080"),
            "proxy.test:8080"
        );
        assert_eq!(redact_setting("SB_API_TOKEN", "abc123"), "[redacted]");
        assert_eq!(redact_setting("SB_TOTP_KEY", "JBSWY3DP"), "[redacted]");
        assert_eq!(redact_setting("SB_BROWSER", "chrome"), "chrome");

        let settings = sb_settings([
            ("SB_HEADLESS".to_owned(), "true".to_owned()),
            ("PATH".to_owned(), "/usr/bin".to_owned()),
            ("SB_PROXY".to_owned(), "u:p@h:1".to_owned()),
            ("SB_BROWSER".to_owned(), "chrome".to_owned()),
        ]);
        let names: Vec<&String> = settings.keys().collect();
        assert_eq!(names, ["SB_BROWSER", "SB_HEADLESS", "SB_PROXY"]);
        assert_eq!(settings["SB_PROXY"], "[redacted]@h:1");
    }

    #[tokio::test]
    async fn a_missing_output_folder_is_fine_when_the_folder_it_would_be_made_in_is_writable() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("out").join("deeper");
        let mut problems = Vec::new();

        let (exists, writable, _) = check_output_dir(&missing, &mut problems).await;

        assert!(!exists);
        assert_eq!(writable, Some(true));
        assert!(problems.is_empty(), "{problems:?}");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_missing_output_folder_under_a_folder_that_cannot_be_written_is_a_problem() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o555)).unwrap();
        let mut problems = Vec::new();

        let (exists, writable, _) = check_output_dir(&dir.path().join("out"), &mut problems).await;

        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(!exists);
        assert_eq!(writable, Some(false));
        assert!(
            problems[0].contains("does not exist yet") && problems[0].contains(OUTPUT_DIR_VAR),
            "{problems:?}"
        );
    }

    #[tokio::test]
    async fn an_existing_output_folder_that_is_a_file_is_not_writable() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("not-a-folder");
        std::fs::write(&file, "x").unwrap();
        let mut problems = Vec::new();

        let (exists, writable, _) = check_output_dir(&file, &mut problems).await;

        assert!(exists);
        assert_eq!(writable, Some(false));
        assert!(
            problems[0].contains("not a writable directory"),
            "{problems:?}"
        );
    }

    #[test]
    fn the_driver_cache_lists_what_is_in_it_and_says_when_it_is_not_there() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("downloaded_drivers");

        let missing = driver_cache(&cache);
        assert_eq!(missing["exists"], false);
        assert_eq!(missing["entries"], json!([]));
        assert!(missing["path"]
            .as_str()
            .unwrap()
            .ends_with("downloaded_drivers"));

        std::fs::create_dir(&cache).unwrap();
        std::fs::write(cache.join("geckodriver"), "").unwrap();
        std::fs::write(cache.join("chromedriver"), "").unwrap();
        let present = driver_cache(&cache);
        assert_eq!(present["exists"], true);
        assert_eq!(present["entries"], json!(["chromedriver", "geckodriver"]));
    }

    #[test]
    fn the_build_says_which_features_it_has_and_lacks() {
        let build = build_info();
        let on: Vec<&str> = build["features"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        let off: Vec<&str> = build["features_off"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();

        // This code is compiled only with the MCP server, and every feature is one or the other.
        assert!(on.contains(&"mcp-server"));
        assert_eq!(on.len() + off.len(), FEATURES.len());
        assert_eq!(on.contains(&"pdf"), cfg!(feature = "pdf"));
        assert_eq!(
            on.contains(&"plugins-builtin"),
            cfg!(feature = "plugins-builtin")
        );
        assert_eq!(build["version"], env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn the_environment_check_reads_free_space_from_df() {
        let macos = "Filesystem   1024-blocks      Used Available Capacity  Mounted on\n\
                     /dev/disk3s5   482797652 411111111  33333333    93%    /System/Volumes/Data\n";
        let linux = "Filesystem     1024-blocks    Used Available Capacity Mounted on\n\
                     /dev/sda1         41152736 9120000  30000000      24% /\n";
        assert_eq!(parse_df(macos), Some(33_333_333 * 1024));
        assert_eq!(parse_df(linux), Some(30_000_000 * 1024));
        assert_eq!(parse_df("df: /nowhere: No such file or directory\n"), None);
        assert_eq!(parse_df(""), None);
    }
}
