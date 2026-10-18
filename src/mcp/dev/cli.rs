//! `cli_help`, `list_plugins`, `plugin_info` and `install_driver`: the `sbrs`
//! command line, from inside an MCP session.
//!
//! `cli_help` reads the CLI reference page (`docs/help/cli_reference.md`),
//! embedded and cut by heading. That page is generated from the usage spec of
//! `sbrs` (`completions/sbrs.usage.kdl`) by a test of the CLI crate, so it
//! cannot drift from the parser. Parsing the spec itself with `usage-lib`
//! would add that crate and what it brings to every build of the library with
//! the MCP server; the page already holds every flag, default, alias and
//! choice.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use serde_json::json;
use tokio::sync::Mutex;

use super::NoBrowser;
use crate::cli::plugin_cmd;
use crate::cli::scripts::sb_install::{self, DriverName};
use crate::error::{closest_match, SeleniumBaseError};
use crate::mcp::support::{json_output, nonempty};
use crate::mcp::{Args, Ctx, Effect, Output, Prop, Schema, ToolDef, ToolError};

/// The CLI reference page, generated from the usage spec of `sbrs`.
const REFERENCE: &str = include_str!("../../../docs/help/cli_reference.md");

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
pub(super) fn tools(installer: &Installer) -> Vec<ToolDef<NoBrowser>> {
    let installer = Arc::clone(installer);
    vec![
        ToolDef::new(
            "cli_help",
            "CLI Help",
            "Explain the `sbrs` command line. With no command: the global flags, one line \
             each, and every command with a one-line description. With a command (`open`, \
             `wait-for-text`, `plugin list`): its usage, arguments, flags with their defaults \
             and choices, and aliases. With `global`: the global flags in full. It is the \
             reference page of this version, the same text `sbrs --help` is built from.",
            Effect::Inspect,
            Schema::new().optional(
                "command",
                Prop::string(
                    "A command or alias, such as `mkdir`, `install` or `plugin list`; `global` \
                     for the flags that come before the command",
                ),
            ),
            cli_help,
        ),
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
// cli_help
// ----------------------------------------------------------------------

/// One command of the reference page: `sbrs` itself or `sbrs <command>`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Section {
    /// `sbrs`, `sbrs open`, `sbrs plugin list`.
    name: String,
    /// The usage line without its backticks.
    usage: String,
    /// The version line of the root section.
    version: Option<String>,
    aliases: Vec<String>,
    /// The first paragraph after the facts.
    description: String,
    /// The section's lines, as markdown.
    lines: Vec<String>,
}

impl Section {
    /// `open` for `sbrs open`; empty for the root.
    fn command(&self) -> &str {
        self.name.strip_prefix("sbrs").unwrap_or(&self.name).trim()
    }

    /// The section as markdown, trimmed.
    fn markdown(&self) -> String {
        format!("## {}\n{}", self.name, self.lines.join("\n"))
            .trim_end()
            .to_owned()
    }
}

/// The name in a heading such as ``## `sbrs open` `` or ``# `sbrs` ``.
fn heading_name(line: &str) -> Option<&str> {
    let rest = line
        .strip_prefix("## ")
        .or_else(|| line.strip_prefix("# "))?;
    let name = rest.strip_prefix('`')?.strip_suffix('`')?;
    (name == "sbrs" || name.starts_with("sbrs ")).then_some(name)
}

/// The text after `label` in a line such as `- **Usage:** `sbrs open``.
fn fact<'a>(lines: &'a [String], label: &str) -> Option<&'a str> {
    let prefix = format!("- **{label}:** ");
    lines
        .iter()
        .find_map(|line| line.strip_prefix(prefix.as_str()))
}

fn finish(name: &str, lines: Vec<String>) -> Section {
    let unquote = |text: &str| text.trim().trim_matches('`').to_owned();
    let aliases = fact(&lines, "Aliases")
        .map(|list| list.split(", ").map(unquote).collect())
        .unwrap_or_default();
    let description = lines
        .iter()
        .map(String::as_str)
        .skip_while(|line| line.trim().is_empty() || line.starts_with("- **"))
        .take_while(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .collect::<Vec<_>>()
        .join(" ");
    Section {
        name: name.to_owned(),
        usage: fact(&lines, "Usage").map(unquote).unwrap_or_default(),
        version: fact(&lines, "Version").map(str::to_owned),
        aliases,
        description,
        lines,
    }
}

/// Cuts the reference page into one section per command.
fn parse_reference(page: &str) -> Vec<Section> {
    let mut sections = Vec::new();
    let mut current: Option<(String, Vec<String>)> = None;
    for line in page.lines() {
        if let Some(name) = heading_name(line) {
            if let Some((previous, lines)) = current.take() {
                sections.push(finish(&previous, lines));
            }
            current = Some((name.to_owned(), Vec::new()));
        } else if let Some((_, lines)) = &mut current {
            lines.push(line.to_owned());
        }
    }
    if let Some((previous, lines)) = current {
        sections.push(finish(&previous, lines));
    }
    sections
}

fn reference() -> &'static [Section] {
    static SECTIONS: OnceLock<Vec<Section>> = OnceLock::new();
    SECTIONS.get_or_init(|| parse_reference(REFERENCE))
}

/// The overview: global flags and every command.
fn overview(sections: &[Section]) -> String {
    let mut text = String::new();
    if let Some(root) = sections.first().filter(|section| section.name == "sbrs") {
        text.push_str(&format!(
            "sbrs {}\n{}\nUsage: {}\n\n",
            root.version.as_deref().unwrap_or(""),
            root.description,
            root.usage
        ));
        // The root's flags follow its own facts, under a `## Flags` heading. The overview
        // keeps the first line of each; their choices, aliases and long help are in
        // `cli_help global`.
        let flags: Vec<&str> = root
            .lines
            .iter()
            .map(String::as_str)
            .skip_while(|line| *line != "## Flags")
            .skip(1)
            .filter(|line| line.starts_with("- "))
            .collect();
        text.push_str(
            "Global flags (they come before the command; cli_help with command `global` gives \
             their choices, aliases and long help):\n",
        );
        text.push_str(&flags.join("\n"));
        text.push_str("\n\n");
    }
    let commands: Vec<&Section> = sections
        .iter()
        .filter(|section| section.name != "sbrs")
        .collect();
    text.push_str(&format!(
        "Commands ({}); cli_help with a command gives its arguments, flags, defaults and \
         aliases:\n",
        commands.len()
    ));
    for section in commands {
        text.push_str(&format!("- {}", section.command()));
        if !section.aliases.is_empty() {
            text.push_str(&format!(" (alias {})", section.aliases.join(", ")));
        }
        if !section.description.is_empty() {
            text.push_str(&format!(": {}", section.description));
        }
        text.push('\n');
    }
    text
}

/// `command` as the reference page names a command: lowercase, with `-` for `_`, and
/// without a leading `sbrs`. The root (`sbrs`, `global`) is the empty name.
fn bare(command: &str) -> String {
    let lower = command.trim().to_lowercase().replace('_', "-");
    let name = match lower.strip_prefix("sbrs") {
        Some(rest) if rest.is_empty() || rest.starts_with(' ') => rest.trim(),
        _ => lower.as_str(),
    };
    if matches!(name, "global" | "global flags" | "flags") {
        String::new()
    } else {
        name.to_owned()
    }
}

/// The sections `command` names: the command itself (or the command an alias
/// stands for), and, for a command that only groups others, its subcommands. The
/// root, with the global flags, is `sbrs` or `global`.
fn find_command<'a>(sections: &'a [Section], command: &str) -> Vec<&'a Section> {
    let wanted = bare(command);
    if wanted.is_empty() {
        return sections.first().into_iter().collect();
    }
    let exact = sections
        .iter()
        .find(|section| section.name != "sbrs" && section.command() == wanted)
        .or_else(|| {
            sections
                .iter()
                .find(|section| section.aliases.iter().any(|alias| *alias == wanted))
        });
    let children = sections
        .iter()
        .filter(|section| section.command().starts_with(&format!("{wanted} ")));
    exact.into_iter().chain(children).collect()
}

async fn cli_help(_ctx: Ctx<NoBrowser>, args: Args) -> Result<Output, ToolError> {
    let sections = reference();
    let Some(command) = nonempty(&args, "command")? else {
        return Ok(overview(sections).into());
    };
    let found = find_command(sections, command);
    if found.is_empty() {
        let names = sections
            .iter()
            .filter(|section| section.name != "sbrs")
            .map(Section::command);
        let mut reason = format!("no command `{command}`; cli_help without a command lists them");
        if let Some(near) = closest_match(&bare(command), names) {
            reason.push_str(&format!("; did you mean `{near}`?"));
        }
        return Err(ToolError::invalid("command", reason));
    }
    let text = found
        .iter()
        .map(|section| section.markdown())
        .collect::<Vec<_>>()
        .join("\n\n");
    Ok(text.into())
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
    let installed = version_line(&path).await;
    json_output(json!({
        "driver": driver.to_string(),
        "browser": browser,
        "requested_version": version,
        "path": path.display().to_string(),
        "installed_version": installed,
        "cache": "the downloaded_drivers folder of the folder the server runs in",
        "note": if installed.is_none() {
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
    // cli_help
    // ------------------------------------------------------------------

    #[test]
    fn the_reference_page_is_cut_into_one_section_per_command() {
        let sections = reference();
        assert_eq!(sections[0].name, "sbrs");
        assert!(sections.len() > 50, "{} sections", sections.len());
        let open = sections.iter().find(|s| s.name == "sbrs open").unwrap();
        assert_eq!(open.usage, "sbrs open <URL>");
        assert_eq!(open.description, "Open a URL in the browser.");
        let names: std::collections::BTreeSet<_> =
            sections.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names.len(), sections.len(), "a command appears twice");
    }

    #[test]
    fn every_command_heading_of_the_page_becomes_a_section() {
        let headings = REFERENCE
            .lines()
            .filter(|line| heading_name(line).is_some())
            .count();
        assert_eq!(headings, reference().len());
    }

    #[tokio::test]
    async fn without_a_command_the_help_lists_the_global_flags_and_every_command() {
        let answer = call("cli_help", json!({})).await.unwrap();

        let text = answer.as_text().unwrap();
        assert!(text.starts_with("sbrs "), "{text}");
        assert!(text.contains("Global flags"), "{text}");
        assert!(text.contains("--webdriver <URL>"), "{text}");
        assert!(
            text.contains("- open: Open a URL in the browser."),
            "{text}"
        );
        assert!(text.contains("- encrypt (alias obfuscate): "), "{text}");
        let listed = text.lines().filter(|l| l.starts_with("- ")).count();
        let commands = reference().len() - 1;
        // The global flags are bullets too.
        assert!(listed >= commands, "{listed} lines for {commands} commands");
    }

    #[tokio::test]
    async fn a_command_shows_its_flags_with_their_defaults() {
        let answer = call("cli_help", json!({ "command": "wait-for-text" }))
            .await
            .unwrap();

        let text = answer.as_text().unwrap();
        assert!(text.starts_with("## sbrs wait-for-text"), "{text}");
        assert!(text.contains("--timeout <TIMEOUT>"), "{text}");
        assert!(text.contains("**Default:** `10`"), "{text}");
    }

    #[tokio::test]
    async fn an_alias_and_the_sbrs_prefix_find_the_command() {
        for given in ["obfuscate", "sbrs encrypt", "Encrypt"] {
            let answer = call("cli_help", json!({ "command": given })).await.unwrap();
            assert!(
                answer.as_text().unwrap().starts_with("## sbrs encrypt"),
                "{given}"
            );
        }
    }

    #[tokio::test]
    async fn the_overview_keeps_one_line_for_each_global_flag_and_global_gives_them_in_full() {
        let overview = call("cli_help", json!({})).await.unwrap();
        let overview = overview.as_text().unwrap();
        assert!(overview.contains("--webdriver <URL>"), "{overview}");
        assert!(!overview.contains("**Choices:**"), "{overview}");
        assert!(!overview.contains("Adds to the plugins"), "{overview}");

        for given in ["global", "sbrs", "Global Flags"] {
            let full = call("cli_help", json!({ "command": given })).await.unwrap();
            let full = full.as_text().unwrap();
            assert!(full.starts_with("## sbrs"), "{given}: {full}");
            assert!(full.contains("**Choices:** `chrome`"), "{given}");
            assert!(
                full.contains("Adds to the plugins the settings file"),
                "{given}"
            );
        }
    }

    #[test]
    fn a_command_is_read_as_the_page_names_it() {
        assert_eq!(bare("  SBRS Wait_For_Text "), "wait-for-text");
        assert_eq!(bare("sbrs plugin list"), "plugin list");
        assert_eq!(bare("sbrs"), "");
        assert_eq!(bare("global"), "");
        // Only the word `sbrs` is dropped, not letters a command starts with.
        assert_eq!(bare("sbrsx"), "sbrsx");
    }

    #[tokio::test]
    async fn a_group_of_commands_shows_itself_and_its_subcommands() {
        let answer = call("cli_help", json!({ "command": "plugin" }))
            .await
            .unwrap();

        let text = answer.as_text().unwrap();
        assert!(text.contains("## sbrs plugin list"), "{text}");
        assert!(text.contains("## sbrs plugin info"), "{text}");
    }

    #[tokio::test]
    async fn an_unknown_command_is_an_error_with_the_nearest_name() {
        let error = call("cli_help", json!({ "command": "opn" }))
            .await
            .unwrap_err();

        assert!(
            matches!(&error, ToolError::InvalidArgument { name, reason }
                if name == "command" && reason.contains("did you mean `open`?")),
            "{error}"
        );
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

    /// An installer that records what it was asked for and downloads nothing.
    fn recording(
        asked: &Arc<std::sync::Mutex<Vec<(DriverName, Option<String>)>>>,
        path: PathBuf,
    ) -> Installer {
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
