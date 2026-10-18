//! The `dev` server: tools for developing with this library, using the `sbrs`
//! command line and finding out what is wrong. It starts no browser.
//!
//! Every tool of the browser servers costs context in every session that loads
//! them, so these live in a server of their own: load `dev` next to one of
//! `cdp`, `driver` or `sb`.
//!
//! | Job | Tools |
//! |---|---|
//! | Find the right method | `find_api`, `parity_lookup` |
//! | Write Rust tests | `python_to_rust`, `scaffold` |
//! | Use the command line | `cli_help`, `list_plugins`, `plugin_info`, `install_driver` |
//! | Diagnose | `explain_error`, `check_config`, `check_fingerprint`, `diagnose_environment` |
//!
//! Only `install_driver` changes anything (it downloads a driver); the rest read
//! embedded data or look at the machine. The design is in
//! `docs/design/mcp-dev-tools.md`.
//!
//! ```no_run
//! # async fn run() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! use seleniumbase_rs::mcp;
//!
//! mcp::serve(mcp::Profile::Dev).await?;
//! # Ok(())
//! # }
//! ```

mod api;
mod author;
mod cli;
mod diagnose;

pub use cli::{InstallFuture, Installer};

use super::{devtools, Closeable, Host, Settings, ToolDef};

const SERVER_NAME: &str = "seleniumbase-dev";

const INSTRUCTIONS: &str = "\
Helps develop with the seleniumbase-rs library, use the sbrs command line and find out what \
is wrong. It starts no browser; load it next to the cdp, driver or sb server. find_api and \
parity_lookup find the right Rust method, by name or by the Python name. python_to_rust and \
scaffold produce Rust tests. cli_help explains sbrs; list_plugins and plugin_info describe \
the built-in plugins. For a problem: explain_error reads an error text, check_config checks \
the settings and SB_* variables, check_fingerprint checks a fingerprint, and \
diagnose_environment checks Chrome, the driver and the machine. install_driver downloads a \
driver, the only tool here that changes anything.";

/// The session of the `dev` server: there is none, because the server has no
/// browser. A tool that asked for one would get
/// [`ToolError::NoBrowser`](super::ToolError::NoBrowser).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoBrowser;

impl Closeable for NoBrowser {
    async fn close(self) {}
}

/// Builds the `dev` server. `install_driver` downloads through the code of
/// `sbrs install`.
#[must_use]
pub fn host(settings: Settings) -> Host<NoBrowser> {
    host_with_installer(settings, cli::sbrs_installer())
}

/// [`host`] with `installer` in place of the downloader of `sbrs install`, so
/// that a test installs without the network.
#[must_use]
pub fn host_with_installer(settings: Settings, installer: Installer) -> Host<NoBrowser> {
    Host::new(SERVER_NAME, INSTRUCTIONS, tools(installer), settings)
}

fn tools(installer: Installer) -> Vec<ToolDef<NoBrowser>> {
    let mut tools = api::tools();
    tools.extend(author::tools());
    tools.extend(cli::tools(installer));
    tools.extend(diagnose::tools());
    tools.extend(devtools::environment_tools());
    tools
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tools of the server, in the order they are listed.
    const DEV_TOOLS: [&str; 12] = [
        "find_api",
        "parity_lookup",
        "python_to_rust",
        "scaffold",
        "cli_help",
        "list_plugins",
        "plugin_info",
        "install_driver",
        "explain_error",
        "check_config",
        "check_fingerprint",
        "diagnose_environment",
    ];

    #[test]
    fn the_server_lists_exactly_the_dev_tools() {
        let host = host(Settings::new("out"));

        assert_eq!(host.tool_names(), DEV_TOOLS);
        assert_eq!(host.name(), "seleniumbase-dev");
    }

    #[test]
    fn every_tool_has_a_description_a_title_a_schema_and_honest_hints() {
        let host = host(Settings::new("out"));

        for tool in host.describe_tools() {
            let name = tool.name.to_string();
            assert!(
                tool.description
                    .as_deref()
                    .is_some_and(|text| text.len() > 40),
                "{name} needs a description a model can choose by"
            );
            assert_eq!(
                tool.input_schema.get("type"),
                Some(&serde_json::json!("object")),
                "{name}"
            );
            assert_eq!(
                tool.input_schema.get("additionalProperties"),
                Some(&serde_json::json!(false)),
                "{name}"
            );
            let hints = tool
                .annotations
                .unwrap_or_else(|| panic!("{name} has no annotations"));
            assert!(
                hints
                    .title
                    .as_deref()
                    .is_some_and(|title| !title.is_empty()),
                "{name} has no title"
            );
            // Nothing here reads or changes a web page.
            if name == "install_driver" {
                assert_eq!(hints.read_only_hint, Some(false), "{name}");
                assert_eq!(hints.open_world_hint, Some(true), "{name}");
            } else {
                assert_eq!(hints.read_only_hint, Some(true), "{name}");
                assert_eq!(hints.destructive_hint, Some(false), "{name}");
                assert_eq!(hints.open_world_hint, Some(false), "{name}");
            }
        }
    }

    #[test]
    fn the_required_arguments_are_declared() {
        let host = host(Settings::new("out"));

        for tool in host.describe_tools() {
            let properties = tool
                .input_schema
                .get("properties")
                .and_then(serde_json::Value::as_object);
            let required = tool
                .input_schema
                .get("required")
                .and_then(serde_json::Value::as_array);
            for name in required.into_iter().flatten() {
                let name = name.as_str().unwrap();
                assert!(
                    properties.is_some_and(|p| p.contains_key(name)),
                    "{} requires {name}, which it does not declare",
                    tool.name
                );
            }
        }
    }

    #[tokio::test]
    async fn no_tool_needs_a_browser() {
        let host = host(Settings::new("out"));

        let environment = host
            .call("diagnose_environment", serde_json::Map::new())
            .await
            .unwrap();

        let report = environment.as_json().unwrap();
        assert_eq!(report["browser_session"], "none");
        // What it covers: the browser, the driver and their fit, the driver cache, the
        // output folder, and the features of this build.
        for key in [
            "chrome",
            "chromedriver",
            "versions",
            "driver_cache",
            "output_dir",
            "build",
        ] {
            assert!(!report[key].is_null(), "{key}: {report}");
        }
        assert!(report["driver_cache"]["path"].is_string());
        assert!(report["driver_cache"]["exists"].is_boolean());
        assert!(report["build"]["features"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("mcp-server")));
        assert!(host.ctx().try_session().await.is_none());
    }

    #[test]
    fn the_profile_is_known_by_name() {
        assert_eq!(
            "dev".parse::<crate::mcp::Profile>(),
            Ok(crate::mcp::Profile::Dev)
        );
        assert_eq!(crate::mcp::Profile::Dev.name(), "dev");
        let error = "browser".parse::<crate::mcp::Profile>().unwrap_err();
        assert!(error.contains("dev"), "{error}");
        assert!(crate::mcp::Profile::ALL.contains(&crate::mcp::Profile::Dev));
    }
}
