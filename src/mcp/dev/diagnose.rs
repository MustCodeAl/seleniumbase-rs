//! `explain_error`, `check_config` and `check_fingerprint`: finding out what
//! is wrong without starting a browser.
//!
//! `explain_error` turns the text of an error into the library's own advice:
//! it rebuilds a [`SeleniumBaseError`] of the right category and returns its
//! `category()`, `hint()` and `exit_code()`, with the sections of the
//! common-problems page (`docs/help/common_problems.md`, embedded) that apply.
//! `check_config` loads the settings the way `sbrs` does and reports every
//! problem, not only the first. `check_fingerprint` parses a fingerprint, or an
//! anti-detect-browser profile payload, and reports its coherence.

use std::path::{Component, Path, PathBuf};

use serde_json::{json, Map, Value};
use thirtyfour::error::{WebDriverError, WebDriverErrorInner};

use super::NoBrowser;
use crate::cli::plugin_cmd;
use crate::config::plugins::PluginSelection;
use crate::config::settings::Settings;
use crate::config::RuntimeConfig;
use crate::error::SeleniumBaseError;
use crate::evasion::fingerprint::Fingerprint;
use crate::mcp::devtools::redact_setting;
use crate::mcp::failure::FailureReport;
use crate::mcp::stealth::coherence_report;
use crate::mcp::support::{json_output, nonempty};
use crate::mcp::{Args, Ctx, Effect, Output, Prop, Schema, ToolDef, ToolError};
use crate::profile_payloads::ProfileParams;

/// The common-problems page.
const COMMON_PROBLEMS: &str = include_str!("../../../docs/help/common_problems.md");

/// The most text of a fingerprint or an error message one call reads.
const MAX_TEXT_BYTES: usize = 256 * 1024;

/// The longest a quoted piece of an error message is repeated, in characters.
const MAX_ECHO: usize = 300;

/// The tools of this file.
pub(super) fn tools() -> Vec<ToolDef<NoBrowser>> {
    vec![
        ToolDef::new(
            "explain_error",
            "Explain Error",
            "Explain an error of this library or of an MCP server: from its text (or just its \
             category tag) give the category, the library's hint with the likely cause and the \
             fix, the exit status `sbrs` ends with, what to try next, and the sections of the \
             common-problems page that apply. An error it does not recognize is said to be \
             unrecognized, not guessed at.",
            Effect::Inspect,
            Schema::new()
                .optional(
                    "message",
                    Prop::string(
                        "The error text, as printed (`element not found: #buy (strategy: css \
                         selector)`, `wait timeout: ...`, or a whole failure report)",
                    ),
                )
                .optional(
                    "category",
                    Prop::choice(
                        &ALL_CATEGORIES,
                        "The category tag (the `kind` of a failure report), when there is no \
                         text; the advice is then the general one for the category",
                    ),
                ),
            explain_error,
        ),
        ToolDef::new(
            "check_config",
            "Check Config",
            "Check the configuration without starting anything: read the settings the way \
             `sbrs` does (the settings file, then the SB_* environment variables) and report \
             every problem, not only the first: an unreadable file, each variable that does not \
             parse, an unknown browser or mode, a window size with one side, plugins this build \
             cannot run. Also the runtime variables (SB_LOG_LEVEL and the rest, whose mistakes \
             are ignored with a warning) and a summary of what would be launched. Secrets in \
             the summary are redacted.",
            Effect::Inspect,
            Schema::new().optional(
                "path",
                Prop::string(
                    "A settings file (TOML, or JSON for another extension), relative to the \
                     folder the server runs in; default: sbase_config.toml there, if any",
                ),
            ),
            check_config,
        ),
        ToolDef::new(
            "check_fingerprint",
            "Check Fingerprint",
            "Check a fingerprint without starting a browser: parse JSON as a fingerprint of this \
             library, or as the profile payload of an anti-detect browser tool, and report the \
             errors and warnings of Fingerprint::validate (a platform that contradicts the user \
             agent, a mobile agent without touch points, and the rest). A document that parses \
             as neither is described, not guessed at.",
            Effect::Inspect,
            Schema::new().required(
                "json",
                Prop::string("The fingerprint or the profile payload, as JSON text"),
            ),
            check_fingerprint,
        ),
    ]
}

// ----------------------------------------------------------------------
// explain_error
// ----------------------------------------------------------------------

/// Every category of [`SeleniumBaseError`] and the start of the message its
/// `Display` writes, so a message can be told from its text. A test builds each
/// category and checks that its message starts as written here.
const CATEGORIES: [(&str, &str); 32] = [
    ("element_not_found", "element not found: "),
    ("element_not_interactable", "element not interactable: "),
    ("stale_element", "stale element reference: "),
    ("invalid_selector", "invalid selector: "),
    ("wait_timeout", "wait timeout: "),
    ("browser_launch", "failed to launch browser binary "),
    ("browser_disconnected", "browser disconnected: "),
    ("session_not_started", "no active browser session"),
    ("navigation", "navigation to "),
    ("webdriver", "webdriver command failed: "),
    ("cdp_driver", "CDP driver error: "),
    ("playwright", "playwright error: "),
    ("patcher", "binary patch failed for "),
    ("stealth", "stealth error: "),
    ("network", "network request to "),
    ("download", "download failed for "),
    ("io", "I/O error: "),
    ("json", "JSON error: "),
    ("parse", "parse error: expected "),
    ("invalid_config", "invalid configuration: "),
    ("assertion_failed", "assertion failed: "),
    ("unsupported", "unsupported operation: "),
    ("gui", "GUI input error: "),
    ("screenshot", "screenshot error: "),
    ("pdf", "PDF error: "),
    ("authentication", "authentication error: "),
    ("mcp", "MCP tool "),
    ("python_migration", "Python migration error: "),
    ("skipped", "test skipped: "),
    ("test_lifecycle", "browser test lifecycle failed: "),
    ("database", "database error: "),
    ("plugin", "plugin error: "),
];

/// What an MCP server itself says when a call cannot be run, as the `kind` of
/// a failure report, with the start of the message and what to do.
const TOOL_KINDS: [(&str, &str, &str); 4] = [
    (
        "no_browser",
        "no browser session is running",
        "A browser tool was called before start_browser. Call start_browser on the cdp, driver \
         or sb server first. The dev server has no browser: it is for the library and the \
         command line, next to a browser server.",
    ),
    (
        "invalid_argument",
        "invalid argument '",
        "A tool argument is missing or has the wrong type. The message names the argument; the \
         tool's input schema in tools/list says what it takes.",
    ),
    (
        "refused",
        "refused: ",
        "The server's configuration forbids the request, for example a file name that leaves \
         the output directory. Choose a name inside it, or ask the operator to change the \
         setting.",
    ),
    (
        "unknown_tool",
        "unknown tool: ",
        "No tool has that name on this server. tools/list names them; a browser tool is only on \
         the cdp, driver and sb servers, and the library tools only on the dev server.",
    ),
];

/// Every value `explain_error` takes as a category: the library's, then the
/// server's own.
const ALL_CATEGORIES: [&str; 36] = [
    "element_not_found",
    "element_not_interactable",
    "stale_element",
    "invalid_selector",
    "wait_timeout",
    "browser_launch",
    "browser_disconnected",
    "session_not_started",
    "navigation",
    "webdriver",
    "cdp_driver",
    "playwright",
    "patcher",
    "stealth",
    "network",
    "download",
    "io",
    "json",
    "parse",
    "invalid_config",
    "assertion_failed",
    "unsupported",
    "gui",
    "screenshot",
    "pdf",
    "authentication",
    "mcp",
    "python_migration",
    "skipped",
    "test_lifecycle",
    "database",
    "plugin",
    "no_browser",
    "invalid_argument",
    "refused",
    "unknown_tool",
];

/// What `sbrs` exit statuses mean, as the table of `SeleniumBaseError::exit_code`.
const EXIT_MEANING: [(u8, &str); 7] = [
    (1, "a check or a test failed"),
    (
        2,
        "the input was wrong: configuration, selector syntax, a value that does not parse",
    ),
    (
        3,
        "an element could not be found or used, or a wait timed out",
    ),
    (4, "the browser or its driver failed"),
    (5, "the network or a site failed"),
    (6, "a local file or database failed"),
    (7, "the MCP server failed"),
];

/// The sections of the common-problems page that help with `category`.
fn docs_headings(category: &str) -> &'static [&'static str] {
    match category {
        "element_not_found"
        | "wait_timeout"
        | "stale_element"
        | "element_not_interactable"
        | "invalid_selector"
        | "assertion_failed" => &["An element is not found or a wait times out"],
        "webdriver" => &[
            "chromedriver executable not found",
            "An element is not found or a wait times out",
        ],
        "browser_launch" => &[
            "Chrome is not found",
            "Chrome will not start in a container",
            "chromedriver executable not found",
        ],
        "browser_disconnected" => &["Chrome will not start in a container"],
        "patcher" => &["chromedriver is killed or refused on macOS"],
        "stealth" => &["Detected as a bot despite UC mode", "Headless detected"],
        "unsupported" | "playwright" => &[
            "Item not found: a Cargo feature is off",
            "Playwright / rustwright feature not compiling",
        ],
        "invalid_config" | "parse" | "json" => &["Environment variable not applied"],
        "mcp" | "no_browser" | "unknown_tool" => &["MCP server not listing tools"],
        "test_lifecycle" => &["Passes alone, fails in the suite"],
        _ => &[],
    }
}

/// `text` cut to `limit` characters.
fn cut(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        text.to_owned()
    } else {
        text.chars().take(limit).collect::<String>() + "…"
    }
}

/// `'name'` at the start of `text` and what follows it.
fn quoted(text: &str) -> (&str, &str) {
    let inner = text.strip_prefix('\'').unwrap_or(text);
    inner.split_once('\'').unwrap_or((inner, ""))
}

/// The error of `category` that `rest` (the message after the category's
/// prefix) describes, or `None` for a category the library does not have.
///
/// Parts the message does not carry are left empty or generic, so the hint the
/// library writes for them is the general one.
fn rebuild(category: &str, rest: &str) -> Option<SeleniumBaseError> {
    use SeleniumBaseError as E;
    let rest = rest.trim();
    Some(match category {
        "element_not_found" => {
            let selector = rest
                .rsplit_once(" (strategy: ")
                .map_or(rest, |(selector, _)| selector);
            E::element_not_found(selector)
        }
        "element_not_interactable" => {
            let (selector, reason) = rest
                .rsplit_once(" (")
                .map_or((rest, ""), |(selector, reason)| {
                    (selector, reason.trim_end_matches(')'))
                });
            E::element_not_interactable(selector, reason)
        }
        "stale_element" => E::stale_element(rest),
        "invalid_selector" => E::InvalidSelector(rest.to_owned()),
        "wait_timeout" => E::WaitTimeout(rest.to_owned()),
        "browser_launch" => {
            let (binary, after) = quoted(rest);
            E::browser_launch(binary, after.trim_start_matches(':').trim())
        }
        "browser_disconnected" => E::browser_disconnected(rest),
        "session_not_started" => E::SessionNotStarted,
        "navigation" => {
            let (url, after) = quoted(rest);
            E::navigation(url, after.trim().trim_start_matches("failed:").trim())
        }
        "webdriver" => E::WebDriver(WebDriverError::from_inner(
            WebDriverErrorInner::UnknownResponse(0, rest.to_owned()),
        )),
        "cdp_driver" => E::cdp_driver(rest),
        "playwright" => E::playwright(rest),
        "patcher" => {
            let (path, after) = quoted(rest);
            E::patcher(path, after.trim_start_matches(':').trim())
        }
        "stealth" => E::stealth(rest),
        "network" => {
            let (url, after) = quoted(rest);
            let after = after.trim().trim_start_matches("failed").trim();
            let (status, reason) = after
                .strip_prefix('(')
                .and_then(|inner| inner.split_once("):"))
                .map_or((0, after), |(status, reason)| {
                    (status.trim().parse().unwrap_or(0), reason.trim())
                });
            E::network(url, status, reason)
        }
        "download" => {
            let (url, after) = quoted(rest);
            E::download(url, after.trim_start_matches(':').trim())
        }
        "io" => E::Io(std::io::Error::new(io_kind(rest), rest.to_owned())),
        "json" => E::Json(json_error(rest)),
        "parse" => {
            let (expected, input) = rest.split_once(", got '").unwrap_or((rest, ""));
            E::parse(input.trim_end_matches('\''), expected)
        }
        "invalid_config" => E::invalid_config(rest),
        "assertion_failed" => E::assertion_failed(rest),
        "unsupported" => E::unsupported(rest),
        "gui" => E::gui(rest),
        "screenshot" => E::screenshot(rest),
        "pdf" => E::pdf(rest),
        "authentication" => E::authentication(rest),
        "mcp" => {
            let (tool, after) = quoted(rest);
            E::mcp(tool, after.trim().trim_start_matches("failed:").trim())
        }
        "python_migration" => E::python_migration(rest),
        "skipped" => E::skipped(rest),
        "test_lifecycle" => E::test_lifecycle(rest),
        "database" => E::Database(rest.to_owned()),
        "plugin" => E::plugin("", rest),
        _ => return None,
    })
}

/// The kind of I/O error a message describes, for the hint that depends on it.
fn io_kind(message: &str) -> std::io::ErrorKind {
    use std::io::ErrorKind;
    let lower = message.to_lowercase();
    [
        ("no such file", ErrorKind::NotFound),
        ("not found", ErrorKind::NotFound),
        ("permission denied", ErrorKind::PermissionDenied),
        ("file exists", ErrorKind::AlreadyExists),
        ("already exists", ErrorKind::AlreadyExists),
        ("timed out", ErrorKind::TimedOut),
        ("connection refused", ErrorKind::ConnectionRefused),
        ("broken pipe", ErrorKind::BrokenPipe),
    ]
    .into_iter()
    .find(|(text, _)| lower.contains(text))
    .map_or(ErrorKind::Other, |(_, kind)| kind)
}

/// A JSON error at the line and column a message names (`line 3 column 7`),
/// or at the start when it names none, so the hint quotes the right place.
fn json_error(message: &str) -> serde_json::Error {
    let number_after = |label: &str| {
        message
            .split_once(label)
            .and_then(|(_, rest)| {
                rest.trim_start()
                    .split(|c: char| !c.is_ascii_digit())
                    .next()
                    .and_then(|digits| digits.parse::<usize>().ok())
            })
            .filter(|n| (1..=100_000).contains(n))
    };
    let line = number_after("line").unwrap_or(1);
    let column = number_after("column").unwrap_or(1);
    let input = format!("{}{}}}", "\n".repeat(line - 1), " ".repeat(column - 1));
    serde_json::from_str::<Value>(&input).expect_err("a lone closing brace is not JSON")
}

/// The sample message for a category given without text.
fn placeholder(category: &str) -> &'static str {
    match category {
        "element_not_found" | "element_not_interactable" | "stale_element" | "invalid_selector" => {
            "<selector>"
        }
        "browser_launch" | "patcher" => "'<path>': <reason>",
        "navigation" => "'<url>' failed: <reason>",
        "network" => "'<url>' failed (0): <reason>",
        "download" => "'<url>': <reason>",
        "mcp" => "'<tool>' failed: <reason>",
        _ => "<details>",
    }
}

/// The category the text of `message` starts as, searched for anywhere in it
/// so a report that wraps the error (`click_element failed: element not
/// found: ...`) still counts. The earliest match wins.
fn classify(message: &str) -> Option<(&'static str, &str)> {
    let from_library = CATEGORIES.iter().filter_map(|(category, prefix)| {
        message.find(prefix).map(|at| (at, *category, prefix.len()))
    });
    let from_server = TOOL_KINDS
        .iter()
        .filter_map(|(kind, prefix, _)| message.find(prefix).map(|at| (at, *kind, 0)));
    from_library
        .chain(from_server)
        .min_by_key(|(at, _, _)| *at)
        .map(|(at, category, length)| {
            let rest = message[at + length..].lines().next().unwrap_or_default();
            (category, rest)
        })
}

/// `markdown` of the common-problems page without the page components:
/// `<Step title="x">` becomes a line `- x`, other component tags go.
fn plain(markdown: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut in_code = false;
    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_code = !in_code;
        }
        let tag = !in_code
            && trimmed.starts_with('<')
            && trimmed.ends_with('>')
            && trimmed
                .trim_start_matches("</")
                .trim_start_matches('<')
                .starts_with(|c: char| c.is_ascii_uppercase());
        if tag {
            if let Some(title) = trimmed
                .split_once("title=\"")
                .and_then(|(_, rest)| rest.split_once('"'))
                .map(|(title, _)| title)
                .filter(|_| trimmed.starts_with("<Step"))
            {
                lines.push(format!("- {title}"));
            }
            continue;
        }
        if trimmed.is_empty() && lines.last().is_some_and(String::is_empty) {
            continue;
        }
        lines.push(line.to_owned());
    }
    lines.join("\n").trim().to_owned()
}

/// The section `heading` of the common-problems page.
fn docs_section(heading: &str) -> Option<Value> {
    let marker = format!("## {heading}");
    let mut lines = COMMON_PROBLEMS.lines();
    lines.find(|line| *line == marker)?;
    let body: Vec<&str> = lines.take_while(|line| !line.starts_with("## ")).collect();
    Some(json!({ "heading": heading, "text": cut(&plain(&body.join("\n")), 3000) }))
}

/// The answer for an error: from its `message`, or from a `category` alone.
fn explain(message: Option<&str>, category: Option<&str>) -> Value {
    let recognized = match (message, category) {
        (Some(message), Some(category)) => {
            // The category is the caller's; the text only supplies the details.
            let rest = classify(message)
                .filter(|(found, _)| *found == category)
                .map_or(message, |(_, rest)| rest);
            Some((category, rest, true))
        }
        (Some(message), None) => classify(message).map(|(category, rest)| (category, rest, true)),
        (None, Some(category)) => Some((category, placeholder(category), false)),
        (None, None) => None,
    };
    let Some((category, rest, specific)) = recognized else {
        return json!({
            "recognized": false,
            "message": message.map(|text| cut(text, MAX_ECHO)),
            "advice": "This text does not start like any error of this library or of an MCP \
                       server. Give the error exactly as printed, or a category.",
            "known_categories": ALL_CATEGORIES.as_slice(),
            "docs": [docs_section("Reading an error")],
        });
    };

    if let Some((kind, _, advice)) = TOOL_KINDS.iter().find(|(kind, _, _)| *kind == category) {
        return json!({
            "recognized": true,
            "category": kind,
            "origin": "the MCP server, not the library",
            "hint": advice,
            "docs": docs_headings(kind).iter().filter_map(|h| docs_section(h)).collect::<Vec<_>>(),
        });
    }
    let Some(error) = rebuild(category, rest) else {
        return json!({
            "recognized": false,
            "message": message.map(|text| cut(text, MAX_ECHO)),
            "advice": format!("{category:?} is not a category of this library."),
            "known_categories": ALL_CATEGORIES.as_slice(),
        });
    };

    let exit_code = error.exit_code();
    let name = error.category();
    let hint = error.hint();
    let transient = error.is_transient();
    let steps = FailureReport::new(
        "explain_error",
        &Map::new(),
        &ToolError::Failed(error),
        None,
        &|_| true,
    )
    .next_steps;
    let docs: Vec<Value> = docs_headings(category)
        .iter()
        .filter_map(|heading| docs_section(heading))
        .collect();
    let mut answer = json!({
        "recognized": true,
        "category": name,
        "from": if message.is_some() { "message" } else { "category" },
        "specific": specific,
        "hint": hint,
        "transient": transient,
        "exit_code": exit_code,
        "exit_meaning": EXIT_MEANING
            .iter()
            .find(|(code, _)| *code == exit_code)
            .map(|(_, meaning)| *meaning),
        "next_steps_with_a_browser_server": steps,
        "docs": docs,
    });
    if !specific {
        answer["note"] = Value::from(
            "This is the general advice for the category. Give the error text as `message` for \
             advice that names your selector, URL or path.",
        );
    }
    answer
}

async fn explain_error(_ctx: Ctx<NoBrowser>, args: Args) -> Result<Output, ToolError> {
    let message = nonempty(&args, "message")?;
    let category = args.choice("category", &ALL_CATEGORIES, "")?;
    let category = Some(category).filter(|category| !category.is_empty());
    if message.is_none() && category.is_none() {
        return Err(ToolError::invalid(
            "message",
            "give the error text, or a category",
        ));
    }
    if message.is_some_and(|text| text.len() > MAX_TEXT_BYTES) {
        return Err(ToolError::invalid(
            "message",
            format!("larger than {MAX_TEXT_BYTES} bytes; give the error line"),
        ));
    }
    json_output(explain(message, category))
}

// ----------------------------------------------------------------------
// check_config
// ----------------------------------------------------------------------

/// `path` if it is a file name inside the folder the server runs in.
fn confine(path: &str) -> Result<PathBuf, ToolError> {
    let candidate = Path::new(path);
    let stays_inside = !candidate.is_absolute()
        && candidate
            .components()
            .all(|part| matches!(part, Component::Normal(_) | Component::CurDir));
    let refused = || {
        ToolError::Refused(format!(
            "the settings path {path:?} must be a file name inside the folder the server runs \
             in, without `..` and without a link out of it"
        ))
    };
    if !stays_inside {
        return Err(refused());
    }
    // A link that leads out of the folder is no better than `..`.
    if let (Ok(real), Ok(here)) = (
        candidate.canonicalize(),
        std::env::current_dir().and_then(|dir| dir.canonicalize()),
    ) {
        if !real.starts_with(&here) {
            return Err(refused());
        }
    }
    Ok(candidate.to_path_buf())
}

/// `message` with every `string "..."` of a type error cut to `string "…"`: the
/// parser repeats the value it refused, which may be a secret.
fn hide_string_values(message: &str) -> String {
    let mut shown = String::new();
    let mut rest = message;
    while let Some(at) = rest.find("string \"") {
        let start = at + "string \"".len();
        shown.push_str(&rest[..start]);
        match rest[start..].find('"') {
            Some(length) => {
                shown.push('…');
                rest = &rest[start + length..];
            }
            None => return shown + "…",
        }
    }
    shown + rest
}

/// An error message without the lines that quote the file, and without the
/// value a type error repeats: the TOML parser shows the offending line, and
/// the file is not the caller's to read.
fn without_quoted_source(message: &str) -> String {
    let lines: Vec<&str> = message
        .lines()
        .filter(|line| {
            let after_number = line
                .trim_start()
                .trim_start_matches(|c: char| c.is_ascii_digit());
            !after_number.trim_start().starts_with('|')
        })
        .collect();
    cut(&hide_string_values(&lines.join("; ")), MAX_ECHO)
}

fn problem_json(source: &str, error: &SeleniumBaseError) -> Value {
    json!({
        "source": source,
        "category": error.category(),
        "message": without_quoted_source(&error.to_string()),
        "hint": error.hint(),
    })
}

/// The settings that matter for a launch, with secrets redacted.
fn settings_summary(settings: &Settings) -> Value {
    let shown = |name: &str, value: &Option<String>| {
        value.as_deref().map(|value| redact_setting(name, value))
    };
    let config = settings.to_browser_config();
    json!({
        "browser": settings.browser,
        "mode": settings.mode,
        "headless": settings.headless,
        "webdriver_url": settings
            .webdriver_url
            .as_deref()
            .map(|url| redact_setting("SB_WEBDRIVER_URL", url)),
        "chrome_bin": settings.chrome_bin.as_ref().map(|path| path.display().to_string()),
        "proxy": shown("SB_PROXY", &settings.proxy),
        "proxy_pac_url": shown("SB_PROXY_PAC_URL", &settings.proxy_pac_url),
        "user_data_dir": settings.user_data_dir,
        "extension_dir": settings.extension_dir,
        "locale": settings.locale,
        "user_agent": settings.user_agent,
        "window": settings
            .window_width
            .zip(settings.window_height)
            .map(|(width, height)| format!("{width}x{height}")),
        "default_timeout_seconds": settings.timeout_seconds,
        "remote_webdriver_timeout_seconds": settings.remote_webdriver_timeout_seconds,
        "ad_block": settings.ad_block,
        "mobile": settings.mobile,
        "threads": settings.threads,
        "reuse_session": settings.reuse_session,
        "reuse_class_session": settings.reuse_class_session,
        "esc_end": settings.esc_end,
        "rec_sb_cdp": settings.rec_sb_cdp,
        "would_launch": {
            "browser": format!("{:?}", config.browser).to_lowercase(),
            "mode": format!("{:?}", config.mode).to_lowercase(),
            "auto_start_driver": config.auto_start_driver,
            "reuse_session": config.reuse_session,
        },
    })
}

/// The runtime variables (`SB_LOG_LEVEL` and the rest) as they were read.
fn runtime_summary(runtime: &RuntimeConfig) -> Value {
    json!({
        "webdriver_url": redact_setting("SB_WEBDRIVER_URL", &runtime.webdriver_url),
        "chrome_bin": runtime.chrome_bin.as_ref().map(|path| path.display().to_string()),
        "patch_cache_dir": runtime
            .patch_cache_dir
            .as_ref()
            .map(|path| path.display().to_string()),
        "log_level": runtime.log_level,
        "log_format": format!("{:?}", runtime.log_format).to_lowercase(),
        "shutdown_timeout_seconds": runtime.shutdown_timeout.as_secs(),
        "chromedriver_port": runtime.chromedriver_port,
        "implicit_wait_seconds": runtime.implicit_wait.as_secs(),
        "remote_webdriver_timeout_seconds": runtime
            .remote_webdriver_timeout
            .map(|timeout| timeout.as_secs()),
    })
}

/// The report of `check_config` for the settings file `path` (the file of the
/// current folder when `None`) and the variables `var` gives.
pub(super) fn config_report(
    path: Option<&Path>,
    var: &dyn Fn(&str) -> Option<String>,
    server_plugins: &PluginSelection,
) -> Value {
    let check = Settings::check_with(path, var);
    let (runtime, runtime_problems) = RuntimeConfig::from_vars_lenient(var);
    let plugin_check = plugin_cmd::check_selection(&check.settings.plugins);

    // The plugin problem is reported once, with the plugin selection.
    let problems: Vec<Value> = check
        .problems
        .iter()
        .filter(|problem| problem.source != "plugins")
        .map(|problem| problem_json(&problem.source, &problem.error))
        .collect();
    let warnings: Vec<Value> = runtime_problems
        .iter()
        .map(|error| {
            json!({
                "message": without_quoted_source(&error.to_string()),
                "effect": "the variable is ignored and its default is used",
            })
        })
        .collect();
    let ok = problems.is_empty() && plugin_check.is_ok();
    json!({
        "ok": ok,
        "starts_nothing": true,
        "settings_file": check.file.as_ref().map(|file| file.display().to_string()),
        "problems": problems,
        "warnings": warnings,
        "settings": settings_summary(&check.settings),
        "runtime": runtime_summary(&runtime),
        "plugins": {
            "enabled": check.settings.plugins.enabled(),
            "check": match &plugin_check {
                Ok(()) => Value::from("ok"),
                Err(error) => Value::from(without_quoted_source(&error.to_string())),
            },
            "chosen_by_the_server_operator": server_plugins.enabled(),
        },
    })
}

async fn check_config(ctx: Ctx<NoBrowser>, args: Args) -> Result<Output, ToolError> {
    let path = nonempty(&args, "path")?.map(confine).transpose()?;
    json_output(config_report(
        path.as_deref(),
        &|name| std::env::var(name).ok(),
        ctx.settings().plugin_selection(),
    ))
}

// ----------------------------------------------------------------------
// check_fingerprint
// ----------------------------------------------------------------------

/// What a fingerprint says about itself, in a few fields.
fn fingerprint_summary(fingerprint: &Fingerprint) -> Value {
    json!({
        "os": format!("{:?}", fingerprint.os_type()),
        "browser": format!("{:?}", fingerprint.browser_type()),
        "user_agent": fingerprint.user_agent().value(),
        "platform": fingerprint.platform().value(),
        "screen": fingerprint
            .screen()
            .value()
            .map(|size| format!("{}x{}", size.width, size.height)),
    })
}

/// Whether a JSON object looks like the payload of an anti-detect browser tool.
fn looks_like_profile_payload(value: &Value) -> bool {
    value.as_object().is_some_and(|object| {
        object.contains_key("parameters")
            || (object.contains_key("browser_type") && object.contains_key("os_type"))
    })
}

/// The report of `check_fingerprint` for the JSON text `text`.
pub(super) fn fingerprint_report(text: &str) -> Result<Value, ToolError> {
    let value: Value = serde_json::from_str(text)
        .map_err(|error| ToolError::invalid("json", format!("not valid JSON: {error}")))?;
    let as_fingerprint = serde_json::from_value::<Fingerprint>(value.clone());
    let (format, fingerprint) = match as_fingerprint {
        Ok(fingerprint) => ("fingerprint", fingerprint),
        Err(fingerprint_error) => {
            let payload = looks_like_profile_payload(&value)
                .then(|| serde_json::from_value::<ProfileParams>(value.clone()));
            match payload {
                Some(Ok(params)) => ("profile_payload", params.to_fingerprint()),
                other => {
                    let payload_error = match other {
                        Some(Err(error)) => Some(error.to_string()),
                        _ => None,
                    };
                    return Ok(json!({
                        "parsed": false,
                        "as_fingerprint": cut(&fingerprint_error.to_string(), MAX_ECHO),
                        "as_profile_payload": payload_error.map_or_else(
                            || Value::from("not tried: the document has neither `parameters` nor `browser_type` with `os_type`"),
                            |error| Value::from(cut(&error, MAX_ECHO)),
                        ),
                        "advice": "A fingerprint is the JSON of Fingerprint (see build_fingerprint on the sb server for one); a profile payload is the body of an anti-detect browser tool's profile-create call.",
                    }));
                }
            }
        }
    };
    let mut report = coherence_report(&fingerprint);
    report["parsed"] = Value::Bool(true);
    report["format"] = Value::from(format);
    report["fingerprint"] = fingerprint_summary(&fingerprint);
    Ok(report)
}

async fn check_fingerprint(_ctx: Ctx<NoBrowser>, args: Args) -> Result<Output, ToolError> {
    let text = args.str("json")?;
    if text.len() > MAX_TEXT_BYTES {
        return Err(ToolError::invalid(
            "json",
            format!("larger than {MAX_TEXT_BYTES} bytes"),
        ));
    }
    json_output(fingerprint_report(text)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::Settings as ServerSettings;

    async fn call(tool: &str, arguments: Value) -> Result<Output, ToolError> {
        super::super::host(ServerSettings::new("out"))
            .call(tool, arguments.as_object().cloned().unwrap())
            .await
    }

    // ------------------------------------------------------------------
    // explain_error
    // ------------------------------------------------------------------

    #[test]
    fn every_category_of_the_library_is_in_both_tables_and_writes_the_prefix_listed() {
        let mut seen = Vec::new();
        for (category, prefix) in CATEGORIES {
            let error = rebuild(category, placeholder(category))
                .unwrap_or_else(|| panic!("{category} cannot be rebuilt"));
            assert_eq!(error.category(), category);
            assert!(
                error.to_string().starts_with(prefix),
                "{category}: {error:?} does not start with {prefix:?}"
            );
            seen.push(category);
        }
        let mut listed: Vec<&str> = ALL_CATEGORIES[..32].to_vec();
        listed.sort_unstable();
        seen.sort_unstable();
        assert_eq!(listed, seen);
        let kinds: Vec<&str> = TOOL_KINDS.iter().map(|(kind, _, _)| *kind).collect();
        assert_eq!(kinds, ALL_CATEGORIES[32..]);
        assert!(rebuild("not_a_category", "x").is_none());
    }

    #[test]
    fn every_exit_code_of_every_category_has_a_meaning() {
        for (category, _) in CATEGORIES {
            let code = rebuild(category, placeholder(category))
                .unwrap()
                .exit_code();
            assert!(
                EXIT_MEANING.iter().any(|(known, _)| *known == code),
                "{category}: {code}"
            );
        }
    }

    #[test]
    fn a_message_is_told_from_its_text_even_inside_a_failure_report() {
        for (message, category) in [
            (
                "element not found: #buy (strategy: css selector)",
                "element_not_found",
            ),
            (
                "click_element failed: wait timeout: visible #go (after 5000ms)",
                "wait_timeout",
            ),
            (
                "error: invalid configuration: SB_THREADS: invalid digit",
                "invalid_config",
            ),
            (
                "navigation to 'https://x.test' failed: dns error",
                "navigation",
            ),
            ("webdriver command failed: session not created", "webdriver"),
            (
                "no browser session is running; call start_browser first",
                "no_browser",
            ),
            (
                "invalid argument 'selector': this argument is required",
                "invalid_argument",
            ),
            ("something else entirely", ""),
        ] {
            let found = classify(message).map_or("", |(category, _)| category);
            assert_eq!(found, category, "{message}");
        }
    }

    #[test]
    fn the_hint_names_what_the_message_names() {
        let answer = explain(
            Some("element not found: lnk=Home (strategy: css selector)"),
            None,
        );

        assert_eq!(answer["recognized"], true);
        assert_eq!(answer["category"], "element_not_found");
        assert_eq!(answer["specific"], true);
        let hint = answer["hint"].as_str().unwrap();
        assert!(hint.contains("`lnk=Home`"), "{hint}");
        assert!(
            hint.contains("link="),
            "a mistyped prefix is pointed out: {hint}"
        );
        assert_eq!(answer["exit_code"], 3);
        assert!(answer["exit_meaning"].as_str().unwrap().contains("element"));
        assert_eq!(answer["transient"], false);
        let docs = answer["docs"].as_array().unwrap();
        assert_eq!(
            docs[0]["heading"],
            "An element is not found or a wait times out"
        );
        assert!(docs[0]["text"]
            .as_str()
            .unwrap()
            .contains("Check the selector"));
        assert!(!docs[0]["text"].as_str().unwrap().contains("<Step"));
    }

    #[test]
    fn a_category_alone_gives_the_general_advice_and_says_so() {
        let answer = explain(None, Some("browser_launch"));

        assert_eq!(answer["category"], "browser_launch");
        assert_eq!(answer["from"], "category");
        assert_eq!(answer["specific"], false);
        assert_eq!(answer["exit_code"], 4);
        assert!(answer["note"].as_str().unwrap().contains("general advice"));
        assert!(answer["hint"].as_str().unwrap().contains("sbrs doctor"));
    }

    #[test]
    fn transient_errors_are_marked_and_each_message_shape_is_rebuilt() {
        let network = explain(
            Some("network request to 'https://x.test/a' failed (503): Service Unavailable"),
            None,
        );
        assert_eq!(network["transient"], true);
        assert_eq!(network["exit_code"], 5);
        let hint = network["hint"].as_str().unwrap();
        assert!(
            hint.contains("https://x.test/a") && hint.contains("Retry"),
            "{network}"
        );
        assert_eq!(
            network["docs"],
            json!([]),
            "no section of the page is about it"
        );

        let io = explain(
            Some("I/O error: No such file or directory (os error 2)"),
            None,
        );
        assert_eq!(io["exit_code"], 6);

        let json_error = explain(Some("JSON error: expected value at line 3 column 7"), None);
        assert!(
            json_error["hint"]
                .as_str()
                .unwrap()
                .contains("line 3, column 7"),
            "{json_error}"
        );

        let driver = explain(
            Some("webdriver command failed: session not created: only supports Chrome 150"),
            None,
        );
        assert!(
            driver["hint"].as_str().unwrap().contains("sbrs install"),
            "{driver}"
        );
    }

    #[test]
    fn the_servers_own_errors_are_explained_as_the_servers() {
        let answer = explain(
            Some("no browser session is running; call start_browser first"),
            None,
        );

        assert_eq!(answer["category"], "no_browser");
        assert_eq!(answer["origin"], "the MCP server, not the library");
        assert!(answer["hint"].as_str().unwrap().contains("start_browser"));
    }

    #[test]
    fn text_that_is_no_error_of_ours_is_not_guessed_at() {
        let answer = explain(Some("segmentation fault in libfoo"), None);

        assert_eq!(answer["recognized"], false);
        assert!(answer["known_categories"].as_array().unwrap().len() > 30);
        assert!(answer.get("category").is_none());
    }

    #[test]
    fn a_given_category_wins_over_the_text_it_is_given_with() {
        let answer = explain(Some("wait timeout: slow page"), Some("wait_timeout"));
        assert_eq!(answer["category"], "wait_timeout");
        assert_eq!(answer["specific"], true);

        // The text is of another category: the caller's choice stands.
        let answer = explain(
            Some("element not found: #x (strategy: css selector)"),
            Some("stealth"),
        );
        assert_eq!(answer["category"], "stealth");
    }

    #[test]
    fn json_errors_are_rebuilt_at_the_place_the_message_names() {
        for (line, column) in [(1, 1), (3, 7), (12, 40)] {
            let error = json_error(&format!("expected value at line {line} column {column}"));
            assert_eq!((error.line(), error.column()), (line, column));
        }
    }

    #[test]
    fn the_page_components_are_removed_from_the_docs() {
        let text = plain(
            "Intro.\n\n<Steps>\n\n<Step title=\"Do it\">\n\nBody `x`.\n\n</Step>\n\n</Steps>\n\n```\n<Not a tag>\n```\n",
        );
        assert_eq!(
            text,
            "Intro.\n\n- Do it\n\nBody `x`.\n\n```\n<Not a tag>\n```"
        );
    }

    #[test]
    fn every_heading_the_tool_cites_exists_on_the_page() {
        for (category, _) in CATEGORIES {
            for heading in docs_headings(category) {
                assert!(docs_section(heading).is_some(), "{category}: {heading}");
            }
        }
        for (kind, _, _) in TOOL_KINDS {
            for heading in docs_headings(kind) {
                assert!(docs_section(heading).is_some(), "{kind}: {heading}");
            }
        }
        assert!(docs_section("Reading an error").is_some());
    }

    #[tokio::test]
    async fn the_tool_wants_a_message_or_a_category_and_checks_the_category() {
        let nothing = call("explain_error", json!({})).await.unwrap_err();
        assert!(
            matches!(nothing, ToolError::InvalidArgument { .. }),
            "{nothing}"
        );
        let typo = call("explain_error", json!({ "category": "elemnt_not_found" }))
            .await
            .unwrap_err();
        assert!(
            typo.to_string()
                .contains("did you mean \"element_not_found\""),
            "{typo}"
        );

        let answer = call(
            "explain_error",
            json!({ "message": "stale element reference: #row" }),
        )
        .await
        .unwrap();
        assert_eq!(answer.as_json().unwrap()["category"], "stale_element");
    }

    // ------------------------------------------------------------------
    // check_config
    // ------------------------------------------------------------------

    fn vars(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    fn write_settings(contents: &str) -> tempfile::NamedTempFile {
        use std::io::Write as _;
        let mut file = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
        file.write_all(contents.as_bytes()).unwrap();
        file
    }

    #[test]
    fn good_settings_are_summarised_and_start_nothing() {
        let file = write_settings(
            "browser = \"edge\"\nheadless = true\nproxy = \"user:pw@proxy.test:8080\"\n\
             window_width = 1280\nwindow_height = 720\n",
        );

        let report = config_report(
            Some(file.path()),
            &vars(&[("SB_TIMEOUT", "7"), ("SB_LOG_LEVEL", "debug")]),
            &PluginSelection::new(),
        );

        assert_eq!(report["ok"], true, "{report}");
        assert_eq!(report["starts_nothing"], true);
        assert_eq!(report["problems"], json!([]));
        assert_eq!(report["settings"]["browser"], "edge");
        assert_eq!(report["settings"]["would_launch"]["browser"], "edge");
        assert_eq!(report["settings"]["window"], "1280x720");
        assert_eq!(report["settings"]["default_timeout_seconds"], 7);
        assert_eq!(report["runtime"]["log_level"], "debug");
        assert_eq!(report["plugins"]["check"], "ok");
        // The proxy password is not repeated.
        assert_eq!(report["settings"]["proxy"], "[redacted]@proxy.test:8080");
        assert!(!report.to_string().contains("pw@"), "{report}");
    }

    #[test]
    fn every_problem_is_reported_with_where_it_came_from_and_the_good_values_are_kept() {
        let file = write_settings("locale = \"de-DE\"\nwindow_width = 800\n");

        let report = config_report(
            Some(file.path()),
            &vars(&[
                ("SB_HEADLESS", "maybe"),
                ("SB_THREADS", "many"),
                ("SB_SHUTDOWN_TIMEOUT_SECS", "soon"),
                ("SB_MOBILE", "on"),
            ]),
            &PluginSelection::new(),
        );

        assert_eq!(report["ok"], false);
        let sources: Vec<&str> = report["problems"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["source"].as_str().unwrap())
            .collect();
        assert_eq!(sources, ["SB_HEADLESS", "SB_THREADS", "validation"]);
        assert!(report["problems"][0]["message"]
            .as_str()
            .unwrap()
            .contains("maybe"));
        assert!(report["problems"][2]["message"]
            .as_str()
            .unwrap()
            .contains("window_width"));
        assert!(report["problems"][0]["hint"].is_string());
        // A runtime variable that does not parse is a warning: it is ignored.
        let warnings = report["warnings"].as_array().unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0]["message"]
            .as_str()
            .unwrap()
            .contains("SB_SHUTDOWN_TIMEOUT_SECS"));
        assert_eq!(report["settings"]["mobile"], true);
        assert_eq!(report["settings"]["locale"], "de-DE");
        assert_eq!(report["runtime"]["shutdown_timeout_seconds"], 30);
    }

    #[test]
    fn a_file_that_does_not_parse_is_reported_without_quoting_its_lines() {
        let file = write_settings("headless = \"hunter2-secret\"\n");

        let report = config_report(Some(file.path()), &vars(&[]), &PluginSelection::new());

        assert_eq!(report["ok"], false);
        let message = report["problems"][0]["message"].as_str().unwrap();
        assert!(
            message.contains("failed to parse TOML settings"),
            "{message}"
        );
        assert!(
            !message.contains("headless = "),
            "the source line is quoted: {message}"
        );
        assert!(!message.contains('|'), "{message}");
        assert!(
            !message.contains("hunter2"),
            "the refused value is repeated: {message}"
        );
        assert_eq!(
            report["problems"][0]["source"],
            file.path().display().to_string()
        );
    }

    #[test]
    fn an_unknown_plugin_is_reported_with_the_plugin_check() {
        let file = write_settings("[plugins]\nenable = [\"no-such-plugin\"]\n");

        let report = config_report(Some(file.path()), &vars(&[]), &PluginSelection::new());

        assert_eq!(report["ok"], false);
        assert_eq!(report["plugins"]["enabled"], json!(["no-such-plugin"]));
        let check = report["plugins"]["check"].as_str().unwrap();
        assert!(
            check.contains("no-such-plugin") || check.contains("plugins-builtin"),
            "{check}"
        );
        // It is said once, under plugins, not again under problems.
        assert_eq!(report["problems"], json!([]));
    }

    #[test]
    fn the_operators_plugins_are_shown() {
        let file = write_settings("");
        let chosen = PluginSelection::new().enable("shield");

        let report = config_report(Some(file.path()), &vars(&[]), &chosen);

        assert_eq!(
            report["plugins"]["chosen_by_the_server_operator"],
            json!(["shield"])
        );
    }

    #[test]
    fn the_settings_path_stays_inside_the_folder() {
        for path in ["/etc/passwd", "../x.toml", "a/../../x.toml", "C:\\x.toml"] {
            if path.starts_with("C:") && cfg!(not(windows)) {
                continue;
            }
            assert!(
                matches!(confine(path), Err(ToolError::Refused(_))),
                "{path} should be refused"
            );
        }
        assert_eq!(
            confine("sbase_config.toml").unwrap(),
            PathBuf::from("sbase_config.toml")
        );
        assert!(confine("./conf/x.json").is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn a_link_out_of_the_folder_is_refused() {
        let link = Path::new(env!("CARGO_MANIFEST_DIR")).join("target-test-settings-link.toml");
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink("/etc/hosts", &link).unwrap();
        let name = link.file_name().unwrap().to_str().unwrap().to_owned();
        let previous = std::env::current_dir().unwrap();
        std::env::set_current_dir(env!("CARGO_MANIFEST_DIR")).unwrap();

        let outcome = confine(&name);

        std::env::set_current_dir(previous).unwrap();
        std::fs::remove_file(&link).unwrap();
        assert!(matches!(outcome, Err(ToolError::Refused(_))), "{outcome:?}");
    }

    #[tokio::test]
    async fn the_tool_refuses_a_path_outside_the_folder_before_reading_anything() {
        let error = call("check_config", json!({ "path": "/etc/hosts" }))
            .await
            .unwrap_err();
        assert!(matches!(error, ToolError::Refused(_)), "{error}");
    }

    #[tokio::test]
    async fn a_settings_file_that_is_not_there_is_a_problem_not_a_tool_error() {
        let answer = call(
            "check_config",
            json!({ "path": "no-such-settings-file.toml" }),
        )
        .await
        .unwrap();

        let report = answer.as_json().unwrap();
        assert_eq!(report["ok"], false);
        assert_eq!(report["settings_file"], "no-such-settings-file.toml");
        assert!(report["problems"][0]["message"]
            .as_str()
            .unwrap()
            .contains("failed to read"));
    }

    // ------------------------------------------------------------------
    // check_fingerprint
    // ------------------------------------------------------------------

    #[test]
    fn a_preset_fingerprint_parses_and_is_coherent() {
        let text = serde_json::to_string(&Fingerprint::windows_desktop()).unwrap();

        let report = fingerprint_report(&text).unwrap();

        assert_eq!(report["parsed"], true);
        assert_eq!(report["format"], "fingerprint");
        assert_eq!(report["coherent"], true, "{report}");
        assert_eq!(report["errors"], json!([]));
        assert_eq!(report["fingerprint"]["os"], "Windows");
        assert!(report["fingerprint"]["user_agent"]
            .as_str()
            .unwrap()
            .contains("Windows"));
    }

    #[test]
    fn the_report_is_the_one_validate_fingerprint_gives() {
        let fingerprint = Fingerprint::macos_desktop();
        let text = serde_json::to_string(&fingerprint).unwrap();

        let report = fingerprint_report(&text).unwrap();

        let shared = coherence_report(&fingerprint);
        for key in ["coherent", "errors", "warnings"] {
            assert_eq!(report[key], shared[key], "{key}");
        }
    }

    #[test]
    fn a_contradiction_is_found() {
        let fingerprint = Fingerprint::windows_desktop()
            .to_builder()
            .platform("MacIntel")
            .build()
            .unwrap();
        let text = serde_json::to_string(&fingerprint).unwrap();

        let report = fingerprint_report(&text).unwrap();

        assert_eq!(report["coherent"], false);
        assert!(
            report["errors"][0].as_str().unwrap().contains("MacIntel"),
            "{report}"
        );
    }

    #[test]
    fn a_profile_payload_is_converted_and_checked() {
        let payload = json!({
            "name": "Windows Chromium",
            "browser_type": "chromium",
            "os_type": "windows",
            "folder_id": "work",
            "parameters": { "fingerprint": {
                "navigator": {
                    "user_agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36",
                    "platform": "Win32",
                    "hardware_concurrency": 8
                },
                "screen": { "width": 1920, "height": 1200, "pixel_ratio": 1 }
            } }
        });

        let report = fingerprint_report(&payload.to_string()).unwrap();

        assert_eq!(report["parsed"], true);
        assert_eq!(report["format"], "profile_payload");
        assert_eq!(
            report["coherent"], false,
            "a Mac agent on a Windows persona: {report}"
        );
    }

    #[test]
    fn a_document_that_is_neither_is_described_not_guessed_at() {
        let report = fingerprint_report(r#"{"user_agent": "x", "unknown_field": 1}"#).unwrap();

        assert_eq!(report["parsed"], false);
        assert!(
            report["as_fingerprint"]
                .as_str()
                .unwrap()
                .contains("unknown"),
            "{report}"
        );
        assert!(report["as_profile_payload"]
            .as_str()
            .unwrap()
            .contains("not tried"));
        assert!(report.get("coherent").is_none());
    }

    #[test]
    fn text_that_is_not_json_names_the_argument() {
        let error = fingerprint_report("{not json").unwrap_err();
        assert!(
            matches!(&error, ToolError::InvalidArgument { name, .. } if name == "json"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn a_huge_document_is_refused() {
        let error = call(
            "check_fingerprint",
            json!({ "json": "x".repeat(MAX_TEXT_BYTES + 1) }),
        )
        .await
        .unwrap_err();
        assert!(
            matches!(error, ToolError::InvalidArgument { .. }),
            "{error}"
        );
    }
}
