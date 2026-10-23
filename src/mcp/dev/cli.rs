//! `list_plugins`, `plugin_info` and `install_driver`: the plugins and the driver
//! downloader of the `sbrs` command line, from inside an MCP session.
//!
//! `cli_help`, which explains the command line itself, is in [`help`](super::help).

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tokio::sync::Mutex;

use super::NoBrowser;
use crate::cli::plugin_cmd;
use crate::cli::scripts::sb_install::{self, DriverName};
use crate::error::SeleniumBaseError;
use crate::mcp::support::{json_output, nonempty};
use crate::mcp::{Args, Ctx, Effect, Output, Prop, Schema, ToolDef, ToolError};

/// How long `<driver> --version` may take.
const VERSION_TIMEOUT: Duration = Duration::from_secs(10);

/// The future of a driver download.
pub type InstallFuture = Pin<Box<dyn Future<Output = Result<PathBuf, SeleniumBaseError>> + Send>>;

/// Downloads a driver and returns where its executable is.
///
/// [`host`](super::host) uses the downloader of `sbrs install`. A test gives
/// [`host_with_installer`](super::host_with_installer) one that reaches no
/// network.
pub type Installer = Arc<dyn Fn(DriverName, Option<String>) -> InstallFuture + Send + Sync>;

/// The downloader `sbrs install` uses.
pub(super) fn sbrs_installer() -> Installer {
    Arc::new(|driver, version| {
        Box::pin(async move { sb_install::install_driver(driver, version.as_deref()).await })
    })
}

/// The tools of this file.
pub(super) fn tools(installer: Installer) -> Vec<ToolDef<NoBrowser>> {
    vec![
        ToolDef::new(
            "list_plugins",
            "List Plugins",
            "List the built-in browser plugins compiled into this build, with their category, \
             whether the operator turned them on, and a summary. A build without them says so \
             and names the cargo feature that adds them (plugins-builtin).",
            Effect::Inspect,
            Schema::new(),
            list_plugins,
        ),
        ToolDef::new(
            "plugin_info",
            "Plugin Info",
            "Show what one built-in plugin declares about itself: version, category, the \
             capabilities it asks for and how risky they are, what it needs from the browser, \
             what it conflicts with, and its options. Does not turn the plugin on.",
            Effect::Inspect,
            Schema::new().required("name", Prop::string("The plugin's name, from list_plugins")),
            plugin_info,
        ),
        ToolDef::new(
            "install_driver",
            "Install Driver",
            "Download a browser driver, as `sbrs install` does, into the downloaded_drivers \
             folder of the folder the server runs in (the folder the library looks in first). \
             Without a version the driver matches the installed browser, or is the latest stable \
             one. Replaces a driver of another version already there. Needs network access. \
             Returns the path and the version that was installed.",
            Effect::Fetch,
            Schema::new()
                .optional(
                    "browser",
                    Prop::choice(
                        &["chrome", "edge", "firefox"],
                        "Which browser the driver is for: chromedriver, msedgedriver or geckodriver",
                    )
                    .default("chrome"),
                )
                .optional(
                    "version",
                    Prop::string(
                        "`latest`, a major version such as `155`, or a full version such as \
                         `155.0.7100.4` (geckodriver: `latest` or `0.36.0`); default: match the \
                         installed browser",
                    ),
                ),
            move |ctx, args| install_driver(Arc::clone(&installer), ctx, args),
        ),
    ]
}

// ----------------------------------------------------------------------
// plugins
// ----------------------------------------------------------------------

async fn list_plugins(ctx: Ctx<NoBrowser>, _args: Args) -> Result<Output, ToolError> {
    Ok(plugin_cmd::list(ctx.settings().plugin_selection()).into())
}

async fn plugin_info(ctx: Ctx<NoBrowser>, args: Args) -> Result<Output, ToolError> {
    let name = args.str("name")?;
    plugin_cmd::info(name, ctx.settings().plugin_selection())
        .map(Output::from)
        .map_err(|error| ToolError::invalid("name", error.to_string()))
}

// ----------------------------------------------------------------------
// install_driver
// ----------------------------------------------------------------------

/// The installs one process runs at a time: two would write the same file.
static INSTALLING: Mutex<()> = Mutex::const_new(());

/// Whether `version` is only what a version is written with.
fn is_version_text(version: &str) -> bool {
    !version.is_empty()
        && version.len() <= 32
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
}

/// The first line a driver prints for `--version`, `None` when it cannot be
/// run or prints nothing.
async fn version_line(driver: &Path) -> Option<String> {
    let output = tokio::time::timeout(
        VERSION_TIMEOUT,
        tokio::process::Command::new(driver)
            .arg("--version")
            .kill_on_drop(true)
            .output(),
    )
    .await
    .ok()?
    .ok()?;
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_owned)
}

async fn install_driver(
    installer: Installer,
    _ctx: Ctx<NoBrowser>,
    args: Args,
) -> Result<Output, ToolError> {
    let browser = args.choice("browser", &["chrome", "edge", "firefox"], "chrome")?;
    let version = nonempty(&args, "version")?;
    if let Some(version) = version.filter(|version| !is_version_text(version)) {
        return Err(ToolError::invalid(
            "version",
            format!("{version:?} is not a version: use `latest`, `155` or `155.0.7100.4`"),
        ));
    }
    let driver = match browser {
        "edge" => DriverName::Msedgedriver,
        "firefox" => DriverName::Geckodriver,
        _ => DriverName::Chromedriver,
    };
    let path = {
        let _one_at_a_time = INSTALLING.lock().await;
        installer(driver, version.map(str::to_owned)).await?
    };
    let printed = version_line(&path).await;
    json_output(json!({
        "driver": driver.to_string(),
        "browser": browser,
        "requested_version": version,
        "path": path.display().to_string(),
        "installed_version": printed,
        "cache": "the downloaded_drivers folder of the folder the server runs in",
        "note": if printed.is_none() {
            "The driver was installed but did not print a version (it may need to be allowed to \
             run, as on macOS); diagnose_environment checks it against the browser."
        } else {
            "Installed. diagnose_environment checks that the driver and the browser agree."
        },
    }))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use serde_json::Value;

    use super::*;
    use crate::mcp::Settings;

    async fn call(tool: &str, arguments: Value) -> Result<Output, ToolError> {
        super::super::host(Settings::new("out"))
            .call(tool, arguments.as_object().cloned().unwrap())
            .await
    }

    // ------------------------------------------------------------------
    // plugins
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn the_plugin_list_is_the_one_sbrs_prints() {
        let answer = call("list_plugins", json!({})).await.unwrap();

        let text = answer.as_text().unwrap();
        let expected = plugin_cmd::list(&crate::config::plugins::PluginSelection::new());
        assert_eq!(text, expected);
        if cfg!(feature = "plugins") && cfg!(feature = "plugin-shield") {
            assert!(text.contains("shield"), "{text}");
        } else if !cfg!(feature = "plugins") {
            assert!(text.contains("plugins-builtin"), "{text}");
        }
    }

    #[tokio::test]
    async fn plugin_info_for_a_name_the_build_lacks_is_an_argument_error_that_lists_the_ways_on() {
        let error = call("plugin_info", json!({ "name": "no-such-plugin" }))
            .await
            .unwrap_err();

        assert!(
            matches!(&error, ToolError::InvalidArgument { name, reason }
                if name == "name" && reason.contains("no-such-plugin")),
            "{error}"
        );
        if !cfg!(feature = "plugins") {
            assert!(error.to_string().contains("plugins-builtin"), "{error}");
        }
    }

    #[cfg(feature = "plugin-shield")]
    #[tokio::test]
    async fn plugin_info_describes_a_built_in_plugin() {
        let answer = call("plugin_info", json!({ "name": "shield" }))
            .await
            .unwrap();

        let text = answer.as_text().unwrap();
        assert!(text.starts_with("shield"), "{text}");
        assert!(text.contains("capabilities"), "{text}");
    }

    // ------------------------------------------------------------------
    // install_driver
    // ------------------------------------------------------------------

    /// What an installer was asked for: the driver and the version.
    type Asked = Arc<std::sync::Mutex<Vec<(DriverName, Option<String>)>>>;

    /// An installer that records what it was asked for and downloads nothing.
    fn recording(asked: &Asked, path: PathBuf) -> Installer {
        let asked = Arc::clone(asked);
        Arc::new(move |driver, version| {
            asked.lock().unwrap().push((driver, version));
            let path = path.clone();
            Box::pin(async move { Ok(path) })
        })
    }

    fn fake_host(installer: Installer) -> crate::mcp::Host<NoBrowser> {
        super::super::host_with_installer(Settings::new("out"), installer)
    }

    #[tokio::test]
    async fn install_driver_asks_the_installer_for_the_driver_of_the_browser() {
        let asked = Arc::default();
        let host = fake_host(recording(
            &asked,
            PathBuf::from("/no/such/dir/chromedriver"),
        ));

        let answer = host
            .call(
                "install_driver",
                json!({ "browser": "firefox", "version": "0.36.0" })
                    .as_object()
                    .cloned()
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            *asked.lock().unwrap(),
            [(DriverName::Geckodriver, Some("0.36.0".to_owned()))]
        );
        let answer = answer.as_json().unwrap();
        assert_eq!(answer["driver"], "geckodriver");
        assert_eq!(answer["path"], "/no/such/dir/chromedriver");
        assert_eq!(answer["requested_version"], "0.36.0");
        assert_eq!(answer["installed_version"], Value::Null);
        assert!(
            answer["note"]
                .as_str()
                .unwrap()
                .contains("did not print a version"),
            "{answer}"
        );
    }

    #[tokio::test]
    async fn without_arguments_it_is_chromedriver_for_the_installed_chrome() {
        let asked = Arc::default();
        let host = fake_host(recording(&asked, PathBuf::from("/no/such/chromedriver")));

        host.call("install_driver", serde_json::Map::new())
            .await
            .unwrap();

        assert_eq!(*asked.lock().unwrap(), [(DriverName::Chromedriver, None)]);
    }

    #[tokio::test]
    async fn edge_is_msedgedriver() {
        let asked = Arc::default();
        let host = fake_host(recording(&asked, PathBuf::from("/no/such/msedgedriver")));

        host.call(
            "install_driver",
            json!({ "browser": "edge", "version": "latest" })
                .as_object()
                .cloned()
                .unwrap(),
        )
        .await
        .unwrap();

        assert_eq!(
            *asked.lock().unwrap(),
            [(DriverName::Msedgedriver, Some("latest".to_owned()))]
        );
    }

    #[tokio::test]
    async fn a_version_that_is_not_a_version_never_reaches_the_installer() {
        let asked = Arc::default();
        let host = fake_host(recording(&asked, PathBuf::from("/no/such/chromedriver")));

        for version in ["../../etc", "1 2", "https://x.test/a.zip", &"9".repeat(40)] {
            let error = host
                .call(
                    "install_driver",
                    json!({ "version": version }).as_object().cloned().unwrap(),
                )
                .await
                .unwrap_err();
            assert!(
                matches!(&error, ToolError::InvalidArgument { name, .. } if name == "version"),
                "{version}: {error}"
            );
        }
        let error = host
            .call(
                "install_driver",
                json!({ "browser": "safari" }).as_object().cloned().unwrap(),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(error, ToolError::InvalidArgument { .. }),
            "{error}"
        );
        assert!(asked.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_failed_download_is_an_error_the_model_can_read() {
        let installer: Installer = Arc::new(|_, _| {
            Box::pin(async {
                Err(SeleniumBaseError::download(
                    "https://example.test/chromedriver.zip",
                    "connection refused",
                ))
            })
        });

        let error = fake_host(installer)
            .call("install_driver", serde_json::Map::new())
            .await
            .unwrap_err();

        assert!(matches!(error, ToolError::Failed(_)), "{error}");
        assert!(error.to_string().contains("connection refused"), "{error}");
    }

    #[tokio::test]
    async fn two_installs_never_run_at_once() {
        let running = Arc::new(AtomicUsize::new(0));
        let overlapped = Arc::new(AtomicUsize::new(0));
        let installer: Installer = {
            let running = Arc::clone(&running);
            let overlapped = Arc::clone(&overlapped);
            Arc::new(move |_, _| {
                let running = Arc::clone(&running);
                let overlapped = Arc::clone(&overlapped);
                Box::pin(async move {
                    if running.fetch_add(1, Ordering::SeqCst) > 0 {
                        overlapped.fetch_add(1, Ordering::SeqCst);
                    }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    running.fetch_sub(1, Ordering::SeqCst);
                    Ok(PathBuf::from("/no/such/chromedriver"))
                })
            })
        };
        let host = fake_host(installer);

        let (a, b) = tokio::join!(
            host.call("install_driver", serde_json::Map::new()),
            host.call("install_driver", serde_json::Map::new()),
        );

        a.unwrap();
        b.unwrap();
        assert_eq!(overlapped.load(Ordering::SeqCst), 0);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn the_version_the_installed_driver_prints_is_reported() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("chromedriver");
        std::fs::write(
            &script,
            "#!/bin/sh\necho 'ChromeDriver 155.0.7100.4 (abc)'\n",
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        // The first run of a freshly written file can be slow on macOS.
        let _ = std::process::Command::new(&script).arg("--warm").output();

        let asked = Arc::default();
        let answer = fake_host(recording(&asked, script.clone()))
            .call("install_driver", serde_json::Map::new())
            .await
            .unwrap();

        let answer = answer.as_json().unwrap();
        assert_eq!(
            answer["installed_version"],
            "ChromeDriver 155.0.7100.4 (abc)"
        );
        assert_eq!(answer["path"], script.display().to_string());
    }

    #[test]
    fn install_driver_is_marked_as_a_download_that_replaces_files() {
        let host = super::super::host(Settings::new("out"));
        let tool = host
            .describe_tools()
            .into_iter()
            .find(|tool| tool.name == "install_driver")
            .unwrap();
        let hints = tool.annotations.unwrap();
        assert_eq!(hints.read_only_hint, Some(false));
        assert_eq!(hints.open_world_hint, Some(true));
        assert_eq!(hints.destructive_hint, Some(true));
        assert_eq!(hints.idempotent_hint, Some(true));
    }
}
