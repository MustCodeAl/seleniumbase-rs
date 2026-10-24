//! Fingerprint tools: what the running browser shows a page, how that
//! compares with a saved fingerprint or a preset, and what a detection
//! script would find in it.
//!
//! - `capture_fingerprint` reads the browser as a page sees it (the same
//!   script and mapping the `sbrs fingerprint capture` command uses) and can
//!   save the result as a fingerprint file.
//! - `compare_fingerprint` asks whether the browser presents a fingerprint:
//!   which settings it contradicts, which it matches and which no page can
//!   read.
//! - `explain_fingerprint` lists the signs of automation a detection script
//!   would find, how coherent the readings are and what such a script reads.
//! - [`offline_tools`] (`diff_fingerprints`, `patch_fingerprint`) and the
//!   helpers [`build`] and [`fingerprint_arg`] need no browser; the `sb`
//!   server's stealth tools use them.
//! - `start_browser` takes a fingerprint to apply through [`start_fingerprint`].
//!
//! A fingerprint argument is `fingerprint` (a JSON object, or a string holding
//! one), `path` (a file in the output folder or the working directory) or
//! `preset`; exactly one. The tools that return a fingerprint show a proxy
//! password as `***`; a file written with `save_to` keeps the real one and is
//! private to its owner.
//!
//! What the capture tools return comes from the page (its address, and what it
//! lets the browser report), so they are marked untrusted.

use std::fmt::Write as _;
use std::future::Future;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use super::stealth::coherence_report;
use super::support::nonempty;
use super::{Args, Ctx, Effect, Output, Prop, Schema, Settings, ToolDef, ToolError};
use crate::api::base_case::BaseCase;
use crate::cli::fingerprint_cmd::{self, ExportSource, PRESET_NAMES};
use crate::error::SeleniumBaseError;
use crate::stealth::capture::CapturedFingerprint;
use crate::stealth::fingerprint::{Fingerprint, FingerprintComparison, OsType};

/// The systems a random identity can claim.
const OS_NAMES: [&str; 5] = ["windows", "macos", "linux", "android", "ios"];

/// The settings no page can read, so a capture never has them.
const NOT_READABLE: [&str; 5] = [
    "accept_languages",
    "geolocation",
    "fonts",
    "proxy",
    "webrtc",
];

/// The settings of a fingerprint that a detection script reads first.
const VIEW_SETTINGS: [&str; 14] = [
    "user_agent",
    "platform",
    "hardware_concurrency",
    "device_memory",
    "max_touch_points",
    "vendor",
    "locale",
    "languages",
    "timezone",
    "screen",
    "pixel_ratio",
    "color_depth",
    "webgl",
    "client_hints",
];

/// The readings of a capture that no setting holds, but a script sees.
const VIEW_READINGS: [&str; 6] = [
    "webdriver",
    "plugins_length",
    "secure_context",
    "canvas_hash",
    "audio_hash",
    "webgl_info",
];

/// The signs of automation the capture script looks for: the name, how loudly
/// a detector reacts, what it concludes, and what to change.
const MARKERS: [(&str, &str, &str, &str); 9] = [
    (
        "user_agent_says_headless",
        "high",
        "the user agent says HeadlessChrome, the first thing a detector checks",
        "run headed (headless false), or start the browser with a preset or fingerprint whose user agent is an ordinary Chrome's",
    ),
    (
        "navigator_webdriver_true",
        "high",
        "navigator.webdriver is true, the flag automation sets",
        "use the cdp server, which has no WebDriver, or uc true (undetected mode) on the driver and sb servers",
    ),
    (
        "no_window_chrome",
        "medium",
        "window.chrome is missing in a Chromium browser, as in headless builds",
        "run headed, with a full Chrome rather than a headless shell",
    ),
    (
        "no_plugins",
        "medium",
        "navigator.plugins is empty in a Chromium browser, as in headless mode",
        "run headed: a real Chrome lists its PDF viewer plugins",
    ),
    (
        "no_languages",
        "medium",
        "navigator.languages is empty, and a real browser always has one",
        "start the browser with a fingerprint that sets locale and languages",
    ),
    (
        "zero_outer_size",
        "medium",
        "window.outerWidth and outerHeight are 0, as in a window with no frame",
        "run headed, or start the browser with a fingerprint that sets a screen",
    ),
    (
        "notification_permission_mismatch",
        "high",
        "Notification.permission says denied while the permissions API says prompt, the classic headless contradiction",
        "run headed: headless Chrome gives these two answers",
    ),
    (
        "software_renderer",
        "low",
        "WebGL renders in software (SwiftShader, llvmpipe), as on a machine with no GPU",
        "run on a machine with a GPU, or start the browser with a fingerprint that sets a WebGL vendor and renderer",
    ),
    (
        "zero_rtt",
        "low",
        "navigator.connection.rtt is 0, which a real connection never reports",
        "start the browser with a fingerprint that sets a connection profile",
    ),
];

/// A browser session that can be asked what it shows a page.
pub(super) trait CaptureSession: Send + Sync + 'static {
    /// Opens `url` in the tab in use.
    fn navigate_to(
        &mut self,
        url: &str,
    ) -> impl Future<Output = Result<(), SeleniumBaseError>> + Send;

    /// Reads what the tab in use shows a page.
    fn capture_now(
        &mut self,
    ) -> impl Future<Output = Result<CapturedFingerprint, SeleniumBaseError>> + Send;
}

impl CaptureSession for BaseCase {
    async fn navigate_to(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        self.open(url).await?;
        Ok(())
    }

    async fn capture_now(&mut self) -> Result<CapturedFingerprint, SeleniumBaseError> {
        self.capture_fingerprint().await
    }
}

// ----------------------------------------------------------------------
// Arguments
// ----------------------------------------------------------------------

/// The argument names one fingerprint source goes by.
#[derive(Clone, Copy)]
struct Names {
    inline: &'static str,
    path: &'static str,
    preset: &'static str,
}

/// The names the tools that take a fingerprint use.
const SOURCE: Names = Names {
    inline: "fingerprint",
    path: "path",
    preset: "preset",
};

/// The names `start_browser` uses, which has other arguments called `path`.
const LAUNCH: Names = Names {
    inline: "fingerprint",
    path: "fingerprint_path",
    preset: "fingerprint_preset",
};

/// The argument that holds a fingerprint written out.
pub(super) fn inline_prop() -> Prop {
    Prop::object(
        "A fingerprint as a JSON object, in the v2 file format (a string holding the JSON is \
         accepted too)",
    )
}

fn path_prop() -> Prop {
    Prop::string("Path of a saved fingerprint file, in the output folder or the working directory")
}

/// The argument that names a built-in fingerprint.
pub(super) fn preset_prop() -> Prop {
    Prop::choice(&PRESET_NAMES, "A built-in fingerprint preset")
}

/// `start_browser`'s argument for a fingerprint written out.
pub(super) fn launch_inline_prop() -> Prop {
    Prop::object(
        "A fingerprint to apply to every tab, as a JSON object in the v2 file format; not with \
         fingerprint_path or fingerprint_preset. Applied in undetected mode (uc, the default) on \
         the driver and sb servers; Chromium browsers only",
    )
}

/// `start_browser`'s argument for a fingerprint file.
pub(super) fn launch_path_prop() -> Prop {
    Prop::string(
        "Path of a saved fingerprint file to apply (in the output folder or the working \
         directory); not with fingerprint or fingerprint_preset",
    )
}

/// `start_browser`'s argument for a built-in fingerprint.
pub(super) fn launch_preset_prop() -> Prop {
    Prop::choice(
        &PRESET_NAMES,
        "A built-in fingerprint to apply; not with fingerprint or fingerprint_path",
    )
}

/// The arguments of a tool that takes a fingerprint: `fingerprint`, `path` or
/// `preset`.
pub(super) fn source_schema() -> Schema {
    Schema::new()
        .optional("fingerprint", inline_prop())
        .optional("path", path_prop())
        .optional("preset", preset_prop())
}

/// The fingerprint the call names, which must name one.
///
/// # Errors
///
/// [`ToolError::InvalidArgument`] when none or more than one of `fingerprint`,
/// `path` and `preset` is given, when the fingerprint is not valid (the error
/// names the setting), or when the file is too large or cannot be read;
/// [`ToolError::Refused`] when `path` is outside the output folder and the
/// working directory.
pub(super) fn fingerprint_arg(args: &Args, settings: &Settings) -> Result<Fingerprint, ToolError> {
    resolve(args, settings, SOURCE)?.ok_or_else(|| {
        ToolError::invalid(
            SOURCE.inline,
            "give exactly one of fingerprint, path and preset",
        )
    })
}

/// The fingerprint `start_browser` is asked to apply, if it is asked to apply one.
///
/// # Errors
///
/// As [`fingerprint_arg`], with the names `fingerprint`, `fingerprint_path`
/// and `fingerprint_preset`.
pub(super) fn start_fingerprint(
    args: &Args,
    settings: &Settings,
) -> Result<Option<Fingerprint>, ToolError> {
    resolve(args, settings, LAUNCH)
}

/// What `start_browser` adds to its answer for a fingerprint it applied.
pub(super) fn applied(fingerprint: Option<&Fingerprint>) -> String {
    fingerprint.map_or_else(String::new, |fingerprint| {
        let report = fingerprint_cmd::check_report("fingerprint", fingerprint);
        let first = report.text.lines().next().unwrap_or_default();
        format!(
            " Fingerprint applied: {}.",
            first.strip_prefix("fingerprint: ").unwrap_or(first)
        )
    })
}

/// What `start_browser` adds to its answer when a browser was already running,
/// so a fingerprint could not be applied.
pub(super) fn ignored(fingerprint: Option<&Fingerprint>) -> String {
    fingerprint.map_or_else(String::new, |_| {
        " The fingerprint was not applied: it is set as the browser starts. Call close_browser, \
         then start_browser again."
            .to_owned()
    })
}

fn is_given(value: &Value) -> bool {
    !matches!(value, Value::String(text) if text.trim().is_empty())
}

fn resolve(
    args: &Args,
    settings: &Settings,
    names: Names,
) -> Result<Option<Fingerprint>, ToolError> {
    let inline = args.value(names.inline).filter(|value| is_given(value));
    let path = nonempty(args, names.path)?;
    let preset = nonempty(args, names.preset)?;
    let given: Vec<&str> = [
        (inline.is_some(), names.inline),
        (path.is_some(), names.path),
        (preset.is_some(), names.preset),
    ]
    .into_iter()
    .filter_map(|(given, name)| given.then_some(name))
    .collect();
    if let [first, second, ..] = given.as_slice() {
        return Err(ToolError::invalid(
            second,
            format!(
                "give only one of {}, {} and {}; {first} is given too",
                names.inline, names.path, names.preset
            ),
        ));
    }
    if let Some(value) = inline {
        return from_inline(names.inline, value).map(Some);
    }
    if let Some(requested) = path {
        let real = readable_path(names.path, requested, settings)?;
        return Fingerprint::load(real)
            .map_err(|error| config_error(names.path, error))
            .map(Some);
    }
    preset.map(|name| named(names.preset, name)).transpose()
}

/// A configuration error blames the argument; anything else is a failure.
fn config_error(name: &str, error: SeleniumBaseError) -> ToolError {
    match error {
        SeleniumBaseError::InvalidConfig(reason) => ToolError::invalid(name, reason),
        other => ToolError::Failed(other),
    }
}

fn from_inline(name: &str, value: &Value) -> Result<Fingerprint, ToolError> {
    let text = match value {
        Value::String(text) => text.clone(),
        Value::Object(_) => value.to_string(),
        _ => {
            return Err(ToolError::invalid(
                name,
                "expected a JSON object, or a string holding one",
            ))
        }
    };
    if text.len() as u64 > Fingerprint::MAX_FILE_BYTES {
        return Err(ToolError::invalid(
            name,
            format!(
                "larger than the {} bytes a fingerprint may have",
                Fingerprint::MAX_FILE_BYTES
            ),
        ));
    }
    Fingerprint::from_json(&text).map_err(|error| config_error(name, error))
}

fn named(name: &str, preset: &str) -> Result<Fingerprint, ToolError> {
    fingerprint_cmd::preset(preset).ok_or_else(|| {
        ToolError::invalid(
            name,
            format!(
                "there is no preset called {preset:?}; the presets are {}",
                PRESET_NAMES.join(", ")
            ),
        )
    })
}

/// Where the file called `requested` is, if it is in the output folder or the
/// working directory. A relative name is looked for in the output folder first,
/// where `save_to` writes.
fn readable_path(name: &str, requested: &str, settings: &Settings) -> Result<PathBuf, ToolError> {
    let asked = Path::new(requested);
    let here = std::env::current_dir().map_err(SeleniumBaseError::from)?;
    let candidates = if asked.is_absolute() {
        vec![asked.to_path_buf()]
    } else {
        vec![settings.output_dir().join(asked), here.join(asked)]
    };
    let real = candidates
        .iter()
        .find_map(|candidate| candidate.canonicalize().ok())
        .ok_or_else(|| {
            ToolError::invalid(
                name,
                format!("no file {requested:?} in the output folder or the working directory"),
            )
        })?;
    let allowed = [settings.output_dir().to_path_buf(), here]
        .iter()
        .filter_map(|root| root.canonicalize().ok())
        .any(|root| real.starts_with(root));
    if allowed {
        Ok(real)
    } else {
        Err(ToolError::Refused(format!(
            "{requested:?} is outside the output folder and the working directory"
        )))
    }
}

/// `a` or `b` of `diff_fingerprints`: a preset name, a path, or a fingerprint
/// written out.
fn reference(args: &Args, name: &str, settings: &Settings) -> Result<Fingerprint, ToolError> {
    match args.value(name) {
        None => Err(ToolError::invalid(name, "this argument is required")),
        Some(Value::String(text)) => {
            let text = text.trim();
            if text.starts_with('{') {
                from_inline(name, &Value::String(text.to_owned()))
            } else if let Some(preset) = fingerprint_cmd::preset(text) {
                Ok(preset)
            } else {
                let real = readable_path(name, text, settings)?;
                Fingerprint::load(real).map_err(|error| config_error(name, error))
            }
        }
        Some(other) => from_inline(name, other),
    }
}

fn reference_prop(what: &str) -> Prop {
    Prop::string(&format!(
        "{what}: a preset name, the path of a fingerprint file (output folder or working \
         directory), or the fingerprint as JSON text (a JSON object is accepted too)"
    ))
}

// ----------------------------------------------------------------------
// Output
// ----------------------------------------------------------------------

/// The fingerprint as the file format writes it, with a proxy password shown
/// as `***`.
pub(super) fn redacted(fingerprint: &Fingerprint) -> Value {
    let mut value = fingerprint
        .to_json_pretty()
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .unwrap_or(Value::Null);
    if let Some(password) = value
        .get_mut("proxy")
        .and_then(|proxy| proxy.get_mut("password"))
    {
        if !password.is_null() {
            *password = json!("***");
        }
    }
    value
}

fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

fn object_schema(properties: &Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": properties, "required": required })
}

fn saved_path(settings: &Settings, args: &Args) -> Result<Option<PathBuf>, ToolError> {
    nonempty(args, "save_to")?
        .map(|name| settings.output_path(None, name))
        .transpose()
}

fn save_to_prop() -> Prop {
    Prop::string(
        "File name in the output folder to save the fingerprint to, as JSON; the file is private \
         to its owner",
    )
}

// ----------------------------------------------------------------------
// Browser tools
// ----------------------------------------------------------------------

/// The tools that read the running browser, for a server whose session is `S`.
#[expect(
    clippy::too_many_lines,
    reason = "the tool table reads best as one list"
)]
pub(super) fn tools<S: CaptureSession>() -> Vec<ToolDef<S>> {
    vec![
        ToolDef::new(
            "capture_fingerprint",
            "Capture Fingerprint",
            "Read what the running browser shows a page, as a fingerprint: the navigator, \
             Client Hints, screen, time zone, WebGL, media devices and more, plus readings no \
             setting holds (webdriver flag, plugins, canvas and audio hashes, headless \
             markers). Optionally opens url first and saves the fingerprint as a file. An \
             https page or about:blank gives the whole fingerprint; an http page has no \
             Client Hints or media devices.",
            Effect::Mixed,
            Schema::new()
                .optional("url", Prop::string("Page to open before reading"))
                .optional("save_to", save_to_prop()),
            capture_fingerprint::<S>,
        )
        .with_output_schema(object_schema(
            &json!({
                "fingerprint": { "type": "object", "description": "The v2 fingerprint file" },
                "unmapped": { "type": "object", "description": "Readings no setting holds" },
                "probe": {
                    "type": "object",
                    "properties": {
                        "url": { "type": "string" },
                        "captured_at": { "type": "string" },
                        "browser_version": { "type": "string" }
                    },
                    "required": ["url", "captured_at"]
                },
                "settings_read": { "type": "integer" },
                "saved": { "type": "string" }
            }),
            &["fingerprint", "unmapped", "probe", "settings_read"],
        ))
        .untrusted(),
        ToolDef::new(
            "compare_fingerprint",
            "Compare Fingerprint",
            "Ask whether the running browser presents a fingerprint (give a fingerprint, a path \
             or a preset): captures what the page shows and lists the settings it contradicts, \
             the ones it matches and the ones no page can read (those are never counted as \
             matches). The text starts with presents: yes|no, verified: yes|no.",
            Effect::Observe,
            source_schema(),
            compare_fingerprint::<S>,
        )
        .with_output_schema(object_schema(
            &json!({
                "presents": { "type": "boolean" },
                "verified": { "type": "boolean" },
                "claimed": { "type": "integer" },
                "differences": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": { "setting": { "type": "string" } },
                        "required": ["setting", "expected", "actual"]
                    }
                },
                "matched": { "type": "array", "items": { "type": "string" } },
                "unverifiable": { "type": "array", "items": { "type": "string" } },
                "url": { "type": "string" }
            }),
            &[
                "presents",
                "verified",
                "claimed",
                "differences",
                "matched",
                "unverifiable",
            ],
        ))
        .untrusted(),
        ToolDef::new(
            "explain_fingerprint",
            "Explain Fingerprint",
            "Say what a detection script would find in the running browser: the signs of \
             automation (webdriver flag, headless markers, software renderer ...) with how \
             loudly a detector reacts and what to change, whether the readings agree with each \
             other, and the values a script reads first. Optionally opens url first.",
            Effect::Mixed,
            Schema::new().optional("url", Prop::string("Page to open before reading")),
            explain_fingerprint::<S>,
        )
        .with_output_schema(object_schema(
            &json!({
                "risk": { "type": "string", "enum": ["none", "low", "medium", "high"] },
                "leaks": { "type": "array", "items": { "type": "object" } },
                "coherence": { "type": "object" },
                "detector_view": { "type": "object" },
                "not_readable": { "type": "array", "items": { "type": "string" } },
                "unanswered": { "type": "object" },
                "url": { "type": "string" }
            }),
            &[
                "risk",
                "leaks",
                "coherence",
                "detector_view",
                "not_readable",
            ],
        ))
        .untrusted(),
    ]
}

/// Reads the browser, reporting progress as it goes.
async fn capture<S: CaptureSession>(
    ctx: &Ctx<S>,
    url: Option<&str>,
) -> Result<CapturedFingerprint, ToolError> {
    let progress = ctx.progress();
    let total = if url.is_some() { 3 } else { 2 };
    let mut step = 0;
    let mut session = ctx.session().await?;
    if let Some(url) = url {
        step += 1;
        progress
            .report(step, Some(total), format!("opening {url}"))
            .await;
        session.navigate_to(url).await?;
    }
    step += 1;
    progress
        .report(step, Some(total), "reading what the page shows")
        .await;
    let captured = session.capture_now().await?;
    progress
        .report(step + 1, Some(total), "mapped to a fingerprint")
        .await;
    Ok(captured)
}

async fn capture_fingerprint<S: CaptureSession>(
    ctx: Ctx<S>,
    args: Args,
) -> Result<Output, ToolError> {
    let url = nonempty(&args, "url")?.map(str::to_owned);
    // Refused before the browser is touched.
    let save = saved_path(ctx.settings(), &args)?;
    let captured = capture(&ctx, url.as_deref()).await?;

    let mut data = json!({
        "fingerprint": redacted(&captured.fingerprint),
        "unmapped": captured.unmapped,
        "probe": serde_json::to_value(&captured.probe).map_err(SeleniumBaseError::from)?,
        "settings_read": captured.fingerprint.claimed_settings().len(),
    });
    let mut text = fingerprint_cmd::capture_summary(&captured);
    if let Some(path) = save {
        captured.fingerprint.save(&path)?;
        data["saved"] = json!(path.display().to_string());
        let _ = writeln!(text, "Saved to {}.", path.display());
    }
    text.push_str(&pretty(&data));
    Ok(Output::Structured { text, data })
}

async fn compare_fingerprint<S: CaptureSession>(
    ctx: Ctx<S>,
    args: Args,
) -> Result<Output, ToolError> {
    let expected = fingerprint_arg(&args, ctx.settings())?;
    let captured = capture(&ctx, None).await?;
    let comparison = expected.compare_to(&captured.fingerprint);
    Ok(comparison_output(
        &comparison,
        expected.claimed_settings().len(),
        &captured.probe.url,
    ))
}

/// The answer of `compare_fingerprint`: the verdict first, then the evidence.
fn comparison_output(comparison: &FingerprintComparison, claimed: usize, url: &str) -> Output {
    let yes = |flag: bool| if flag { "yes" } else { "no" };
    let mut text = format!(
        "presents: {}, verified: {}\n",
        yes(comparison.presents()),
        yes(comparison.is_verified())
    );
    if comparison.differences.is_empty() {
        let _ = writeln!(
            text,
            "No setting the page could be read for contradicts the {claimed} the fingerprint gives."
        );
    } else {
        let _ = writeln!(
            text,
            "{} of the {claimed} settings differ:",
            comparison.differences.len()
        );
        for difference in &comparison.differences {
            let _ = writeln!(text, "- {difference}");
        }
    }
    if !comparison.matched.is_empty() {
        let _ = writeln!(
            text,
            "matched ({}): {}",
            comparison.matched.len(),
            comparison.matched.join(", ")
        );
    }
    if !comparison.unverifiable.is_empty() {
        let _ = writeln!(
            text,
            "could not be read, so not counted as matches ({}): {}",
            comparison.unverifiable.len(),
            comparison.unverifiable.join(", ")
        );
    }
    let data = json!({
        "presents": comparison.presents(),
        "verified": comparison.is_verified(),
        "claimed": claimed,
        "differences": comparison.differences,
        "matched": comparison.matched,
        "unverifiable": comparison.unverifiable,
        "url": url,
    });
    Output::Structured { text, data }
}

async fn explain_fingerprint<S: CaptureSession>(
    ctx: Ctx<S>,
    args: Args,
) -> Result<Output, ToolError> {
    let url = nonempty(&args, "url")?.map(str::to_owned);
    let captured = capture(&ctx, url.as_deref()).await?;
    let (text, data) = explain(&captured);
    Ok(Output::Structured { text, data })
}

/// What a detection script would find in a capture, as text for the model and
/// as an object.
pub(super) fn explain(captured: &CapturedFingerprint) -> (String, Value) {
    let seen: Vec<&str> = captured
        .unmapped
        .get("headless_markers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let mut worst = 0_u8;
    let mut leaks = Vec::new();
    for marker in &seen {
        let (severity, what, fix) = MARKERS.iter().find(|(name, ..)| name == marker).map_or(
            (
                "medium",
                "a reading that automated browsers show and ordinary ones do not",
                "compare with a browser a person uses and apply its fingerprint",
            ),
            |&(_, severity, what, fix)| (severity, what, fix),
        );
        worst = worst.max(match severity {
            "high" => 3,
            "medium" => 2,
            _ => 1,
        });
        leaks.push((*marker, severity, what, fix));
    }
    let risk = match worst {
        3 => "high",
        2 => "medium",
        1 => "low",
        _ => "none",
    };
    let coherence = coherence_report(&captured.fingerprint);
    let file = redacted(&captured.fingerprint);
    let mut view = Map::new();
    for key in VIEW_SETTINGS {
        if let Some(value) = file.get(key) {
            view.insert(key.to_owned(), value.clone());
        }
    }
    for key in VIEW_READINGS {
        if let Some(value) = captured.unmapped.get(key) {
            view.insert(key.to_owned(), value.clone());
        }
    }

    let url = &captured.probe.url;
    let mut text = String::new();
    if leaks.is_empty() {
        let _ = writeln!(
            text,
            "Risk: none. A detection script finds no sign of automation in this browser ({url})."
        );
    } else {
        let _ = writeln!(
            text,
            "Risk: {risk}. A detection script finds {} sign(s) of automation in this browser ({url}):",
            leaks.len()
        );
        for (marker, severity, what, fix) in &leaks {
            let _ = writeln!(text, "- {marker} ({severity}): {what}. Fix: {fix}.");
        }
    }
    for (label, key) in [("error", "errors"), ("warning", "warnings")] {
        for line in coherence[key]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            let _ = writeln!(text, "Readings disagree ({label}): {line}");
        }
    }
    let _ = writeln!(
        text,
        "A script reads first: user agent {}, platform {}, time zone {}, screen {}.",
        view.get("user_agent").unwrap_or(&Value::Null),
        view.get("platform").unwrap_or(&Value::Null),
        view.get("timezone").unwrap_or(&Value::Null),
        view.get("screen").unwrap_or(&Value::Null),
    );
    let _ = writeln!(text, "No page can read: {}.", NOT_READABLE.join(", "));

    let data = json!({
        "risk": risk,
        "leaks": leaks
            .iter()
            .map(|(marker, severity, what, fix)| json!({
                "marker": marker,
                "severity": severity,
                "a_detector_sees": what,
                "fix": fix,
            }))
            .collect::<Vec<_>>(),
        "coherence": coherence,
        "detector_view": view,
        "not_readable": NOT_READABLE,
        "unanswered": captured.unmapped.get("unanswered").cloned().unwrap_or_else(|| json!({})),
        "url": url,
    });
    (text, data)
}

// ----------------------------------------------------------------------
// Tools that need no browser
// ----------------------------------------------------------------------

/// `diff_fingerprints` and `patch_fingerprint`.
pub(super) fn offline_tools<S: Send + 'static>() -> Vec<ToolDef<S>> {
    vec![
        ToolDef::new(
            "diff_fingerprints",
            "Diff Fingerprints",
            "Compare two fingerprints: a is what is expected and b what is found, so a setting \
             a leaves natural is not compared. Each is a preset name, a file path or the \
             fingerprint as JSON text. Lists every setting that differs.",
            Effect::Inspect,
            Schema::new()
                .required("a", reference_prop("The expected fingerprint"))
                .required("b", reference_prop("The fingerprint found")),
            diff_fingerprints::<S>,
        )
        .with_output_schema(object_schema(
            &json!({
                "identical": { "type": "boolean" },
                "compared": { "type": "integer" },
                "differences": { "type": "array", "items": { "type": "object" } }
            }),
            &["identical", "compared", "differences"],
        )),
        ToolDef::new(
            "patch_fingerprint",
            "Patch Fingerprint",
            "Change a fingerprint (not the chromedriver patcher): apply a JSON merge patch \
             (RFC 7386) to the v2 file format of a fingerprint, a path or a preset. A null \
             removes a setting, which leaves it to the browser. The result is parsed and \
             checked for contradictions; a patch that makes it unreadable is refused with the \
             setting at fault. Optionally saves a valid result.",
            Effect::Mixed,
            source_schema()
                .required(
                    "patch",
                    Prop::object("The merge patch, for example {\"timezone\": \"Asia/Tokyo\"}"),
                )
                .optional("save_to", save_to_prop()),
            patch_fingerprint::<S>,
        )
        .with_output_schema(object_schema(
            &json!({
                "fingerprint": { "type": "object" },
                "valid": { "type": "boolean" },
                "errors": { "type": "array", "items": { "type": "string" } },
                "warnings": { "type": "array", "items": { "type": "string" } },
                "saved": { "type": "string" }
            }),
            &["fingerprint", "valid", "errors", "warnings"],
        )),
    ]
}

async fn diff_fingerprints<S: Send + 'static>(
    ctx: Ctx<S>,
    args: Args,
) -> Result<Output, ToolError> {
    let expected = reference(&args, "a", ctx.settings())?;
    let found = reference(&args, "b", ctx.settings())?;
    let differences = expected.diff(&found);
    let report = fingerprint_cmd::diff_report("a", "b", &expected, &found);
    let data = json!({
        "identical": differences.is_empty(),
        "compared": expected.claimed_settings().len(),
        "differences": differences,
    });
    Ok(Output::Structured {
        text: report.text,
        data,
    })
}

/// RFC 7386: `patch` merged into `target`.
fn merge_patch(target: &mut Value, patch: &Value) {
    let Value::Object(patch) = patch else {
        *target = patch.clone();
        return;
    };
    if !target.is_object() {
        *target = Value::Object(Map::new());
    }
    let Value::Object(target) = target else {
        return;
    };
    for (key, value) in patch {
        if value.is_null() {
            target.remove(key);
        } else {
            merge_patch(target.entry(key.clone()).or_insert(Value::Null), value);
        }
    }
}

async fn patch_fingerprint<S: Send + 'static>(
    ctx: Ctx<S>,
    args: Args,
) -> Result<Output, ToolError> {
    let base = fingerprint_arg(&args, ctx.settings())?;
    let save = saved_path(ctx.settings(), &args)?;
    let changes = match args.value("patch") {
        Some(Value::Object(object)) => Value::Object(object.clone()),
        Some(Value::String(text)) => serde_json::from_str::<Value>(text)
            .ok()
            .filter(Value::is_object)
            .ok_or_else(|| ToolError::invalid("patch", "not a JSON object"))?,
        _ => return Err(ToolError::invalid("patch", "expected a JSON object")),
    };
    let mut document = base
        .to_json_pretty()
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .ok_or_else(|| ToolError::invalid("fingerprint", "cannot be written as JSON"))?;
    merge_patch(&mut document, &changes);
    let patched = Fingerprint::from_json(&document.to_string()).map_err(|error| match error {
        SeleniumBaseError::InvalidConfig(reason) => ToolError::invalid(
            "patch",
            format!("the patched fingerprint is not valid: {reason}"),
        ),
        other => ToolError::Failed(other),
    })?;

    let report = coherence_report(&patched);
    let valid = report["coherent"].as_bool().unwrap_or(false);
    let mut data = json!({
        "fingerprint": redacted(&patched),
        "valid": valid,
        "errors": report["errors"],
        "warnings": report["warnings"],
    });
    let mut text = String::new();
    match (save, valid) {
        (Some(path), true) => {
            patched.save(&path)?;
            data["saved"] = json!(path.display().to_string());
            let _ = writeln!(text, "Saved to {}.", path.display());
        }
        (Some(_), false) => text.push_str("Not saved: the patched fingerprint has errors.\n"),
        (None, _) => {}
    }
    text.push_str(&pretty(&data));
    Ok(Output::Structured { text, data })
}

// ----------------------------------------------------------------------
// build_fingerprint
// ----------------------------------------------------------------------

/// The arguments of `build_fingerprint`.
pub(super) fn build_schema() -> Schema {
    Schema::new()
        .optional(
            "preset",
            Prop::choice(
                &PRESET_NAMES,
                "Which built-in fingerprint profile (default windows); not with random",
            ),
        )
        .optional(
            "random",
            Prop::boolean("Draw a random but coherent identity instead of using a preset")
                .default(false),
        )
        .optional(
            "seed",
            Prop::integer("Seed that makes a random identity repeatable; needs random").min(0.0),
        )
        .optional(
            "os",
            Prop::choice(
                &OS_NAMES,
                "System a random identity claims (default: this machine's); needs random",
            ),
        )
        .optional(
            "user_agent",
            Prop::string("Replace the user agent of the preset or the random identity"),
        )
        .optional(
            "screen_width",
            Prop::integer("Replace the screen width").min(1.0),
        )
        .optional(
            "screen_height",
            Prop::integer("Replace the screen height").min(1.0),
        )
        .optional("save_to", save_to_prop())
}

fn os_name(os: OsType) -> &'static str {
    match os {
        OsType::Macos => "macos",
        OsType::Linux => "linux",
        OsType::Android => "android",
        OsType::Ios => "ios",
        _ => "windows",
    }
}

fn argument_given(args: &Args, name: &str) -> bool {
    args.value(name).is_some_and(is_given)
}

fn base_fingerprint(args: &Args) -> Result<Fingerprint, ToolError> {
    if !args.bool_or("random", false)? {
        for name in ["seed", "os"] {
            if argument_given(args, name) {
                return Err(ToolError::invalid(name, "needs random: true"));
            }
        }
        let name = args.choice("preset", &PRESET_NAMES, "windows")?;
        return fingerprint_cmd::preset(name)
            .ok_or_else(|| ToolError::invalid("preset", "unknown preset"));
    }
    if argument_given(args, "preset") {
        return Err(ToolError::invalid(
            "preset",
            "cannot be combined with random",
        ));
    }
    let os = match args.choice("os", &OS_NAMES, os_name(fingerprint_cmd::host_os()))? {
        "macos" => OsType::Macos,
        "linux" => OsType::Linux,
        "android" => OsType::Android,
        "ios" => OsType::Ios,
        _ => OsType::Windows,
    };
    let seed = args
        .opt_i64("seed")?
        .map(|seed| {
            u64::try_from(seed)
                .map_err(|_negative| ToolError::invalid("seed", "must not be negative"))
        })
        .transpose()?;
    Ok(fingerprint_cmd::export(&ExportSource::Random {
        os: Some(os),
        seed,
    })?)
}

fn with_overrides(fingerprint: Fingerprint, args: &Args) -> Result<Fingerprint, ToolError> {
    let user_agent = nonempty(args, "user_agent")?;
    let current = fingerprint.screen().value().copied().unwrap_or_default();
    let mut size = [current.width, current.height];
    let mut changed = false;
    for (name, slot) in [("screen_width", 0), ("screen_height", 1)] {
        if let Some(value) = args.opt_usize(name)? {
            size[slot] = u32::try_from(value)
                .ok()
                .ok_or_else(|| ToolError::invalid(name, "too large"))?;
            changed = true;
        }
    }
    if user_agent.is_none() && !changed {
        return Ok(fingerprint);
    }
    let mut builder = fingerprint.to_builder();
    if let Some(user_agent) = user_agent {
        builder = builder.user_agent(user_agent);
    }
    if changed {
        builder = builder.screen(size[0], size[1]);
    }
    builder
        .build()
        .map_err(|error| ToolError::invalid("fingerprint", error.to_string()))
}

/// The answer of `build_fingerprint`: the fingerprint of a preset or of a random
/// identity (the one `sbrs fingerprint export` writes for the same seed), with
/// the overrides applied, and saved when `save_to` is given.
pub(super) fn build(settings: &Settings, args: &Args) -> Result<Output, ToolError> {
    let save = saved_path(settings, args)?;
    let fingerprint = with_overrides(base_fingerprint(args)?, args)?;
    let file = redacted(&fingerprint);
    let Some(path) = save else {
        return Ok(Output::Json(file));
    };
    fingerprint.save(&path)?;
    let text = format!("Saved to {}.\n{}", path.display(), pretty(&file));
    Ok(Output::Structured { text, data: file })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(value: Value) -> Args {
        let Value::Object(map) = value else {
            panic!("arguments are an object")
        };
        Args::new(map)
    }

    fn settings_in(dir: &tempfile::TempDir) -> Settings {
        Settings::new(dir.path())
    }

    #[test]
    fn exactly_one_source_is_required() {
        let dir = tempfile::tempdir().unwrap();
        let settings = settings_in(&dir);
        assert!(fingerprint_arg(&args(json!({})), &settings).is_err());
        let two = args(json!({ "preset": "windows", "path": "x.json" }));
        let error = fingerprint_arg(&two, &settings).unwrap_err().to_string();
        assert!(error.contains("only one"), "{error}");
        assert!(fingerprint_arg(&args(json!({ "preset": "macos" })), &settings).is_ok());
        assert!(
            fingerprint_arg(&args(json!({ "preset": "windows", "path": "" })), &settings).is_ok(),
            "an empty string is unset"
        );
    }

    #[test]
    fn a_fingerprint_may_be_an_object_or_a_string_and_errors_name_the_setting() {
        let dir = tempfile::tempdir().unwrap();
        let settings = settings_in(&dir);
        let object = args(json!({ "fingerprint": { "timezone": "Europe/Berlin" } }));
        let fp = fingerprint_arg(&object, &settings).unwrap();
        assert_eq!(fp.timezone().as_str(), Some("Europe/Berlin"));
        let string = args(json!({ "fingerprint": "{\"timezone\": \"Asia/Tokyo\"}" }));
        let fp = fingerprint_arg(&string, &settings).unwrap();
        assert_eq!(fp.timezone().as_str(), Some("Asia/Tokyo"));
        let bad = args(json!({ "fingerprint": { "screen_width": 1920 } }));
        let error = fingerprint_arg(&bad, &settings).unwrap_err().to_string();
        assert!(error.contains("screen_width"), "{error}");
    }

    #[test]
    fn a_path_is_read_inside_the_output_folder_and_refused_outside() {
        let dir = tempfile::tempdir().unwrap();
        let settings = settings_in(&dir);
        Fingerprint::windows_desktop()
            .save(dir.path().join("w.json"))
            .unwrap();
        let fp = fingerprint_arg(&args(json!({ "path": "w.json" })), &settings).unwrap();
        assert_eq!(fp, Fingerprint::windows_desktop());

        let outside = tempfile::tempdir().unwrap();
        let file = outside.path().join("o.json");
        Fingerprint::windows_desktop().save(&file).unwrap();
        let path = file.display().to_string();
        let error = fingerprint_arg(&args(json!({ "path": path })), &settings).unwrap_err();
        assert!(matches!(error, ToolError::Refused(_)), "{error}");
        let up = fingerprint_arg(
            &args(json!({ "path": "../../../../../etc/hosts" })),
            &settings,
        );
        assert!(up.is_err());
    }

    #[test]
    fn a_file_above_the_cap_is_refused_with_its_size() {
        let dir = tempfile::tempdir().unwrap();
        let settings = settings_in(&dir);
        let big = vec![b' '; usize::try_from(Fingerprint::MAX_FILE_BYTES).unwrap() + 1];
        std::fs::write(dir.path().join("big.json"), big).unwrap();
        let error = fingerprint_arg(&args(json!({ "path": "big.json" })), &settings)
            .unwrap_err()
            .to_string();
        assert!(error.contains("more than"), "{error}");
    }

    #[test]
    fn a_merge_patch_follows_rfc_7386() {
        let mut target = json!({ "a": "b", "c": { "d": "e", "f": "g" } });
        merge_patch(
            &mut target,
            &json!({ "a": "z", "c": { "f": null }, "n": [1] }),
        );
        assert_eq!(target, json!({ "a": "z", "c": { "d": "e" }, "n": [1] }));
        let mut scalar = json!({ "a": 1 });
        merge_patch(&mut scalar, &json!("x"));
        assert_eq!(scalar, json!("x"));
    }

    #[test]
    fn a_proxy_password_is_never_in_the_shown_fingerprint() {
        let proxy = json!({ "type": "http", "host": "p.test", "port": 8080,
            "username": "u", "password": "hunter2", "save_traffic": false });
        let fp = Fingerprint::from_json(&json!({ "proxy": proxy }).to_string()).unwrap();
        let shown = redacted(&fp);
        assert_eq!(shown["proxy"]["password"], "***");
        assert!(!shown.to_string().contains("hunter2"));
    }

    fn captured(markers: &[&str]) -> CapturedFingerprint {
        let readings = json!({
            "version": 1,
            "url": "https://example.test/",
            "navigator": {
                "userAgent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36",
                "platform": "Linux x86_64",
            },
            "headless": markers,
        });
        CapturedFingerprint::from_probe(&readings, "2026-10-23T12:00:00Z").unwrap()
    }

    #[test]
    fn the_loudest_sign_decides_the_risk() {
        let (text, data) = explain(&captured(&[
            "software_renderer",
            "navigator_webdriver_true",
        ]));
        assert_eq!(data["risk"], "high");
        assert_eq!(data["leaks"].as_array().unwrap().len(), 2);
        assert!(text.starts_with("Risk: high."), "{text}");
        assert!(text.contains("Fix:"), "{text}");
        let (text, data) = explain(&captured(&["software_renderer"]));
        assert_eq!(data["risk"], "low");
        assert!(text.contains("software_renderer"));
    }

    #[test]
    fn a_clean_capture_has_no_risk_and_every_known_marker_is_explained() {
        let (text, data) = explain(&captured(&[]));
        assert_eq!(data["risk"], "none");
        assert!(text.starts_with("Risk: none."), "{text}");
        let (_, data) = explain(&captured(&["a_marker_from_the_future"]));
        assert_eq!(data["risk"], "medium", "an unknown marker still counts");
        for (name, ..) in MARKERS {
            let (_, data) = explain(&captured(&[name]));
            assert_ne!(data["risk"], "none", "{name}");
        }
    }

    #[test]
    fn a_comparison_leads_with_the_verdict() {
        let claimed = Fingerprint::windows_desktop();
        let seen = captured(&[]).fingerprint;
        let comparison = claimed.compare_to(&seen);
        let Output::Structured { text, data } = comparison_output(
            &comparison,
            claimed.claimed_settings().len(),
            "https://x.test/",
        ) else {
            panic!("a comparison has text and data")
        };
        assert!(text.starts_with("presents: no, verified: no\n"), "{text}");
        assert_eq!(data["presents"], false);
        assert_ne!(data["differences"], json!([]));
    }
}
