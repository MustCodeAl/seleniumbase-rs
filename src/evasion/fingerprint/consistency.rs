//! Whether a browser agrees with itself: the rules behind
//! [`CapturedFingerprint::contradictions`](crate::evasion::capture::CapturedFingerprint::contradictions).
//!
//! A detector does not need to know the real machine. It needs two surfaces
//! that disagree: a WebGL renderer that names an NVIDIA card next to a WebGPU
//! adapter that says `apple`, a worker that reports ten cores next to a window
//! that reports eight, a phone whose pointer is `fine`. Each rule here reads two
//! surfaces of one capture and says, when they disagree, which persona rule they
//! break. The persona is the one the capture's own user agent names, so no
//! expected fingerprint is needed.
//!
//! The rules are the ones the open-source detectors apply (fpscanner, CreepJS,
//! BotD, FingerprintJS, rebrowser-bot-detector, fp-collect, ThumbmarkJS); the
//! inventory with the sources is `docs/design/fingerprint_consistency.md`. A
//! rule whose readings the capture lacks says nothing: an older reading, or a
//! page whose policy blocked a worker, is not a contradiction.

use std::fmt;

use serde::Serialize;
use serde_json::{json, Map, Value};

use super::{BrowserType, OsType};
use crate::evasion::capture::CapturedFingerprint;

/// How loudly a detector reacts to a contradiction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// A quiet preference or a value few detectors read.
    Low,
    /// A value several detectors read, or one that only sometimes differs.
    Medium,
    /// A value that names the other machine, or that every detector reads.
    High,
}

impl Severity {
    /// The word a report prints.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Two surfaces of one capture that disagree.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Contradiction {
    /// The rule, a short stable name such as `webgpu_vs_webgl`.
    pub rule: &'static str,
    /// How loudly a detector reacts.
    pub severity: Severity,
    /// The two surfaces, as a page reads them (`webgl.renderer`, `webgpu.vendor`).
    pub surfaces: [String; 2],
    /// What each surface says.
    pub values: [Value; 2],
    /// The persona rule the two break.
    pub persona: String,
    /// Why that is a contradiction, in one sentence.
    pub reason: String,
}

impl fmt::Display for Contradiction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}): {} is {} but {} is {}: {}. {}",
            self.rule,
            self.severity,
            self.surfaces[0],
            self.values[0],
            self.surfaces[1],
            self.values[1],
            self.persona,
            self.reason
        )
    }
}

/// The identity a capture claims, read from its own user agent.
struct Persona {
    os: OsType,
    browser: BrowserType,
    mobile: bool,
    chrome_major: Option<u32>,
    user_agent: String,
}

impl Persona {
    fn of(captured: &CapturedFingerprint) -> Self {
        let fp = &captured.fingerprint;
        let user_agent = fp.user_agent().as_str().unwrap_or("").to_owned();
        let os = fp.os_type();
        Self {
            os,
            browser: fp.browser_type(),
            mobile: os.is_mobile() || user_agent.contains(" Mobile"),
            chrome_major: fp.core_version(),
            user_agent,
        }
    }

    fn os_name(&self) -> &'static str {
        match self.os {
            OsType::Windows => "Windows",
            OsType::Macos => "macOS",
            OsType::Linux => "Linux",
            OsType::Android => "Android",
            OsType::Ios => "iOS",
        }
    }

    fn browser_name(&self) -> &'static str {
        match self.browser {
            BrowserType::Chromium => "Chromium",
            BrowserType::Firefox => "Firefox",
            BrowserType::MobileSafari => "Safari",
        }
    }

    fn describe(&self) -> String {
        format!(
            "a {} {} on {}",
            if self.mobile { "mobile" } else { "desktop" },
            self.browser_name(),
            self.os_name()
        )
    }
}

/// A reading under `unmapped`, followed along `path` (`worker.webgl.vendor`).
fn at<'a>(readings: &'a Map<String, Value>, path: &str) -> Option<&'a Value> {
    let mut parts = path.split('.');
    let mut current = readings.get(parts.next()?)?;
    for part in parts {
        current = current.get(part)?;
    }
    Some(current)
}

fn text<'a>(readings: &'a Map<String, Value>, path: &str) -> Option<&'a str> {
    at(readings, path)?.as_str()
}

fn number(readings: &Map<String, Value>, path: &str) -> Option<f64> {
    at(readings, path)?.as_f64()
}

fn flag(readings: &Map<String, Value>, path: &str) -> Option<bool> {
    at(readings, path)?.as_bool()
}

/// Whether an optional reading answered (`{ supported: false }` and
/// `{ unknown: ... }` did not).
fn answered(readings: &Map<String, Value>, key: &str) -> bool {
    at(readings, key).is_some_and(|value| {
        value.is_object()
            && value.get("supported") != Some(&json!(false))
            && value.get("unknown").is_none()
    })
}

/// The builder of one list of contradictions.
struct Findings {
    list: Vec<Contradiction>,
}

impl Findings {
    fn push(
        &mut self,
        rule: &'static str,
        severity: Severity,
        surfaces: [&str; 2],
        values: [Value; 2],
        persona: impl Into<String>,
        reason: impl Into<String>,
    ) {
        self.list.push(Contradiction {
            rule,
            severity,
            surfaces: [surfaces[0].to_owned(), surfaces[1].to_owned()],
            values,
            persona: persona.into(),
            reason: reason.into(),
        });
    }
}

/// Every contradiction of `captured`, loudest first, then in the order the
/// rules run.
pub(super) fn check(captured: &CapturedFingerprint) -> Vec<Contradiction> {
    let persona = Persona::of(captured);
    let readings = &captured.unmapped;
    let mut findings = Findings { list: Vec::new() };
    let fp = &captured.fingerprint;

    gpu(&persona, fp, readings, &mut findings);
    worker(fp, readings, &mut findings);
    frame(fp, readings, &mut findings);
    geometry(&persona, fp, readings, &mut findings);
    media_features(&persona, readings, &mut findings);
    codecs(&persona, readings, &mut findings);
    fonts(&persona, readings, &mut findings);
    apis(&persona, readings, &mut findings);
    plugins(&persona, readings, &mut findings);
    touch(&persona, fp, readings, &mut findings);
    locale(fp, readings, &mut findings);
    time_zone(fp, readings, &mut findings);
    identity(&persona, fp, &mut findings);
    voices(&persona, fp, &mut findings);
    keyboard(fp, readings, &mut findings);
    webgl_parameters(fp, readings, &mut findings);
    storage(&persona, readings, &mut findings);
    battery(fp, &mut findings);
    engine(&persona, fp, readings, &mut findings);

    findings.list.sort_by(|a, b| b.severity.cmp(&a.severity));
    findings.list
}

// ---------------------------------------------------------------------------
// Graphics
// ---------------------------------------------------------------------------

/// The maker a WebGL vendor or renderer string names, in the word WebGPU uses.
fn gpu_maker(text: &str) -> Option<&'static str> {
    let lower = text.to_ascii_lowercase();
    [
        ("swiftshader", "google"),
        ("apple", "apple"),
        ("nvidia", "nvidia"),
        ("geforce", "nvidia"),
        ("radeon", "amd"),
        ("amd", "amd"),
        ("ati ", "amd"),
        ("intel", "intel"),
        ("adreno", "qualcomm"),
        ("qualcomm", "qualcomm"),
        ("mali", "arm"),
        ("arm", "arm"),
        ("powervr", "imgtec"),
        ("llvmpipe", "mesa"),
        ("mesa", "mesa"),
    ]
    .into_iter()
    .find(|(needle, _)| lower.contains(needle))
    .map(|(_, maker)| maker)
}

/// The WebGL renderer and vendor of a capture, unmasked when the browser gave
/// them.
fn webgl_strings(fp: &super::Fingerprint) -> Option<(String, String)> {
    let gl = fp.webgl().value()?;
    Some((
        gl.vendor.clone().unwrap_or_default(),
        gl.renderer.clone().unwrap_or_default(),
    ))
}

fn gpu(
    persona: &Persona,
    fp: &super::Fingerprint,
    readings: &Map<String, Value>,
    findings: &mut Findings,
) {
    let Some((vendor, renderer)) = webgl_strings(fp) else {
        return;
    };
    let webgl_maker = gpu_maker(&renderer).or_else(|| gpu_maker(&vendor));
    if answered(readings, "webgpu") && flag(readings, "webgpu.adapter") == Some(true) {
        if let Some(webgpu_vendor) = text(readings, "webgpu.vendor").filter(|v| !v.is_empty()) {
            let webgpu_maker = gpu_maker(webgpu_vendor).unwrap_or(webgpu_vendor);
            if let Some(webgl_maker) = webgl_maker
                && webgl_maker != webgpu_maker
            {
                findings.push(
                    "webgpu_vs_webgl",
                    Severity::High,
                    ["webgl.renderer", "webgpu.vendor"],
                    [json!(renderer), json!(webgpu_vendor)],
                    "one machine has one GPU, and WebGL and WebGPU both name its maker",
                    format!(
                        "WebGL names a {webgl_maker} GPU while the WebGPU adapter is made by {webgpu_maker}"
                    ),
                );
            }
        }
        if let Some(architecture) =
            text(readings, "webgpu.architecture").filter(|a| a.starts_with("metal"))
            && !matches!(persona.os, OsType::Macos | OsType::Ios)
        {
            findings.push(
                "webgpu_vs_os",
                Severity::High,
                ["navigator.userAgent", "webgpu.architecture"],
                [json!(persona.user_agent), json!(architecture)],
                "Metal is Apple's API: only macOS and iOS report a metal-* WebGPU architecture",
                format!("{} does not run Metal", persona.os_name()),
            );
        }
    }
    // The renderer family against the operating system, as `validate` checks it
    // for a fingerprint one writes.
    if let Some((api, systems)) = super::renderer_family(&format!("{vendor} {renderer}"))
        && !systems.contains(&persona.os)
    {
        findings.push(
            "webgl_vs_os",
            Severity::High,
            ["navigator.userAgent", "webgl.renderer"],
            [json!(persona.user_agent), json!(renderer)],
            format!("{api} renderers are reported by {systems:?} only"),
            format!("{} does not report a {api} renderer", persona.os_name()),
        );
    }
}

// ---------------------------------------------------------------------------
// Other contexts: a worker and a frame
// ---------------------------------------------------------------------------

/// Whether two readings of one value, both present, differ.
fn differ(a: Option<&Value>, b: Option<&Value>) -> Option<(Value, Value)> {
    match (a, b) {
        (Some(a), Some(b)) if !a.is_null() && !b.is_null() && !same(a, b) => {
            Some((a.clone(), b.clone()))
        }
        _ => None,
    }
}

/// JSON equality that reads `8` and `8.0` as one number and compares strings
/// without regard to case.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => match (x.as_f64(), y.as_f64()) {
            (Some(x), Some(y)) => (x - y).abs() < 1e-9,
            _ => x == y,
        },
        (Value::String(x), Value::String(y)) => x.eq_ignore_ascii_case(y),
        _ => a == b,
    }
}

fn worker(fp: &super::Fingerprint, readings: &Map<String, Value>, findings: &mut Findings) {
    if !answered(readings, "worker") {
        return;
    }
    let window_values: [(&str, &str, Option<Value>, Severity); 5] = [
        (
            "navigator.userAgent",
            "worker.userAgent",
            fp.user_agent().as_str().map(Value::from),
            Severity::High,
        ),
        (
            "navigator.platform",
            "worker.platform",
            fp.platform().as_str().map(Value::from),
            Severity::High,
        ),
        (
            "navigator.hardwareConcurrency",
            "worker.hardwareConcurrency",
            fp.hardware_concurrency().value().map(|n| json!(n)),
            Severity::Medium,
        ),
        (
            "navigator.deviceMemory",
            "worker.deviceMemory",
            fp.device_memory().value().map(|n| json!(n)),
            Severity::Medium,
        ),
        (
            "Intl.DateTimeFormat().resolvedOptions().timeZone",
            "worker.timeZone",
            fp.timezone().as_str().map(Value::from),
            Severity::High,
        ),
    ];
    for (window_surface, worker_surface, window_value, severity) in window_values {
        let key = worker_surface.trim_start_matches("worker.");
        if let Some((a, b)) = differ(
            window_value.as_ref(),
            at(readings, &format!("worker.{key}")),
        ) {
            findings.push(
                "worker_vs_window",
                severity,
                [window_surface, worker_surface],
                [a, b],
                "a dedicated worker runs in the same browser and reports the same machine",
                "the window was changed and the worker was not",
            );
        }
    }
    if let Some((vendor, renderer)) = webgl_strings(fp) {
        for (half, value) in [("vendor", vendor), ("renderer", renderer)] {
            if let Some((a, b)) = differ(
                Some(&json!(value)),
                at(readings, &format!("worker.webgl.{half}")),
            ) {
                findings.push(
                    "worker_vs_window",
                    Severity::High,
                    [&format!("webgl.{half}"), &format!("worker.webgl.{half}")],
                    [a, b],
                    "an OffscreenCanvas in a worker draws on the same GPU as the page",
                    "the page's WebGL strings were replaced and the worker's were not",
                );
            }
        }
    }
    if let Some(hints) = fp.client_hints().value()
        && let Some((a, b)) = differ(
            Some(&json!(hints.platform)),
            at(readings, "worker.uaPlatform"),
        )
    {
        findings.push(
            "worker_vs_window",
            Severity::High,
            ["navigator.userAgentData.platform", "worker.uaPlatform"],
            [a, b],
            "Client Hints are the browser's, in every context",
            "the window's userAgentData was replaced and the worker's was not",
        );
    }
}

fn frame(fp: &super::Fingerprint, readings: &Map<String, Value>, findings: &mut Findings) {
    if !answered(readings, "frame") {
        return;
    }
    let pairs: [(&str, &str, Option<Value>); 5] = [
        (
            "navigator.userAgent",
            "frame.userAgent",
            fp.user_agent().as_str().map(Value::from),
        ),
        (
            "navigator.platform",
            "frame.platform",
            fp.platform().as_str().map(Value::from),
        ),
        (
            "navigator.hardwareConcurrency",
            "frame.hardwareConcurrency",
            fp.hardware_concurrency().value().map(|n| json!(n)),
        ),
        (
            "navigator.deviceMemory",
            "frame.deviceMemory",
            fp.device_memory().value().map(|n| json!(n)),
        ),
        (
            "navigator.webdriver",
            "frame.webdriver",
            readings.get("webdriver").cloned(),
        ),
    ];
    for (window_surface, frame_surface, window_value) in pairs {
        let key = frame_surface.trim_start_matches("frame.");
        if let Some((a, b)) = differ(window_value.as_ref(), at(readings, &format!("frame.{key}"))) {
            findings.push(
                "frame_vs_window",
                Severity::High,
                [window_surface, frame_surface],
                [a, b],
                "an about:blank frame is the same browser and reports the same values",
                "the window was changed and the frame was not",
            );
        }
    }
    if let Some((a, b)) = differ(Some(&json!(true)), at(readings, "frame.chrome"))
        && fp.browser_type() == BrowserType::Chromium
        && flag(readings, "features.chrome") == Some(true)
    {
        findings.push(
            "frame_vs_window",
            Severity::High,
            ["window.chrome", "frame.chrome"],
            [a, b],
            "window.chrome is in every frame of a Chromium browser",
            "the window has a chrome object and its frame has none",
        );
    }
}

// ---------------------------------------------------------------------------
// Screen and window
// ---------------------------------------------------------------------------

fn geometry(
    persona: &Persona,
    fp: &super::Fingerprint,
    readings: &Map<String, Value>,
    findings: &mut Findings,
) {
    let Some(screen) = fp.screen().value() else {
        return;
    };
    let (width, height) = (f64::from(screen.width), f64::from(screen.height));
    if screen.width == 800 && screen.height == 600 {
        findings.push(
            "headless_screen",
            Severity::High,
            ["screen.width", "screen.height"],
            [json!(800), json!(600)],
            "800 by 600 is the screen of headless Chrome and of no device anyone uses",
            "set the screen of the persona, or run headed",
        );
    }
    let outer = (
        number(readings, "window.outerWidth"),
        number(readings, "window.outerHeight"),
    );
    let inner = (
        number(readings, "window.innerWidth"),
        number(readings, "window.innerHeight"),
    );
    if let (Some(outer_width), Some(outer_height)) = outer
        && (outer_width > width || outer_height > height)
    {
        findings.push(
            "window_vs_screen",
            Severity::High,
            ["window.outerWidth x outerHeight", "screen.width x height"],
            [
                json!(format!("{outer_width} x {outer_height}")),
                json!(format!("{width} x {height}")),
            ],
            "a window fits on the screen it is shown on",
            "the screen was made smaller than the real window, or the window larger than the screen",
        );
    }
    if let (Some(inner_width), Some(outer_width)) = (inner.0, outer.0)
        && inner_width > outer_width
    {
        findings.push(
            "window_vs_screen",
            Severity::High,
            ["window.innerWidth", "window.outerWidth"],
            [json!(inner_width), json!(outer_width)],
            "the viewport is inside the window",
            "innerWidth is larger than outerWidth",
        );
    }
    if !persona.mobile
        && let Some(avail) = fp.avail_screen().value()
        && avail.width == screen.width
        && avail.height == screen.height
    {
        let severity = if persona.os == OsType::Linux {
            Severity::Low
        } else {
            Severity::Medium
        };
        findings.push(
            "no_taskbar",
            severity,
            ["screen.availHeight", "screen.height"],
            [json!(avail.height), json!(screen.height)],
            "a desktop keeps a taskbar or a menu bar out of the available screen",
            "availWidth and availHeight equal the screen, which a desktop shows only in a kiosk",
        );
    }
    if !persona.mobile
        && let (Some(inner_width), Some(inner_height)) = inner
        && (inner_width - width).abs() < 0.5
        && (inner_height - height).abs() < 0.5
    {
        findings.push(
            "viewport_is_screen",
            Severity::Medium,
            ["window.innerWidth x innerHeight", "screen.width x height"],
            [
                json!(format!("{inner_width} x {inner_height}")),
                json!(format!("{width} x {height}")),
            ],
            "a desktop browser keeps its own frame around the page",
            "the viewport is the whole screen, as in a headless shell or a kiosk",
        );
    }
    if persona.mobile
        && let Some(ratio) = fp.pixel_ratio().value()
        && *ratio < 1.5
    {
        findings.push(
            "pixel_ratio_vs_device",
            Severity::Medium,
            ["navigator.userAgent", "window.devicePixelRatio"],
            [json!(persona.user_agent), json!(ratio)],
            "every phone of the last decade has a pixel ratio of 2 or more",
            "a phone with a pixel ratio under 1.5",
        );
    }
}

// ---------------------------------------------------------------------------
// CSS media features
// ---------------------------------------------------------------------------

fn media_features(persona: &Persona, readings: &Map<String, Value>, findings: &mut Findings) {
    let Some(media) = at(readings, "media_features").and_then(Value::as_object) else {
        return;
    };
    let read = |feature: &str| media.get(feature).and_then(Value::as_str);
    let expected_pointer = if persona.mobile { "coarse" } else { "fine" };
    let expected_hover = if persona.mobile { "none" } else { "hover" };
    for (feature, expected) in [
        ("pointer", expected_pointer),
        ("any-pointer", expected_pointer),
        ("hover", expected_hover),
        ("any-hover", expected_hover),
    ] {
        let Some(found) = read(feature) else {
            continue;
        };
        if found == expected {
            continue;
        }
        let reason = if found == "none" && feature.ends_with("pointer") {
            "no pointing device at all is how a headless browser answers".to_owned()
        } else {
            format!("{} answers ({feature}: {expected})", persona.describe())
        };
        findings.push(
            "media_pointer_vs_device",
            Severity::High,
            ["navigator.userAgent", &format!("matchMedia (({feature}))")],
            [json!(persona.user_agent), json!(found)],
            "a phone has a coarse pointer and cannot hover; a desktop has a fine pointer and hovers",
            reason,
        );
    }
    if let Some(gamut) = read("color-gamut")
        && gamut != "srgb"
        && matches!(persona.os, OsType::Windows | OsType::Linux)
    {
        findings.push(
            "color_gamut_vs_os",
            Severity::Low,
            ["navigator.userAgent", "matchMedia ((color-gamut))"],
            [json!(persona.user_agent), json!(gamut)],
            "Chrome reports a wide gamut on Retina Macs and recent phones; a Windows or Linux desktop is sRGB",
            "a wide colour gamut on a persona whose displays are sRGB",
        );
    }
    if read("forced-colors") == Some("active") && persona.os != OsType::Windows {
        findings.push(
            "forced_colors_vs_os",
            Severity::Low,
            ["navigator.userAgent", "matchMedia ((forced-colors))"],
            [json!(persona.user_agent), json!("active")],
            "forced colours are Windows high contrast",
            "no other system turns forced-colors on",
        );
    }
    // The stylesheet reading of a feature against the script reading of it: a
    // matchMedia patch alone leaves @media rules answering the truth.
    if let Some(css) = at(readings, "css_media").and_then(Value::as_object) {
        for (feature, css_value) in css {
            if let (Some(css_value), Some(script_value)) = (css_value.as_str(), read(feature))
                && css_value != script_value
            {
                findings.push(
                    "media_query_lie",
                    Severity::High,
                    [
                        &format!("matchMedia (({feature}))"),
                        &format!("@media ({feature}) in a stylesheet"),
                    ],
                    [json!(script_value), json!(css_value)],
                    "matchMedia and @media read one media feature",
                    "the script answer was replaced and the stylesheet answer was not",
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Codecs
// ---------------------------------------------------------------------------

fn codecs(persona: &Persona, readings: &Map<String, Value>, findings: &mut Findings) {
    let Some(codecs) = at(readings, "codecs").and_then(Value::as_object) else {
        return;
    };
    let can_play = |name: &str| {
        codecs
            .get("canPlayType")
            .and_then(|c| c.get(name))
            .and_then(Value::as_str)
    };
    let source = |name: &str| {
        codecs
            .get("isTypeSupported")
            .and_then(|c| c.get(name))
            .and_then(Value::as_bool)
    };
    let decodes = |name: &str| {
        codecs
            .get("decodingInfo")
            .and_then(|c| c.get(name))
            .and_then(|c| c.get("supported"))
            .and_then(Value::as_bool)
    };
    for name in ["h264", "vp9", "hevc", "aac"] {
        let played = can_play(name).map(|answer| !answer.is_empty());
        if let (Some(played), Some(supported)) = (played, source(name))
            && played != supported
        {
            findings.push(
                "codecs_disagree",
                Severity::High,
                [
                    &format!("canPlayType ({name})"),
                    &format!("MediaSource.isTypeSupported ({name})"),
                ],
                [json!(can_play(name).unwrap_or("")), json!(supported)],
                "one media engine answers canPlayType, isTypeSupported and decodingInfo",
                "canPlayType was changed and the other answers were not",
            );
        }
        if let (Some(played), Some(decoded)) = (played, decodes(name))
            && played != decoded
        {
            findings.push(
                "codecs_disagree",
                Severity::High,
                [
                    &format!("canPlayType ({name})"),
                    &format!("mediaCapabilities.decodingInfo ({name})"),
                ],
                [json!(can_play(name).unwrap_or("")), json!(decoded)],
                "one media engine answers canPlayType, isTypeSupported and decodingInfo",
                "canPlayType was changed and the other answers were not",
            );
        }
    }
    let chrome_brand = persona.browser == BrowserType::Chromium
        && (persona.user_agent.contains("Chrome/") && !persona.user_agent.contains("Chromium"));
    if chrome_brand && can_play("h264") == Some("") {
        findings.push(
            "codecs_vs_browser",
            Severity::Medium,
            ["navigator.userAgent", "canPlayType (h264)"],
            [json!(persona.user_agent), json!("")],
            "Google Chrome ships H.264 and AAC; a Chromium build without them says it cannot play MP4",
            "a browser that calls itself Chrome cannot play H.264",
        );
    }
    if persona.browser == BrowserType::MobileSafari {
        for (name, what) in [("vorbis", "Ogg Vorbis"), ("theora", "Ogg Theora")] {
            if can_play(name).is_some_and(|answer| !answer.is_empty()) {
                findings.push(
                    "codecs_vs_browser",
                    Severity::High,
                    ["navigator.userAgent", &format!("canPlayType ({name})")],
                    [
                        json!(persona.user_agent),
                        json!(can_play(name).unwrap_or("")),
                    ],
                    "Safari plays no Ogg",
                    format!("a browser that calls itself Safari plays {what}"),
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Fonts
// ---------------------------------------------------------------------------

/// Fonts that ship with one system and no other.
const OS_FONTS: [(OsType, &[&str]); 4] = [
    (
        OsType::Windows,
        &[
            "Segoe UI",
            "Calibri",
            "Cambria",
            "Consolas",
            "Tahoma",
            "Microsoft Sans Serif",
        ],
    ),
    (
        OsType::Macos,
        &[
            "Helvetica Neue",
            "Menlo",
            "Monaco",
            "Geneva",
            "Lucida Grande",
            "Avenir Next",
        ],
    ),
    (
        OsType::Linux,
        &["DejaVu Sans", "Liberation Sans", "Ubuntu", "Cantarell"],
    ),
    (OsType::Android, &["Roboto", "Droid Sans"]),
];

fn fonts(persona: &Persona, readings: &Map<String, Value>, findings: &mut Findings) {
    let Some(present) = at(readings, "fonts_seen.present").and_then(Value::as_array) else {
        return;
    };
    let present: Vec<&str> = present.iter().filter_map(Value::as_str).collect();
    let count = |os: OsType| {
        OS_FONTS
            .iter()
            .find(|(system, _)| *system == os)
            .map_or(0, |(_, fonts)| {
                fonts.iter().filter(|font| present.contains(font)).count()
            })
    };
    // iOS has the Mac's fonts.
    let own = match persona.os {
        OsType::Ios => OsType::Macos,
        other => other,
    };
    if count(own) > 0 {
        return;
    }
    for (other, fonts) in OS_FONTS {
        if other == own || count(other) < 2 {
            continue;
        }
        let seen: Vec<&str> = fonts
            .iter()
            .copied()
            .filter(|font| present.contains(font))
            .collect();
        findings.push(
            "fonts_vs_os",
            Severity::Medium,
            ["navigator.userAgent", "fonts measured in the page"],
            [json!(persona.user_agent), json!(seen)],
            "the system fonts a page can measure are the operating system's",
            format!(
                "fonts of {other:?} are installed and none of {}'s is",
                persona.os_name()
            ),
        );
    }
}

// ---------------------------------------------------------------------------
// API presence by browser and operating system
// ---------------------------------------------------------------------------

/// Whether an API is there on a persona: `Some(true)` it must be, `Some(false)`
/// it never is, `None` it depends (a flag, a version, a device).
///
/// From MDN's browser compatibility data (read 2026-11-06) for Chrome 128 and
/// later, Safari on iOS and desktop Firefox.
fn api_expected(persona: &Persona, api: &str) -> Option<bool> {
    use BrowserType::{Chromium, Firefox, MobileSafari};
    use OsType::{Android, Ios, Linux, Macos, Windows};
    let (browser, os) = (persona.browser, persona.os);
    match (api, browser, os) {
        // Engine-level APIs: what the family has or lacks everywhere.
        (
            "chrome" | "userAgentData" | "deviceMemory" | "getBattery" | "connection",
            Chromium,
            _,
        ) => Some(true),
        ("chrome" | "userAgentData" | "deviceMemory" | "getBattery" | "connection", _, _) => {
            Some(false)
        }
        ("oscpu", Firefox, _) => Some(true),
        ("oscpu", _, _) => Some(false),
        ("applePaySession" | "audioSession" | "safari", MobileSafari, _) => Some(true),
        ("applePaySession" | "audioSession" | "safari", _, _) => Some(false),
        ("keyboard", Chromium, _) => Some(true),
        ("keyboard", _, _) => Some(false),
        ("bluetooth", Chromium, _) => Some(true),
        ("bluetooth", _, _) => Some(false),
        ("usb" | "xr", Chromium, _) => Some(true),
        ("usb" | "xr", _, _) => Some(false),
        // Desktop-only Chromium APIs.
        (
            "hid" | "windowControlsOverlay" | "eyeDropper" | "documentPictureInPicture",
            Chromium,
            Android,
        ) => Some(false),
        ("hid" | "windowControlsOverlay" | "documentPictureInPicture", Chromium, _) => Some(true),
        // X11 has it and Wayland has not.
        ("eyeDropper", Chromium, Linux) => None,
        ("eyeDropper", Chromium, _) => Some(true),
        ("hid" | "windowControlsOverlay" | "eyeDropper" | "documentPictureInPicture", _, _) => {
            Some(false)
        }
        // Android-only and Apple-only.
        ("contacts", Chromium, Android) => Some(true),
        ("contacts", _, _) => Some(false),
        ("barcodeDetector", Chromium, Windows | Linux) => Some(false),
        ("barcodeDetector", Chromium, Macos | Android) => Some(true),
        ("barcodeDetector", _, _) => None,
        ("share", Chromium, Windows | Macos | Linux | Android) => Some(true),
        ("share", MobileSafari, _) => Some(true),
        ("share", Firefox, _) => Some(false),
        // Serial reached Android Chrome in 148.
        ("serial", Chromium, Android) => persona.chrome_major.map(|major| major >= 148),
        ("serial", Chromium, _) => Some(true),
        ("serial", _, _) => Some(false),
        ("notification", MobileSafari, Ios) => Some(false),
        ("notification", _, _) => Some(true),
        _ => None,
    }
}

const APIS: [(&str, &str); 22] = [
    ("chrome", "window.chrome"),
    ("userAgentData", "navigator.userAgentData"),
    ("deviceMemory", "navigator.deviceMemory"),
    ("getBattery", "navigator.getBattery"),
    ("connection", "navigator.connection"),
    ("oscpu", "navigator.oscpu"),
    ("applePaySession", "window.ApplePaySession"),
    ("audioSession", "navigator.audioSession"),
    ("safari", "window.safari"),
    ("keyboard", "navigator.keyboard"),
    ("bluetooth", "navigator.bluetooth"),
    ("usb", "navigator.usb"),
    ("xr", "navigator.xr"),
    ("hid", "navigator.hid"),
    ("serial", "navigator.serial"),
    ("windowControlsOverlay", "navigator.windowControlsOverlay"),
    ("eyeDropper", "window.EyeDropper"),
    (
        "documentPictureInPicture",
        "window.documentPictureInPicture",
    ),
    ("contacts", "navigator.contacts"),
    ("barcodeDetector", "window.BarcodeDetector"),
    ("share", "navigator.share"),
    ("notification", "window.Notification"),
];

fn apis(persona: &Persona, readings: &Map<String, Value>, findings: &mut Findings) {
    let Some(features) = at(readings, "features").and_then(Value::as_object) else {
        return;
    };
    for (key, surface) in APIS {
        let (Some(found), Some(expected)) = (
            features.get(key).and_then(Value::as_bool),
            api_expected(persona, key),
        ) else {
            continue;
        };
        if found == expected {
            continue;
        }
        let engine_level = matches!(
            key,
            "chrome" | "userAgentData" | "deviceMemory" | "getBattery" | "connection" | "oscpu"
        );
        let severity = match (found, engine_level) {
            (_, true) => Severity::High,
            (true, false) => Severity::Medium,
            (false, false) => Severity::Low,
        };
        let reason = if found {
            format!("{} has no {surface}", persona.describe())
        } else {
            format!("{} always has {surface}", persona.describe())
        };
        findings.push(
            "api_presence_vs_persona",
            severity,
            ["navigator.userAgent", surface],
            [json!(persona.user_agent), json!(found)],
            "the APIs a browser exposes are decided by its engine and its operating system",
            reason,
        );
    }
}

// ---------------------------------------------------------------------------
// Plugins, touch, locale, time zone
// ---------------------------------------------------------------------------

fn plugins(persona: &Persona, readings: &Map<String, Value>, findings: &mut Findings) {
    let Some(count) = number(readings, "plugins_length") else {
        return;
    };
    if persona.mobile && count > 0.0 {
        findings.push(
            "plugins_vs_device",
            Severity::High,
            ["navigator.userAgent", "navigator.plugins.length"],
            [json!(persona.user_agent), json!(count)],
            "a phone browser lists no plugins",
            "Chrome on Android and Safari on iOS have an empty plugin list",
        );
    }
    if let Some(pdf) = flag(readings, "pdf_viewer_enabled")
        && !persona.mobile
        && pdf != (count > 0.0)
    {
        findings.push(
            "pdf_viewer_vs_plugins",
            Severity::Medium,
            ["navigator.pdfViewerEnabled", "navigator.plugins.length"],
            [json!(pdf), json!(count)],
            "the PDF viewer is what the plugin list of a desktop Chromium holds",
            "one says there is a PDF viewer and the other says there is not",
        );
    }
}

fn touch(
    persona: &Persona,
    fp: &super::Fingerprint,
    readings: &Map<String, Value>,
    findings: &mut Findings,
) {
    let Some(on_touch_start) = flag(readings, "features.onTouchStart") else {
        return;
    };
    let points = fp.max_touch_points().value().copied();
    if persona.mobile && !on_touch_start {
        findings.push(
            "touch_vs_device",
            Severity::High,
            ["navigator.userAgent", "'ontouchstart' in window"],
            [json!(persona.user_agent), json!(false)],
            "a phone fires touch events",
            "maxTouchPoints alone was set; the touch events were not turned on",
        );
    }
    if !persona.mobile && points == Some(0) && on_touch_start {
        findings.push(
            "touch_vs_device",
            Severity::Medium,
            ["navigator.maxTouchPoints", "'ontouchstart' in window"],
            [json!(0), json!(true)],
            "touch events come with touch points",
            "the page fires touch events on a device that claims none",
        );
    }
}

fn locale(fp: &super::Fingerprint, readings: &Map<String, Value>, findings: &mut Findings) {
    let Some(language) = fp.locale().as_str() else {
        return;
    };
    if let Some(list) = fp.languages().as_str()
        && let Some(first) = list
            .split(',')
            .next()
            .map(|tag| tag.split(';').next().unwrap_or(tag).trim())
        && !first.is_empty()
        && !first.eq_ignore_ascii_case(language)
    {
        findings.push(
            "language_vs_languages",
            Severity::High,
            ["navigator.language", "navigator.languages[0]"],
            [json!(language), json!(first)],
            "navigator.language is the first entry of navigator.languages",
            "the two were set apart",
        );
    }
    if let Some(intl) = text(readings, "intl_locale")
        && language_of(intl) != language_of(language)
    {
        findings.push(
            "intl_vs_language",
            Severity::High,
            [
                "navigator.language",
                "Intl.DateTimeFormat().resolvedOptions().locale",
            ],
            [json!(language), json!(intl)],
            "Chrome formats dates and numbers in the browser's own language",
            "navigator.language was changed and the Intl locale was not (a locale override moves both)",
        );
    }
    if let Some(locales) = at(readings, "intl_locales").and_then(Value::as_object) {
        for (constructor, value) in locales {
            if let Some(value) = value.as_str()
                && language_of(value) != language_of(language)
            {
                findings.push(
                    "intl_vs_language",
                    Severity::Medium,
                    [
                        "navigator.language",
                        &format!("Intl.{constructor}().resolvedOptions().locale"),
                    ],
                    [json!(language), json!(value)],
                    "every Intl constructor defaults to the browser's own locale",
                    "one Intl constructor was changed and another was not",
                );
            }
        }
    }
}

/// The language subtag of a BCP 47 tag, lower-cased (`en-US` gives `en`).
fn language_of(tag: &str) -> String {
    tag.split(['-', '_'])
        .next()
        .unwrap_or(tag)
        .to_ascii_lowercase()
}

fn time_zone(fp: &super::Fingerprint, readings: &Map<String, Value>, findings: &mut Findings) {
    let Some(zone) = fp.timezone().as_str() else {
        return;
    };
    let Ok(tz) = jiff::tz::TimeZone::get(zone) else {
        return;
    };
    for (month, key) in [("january", "january"), ("july", "july")] {
        let Some(found) = number(readings, &format!("time_zone_offsets.{key}")) else {
            continue;
        };
        let Ok(instant) = format!(
            "2024-{:02}-15T12:00:00Z",
            if month == "january" { 1 } else { 7 }
        )
        .parse::<jiff::Timestamp>() else {
            continue;
        };
        // `getTimezoneOffset` counts minutes west of UTC.
        let expected = -f64::from(tz.to_offset(instant).seconds()) / 60.0;
        if (found - expected).abs() > 0.5 {
            findings.push(
                "offset_vs_zone",
                Severity::High,
                [
                    "Intl.DateTimeFormat().resolvedOptions().timeZone",
                    &format!("new Date(2024, {month}).getTimezoneOffset()"),
                ],
                [json!(zone), json!(found)],
                format!("{zone} is {expected} minutes from UTC in {month}"),
                "Intl was changed and Date was not (a time zone override moves both)",
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The identity's own strings
// ---------------------------------------------------------------------------

fn identity(persona: &Persona, fp: &super::Fingerprint, findings: &mut Findings) {
    let ua = &persona.user_agent;
    if ua.is_empty() {
        return;
    }
    if let Some(platform) = fp.platform().as_str() {
        let ok = match persona.os {
            OsType::Windows => platform.starts_with("Win"),
            OsType::Macos => platform.contains("Mac"),
            OsType::Linux => platform.contains("Linux") && !platform.contains("arm"),
            OsType::Android => platform.contains("Linux") || platform.contains("arm"),
            OsType::Ios => platform.contains("iP"),
        };
        if !ok {
            findings.push(
                "platform_vs_user_agent",
                Severity::High,
                ["navigator.userAgent", "navigator.platform"],
                [json!(ua), json!(platform)],
                format!("{} reports a platform of its own family", persona.os_name()),
                "the user agent and navigator.platform name different systems",
            );
        }
    }
    let Some(hints) = fp.client_hints().value() else {
        return;
    };
    let expected_platform = match persona.os {
        OsType::Windows => "Windows",
        OsType::Macos => "macOS",
        OsType::Linux => "Linux",
        OsType::Android => "Android",
        OsType::Ios => "",
    };
    if !hints.platform.is_empty() && hints.platform != expected_platform {
        findings.push(
            "client_hints_vs_user_agent",
            Severity::High,
            ["navigator.userAgent", "navigator.userAgentData.platform"],
            [json!(ua), json!(hints.platform)],
            "the Client Hints platform names the system of the user agent",
            format!(
                "the user agent says {} and the hints say {}",
                persona.os_name(),
                hints.platform
            ),
        );
    }
    let ua_mobile = ua.contains(" Mobile");
    if hints.mobile != ua_mobile {
        findings.push(
            "client_hints_vs_user_agent",
            Severity::High,
            ["navigator.userAgent", "navigator.userAgentData.mobile"],
            [json!(ua), json!(hints.mobile)],
            "the Mobile token of the user agent and the mobile hint say the same",
            "one says mobile and the other does not",
        );
    }
    if let Some(major) = persona.chrome_major {
        let brand_major = hints
            .brands
            .iter()
            .chain(hints.full_version_list.iter())
            .filter(|brand| !brand.brand.contains("Not") && !brand.brand.contains("Brand"))
            .find_map(|brand| brand.version.split('.').next()?.parse::<u32>().ok());
        if let Some(brand_major) = brand_major
            && brand_major != major
        {
            findings.push(
                "client_hints_vs_user_agent",
                Severity::High,
                ["navigator.userAgent", "navigator.userAgentData.brands"],
                [json!(ua), json!(brand_major)],
                "the brands name the Chromium major of the user agent",
                format!("the user agent is Chrome {major} and the brands say {brand_major}"),
            );
        }
        if let Some(full_major) = hints
            .ua_full_version
            .split('.')
            .next()
            .and_then(|m| m.parse::<u32>().ok())
            && full_major != major
        {
            findings.push(
                "client_hints_vs_user_agent",
                Severity::High,
                [
                    "navigator.userAgent",
                    "navigator.userAgentData uaFullVersion",
                ],
                [json!(ua), json!(hints.ua_full_version)],
                "the full version is the user agent's major, continued",
                format!(
                    "the user agent is Chrome {major} and the full version is {}",
                    hints.ua_full_version
                ),
            );
        }
    }
    if persona.os == OsType::Android && !(hints.architecture.is_empty() && hints.bitness.is_empty())
    {
        findings.push(
            "client_hints_vs_user_agent",
            Severity::Medium,
            [
                "navigator.userAgent",
                "navigator.userAgentData architecture, bitness",
            ],
            [
                json!(ua),
                json!(format!("{} {}", hints.architecture, hints.bitness)),
            ],
            "Chrome on Android reports no architecture and no bitness",
            "an Android persona with a desktop's architecture",
        );
    }
    if hints
        .brands
        .iter()
        .any(|brand| brand.brand.contains("Headless"))
    {
        findings.push(
            "client_hints_vs_user_agent",
            Severity::High,
            ["navigator.userAgent", "navigator.userAgentData.brands"],
            [
                json!(ua),
                json!(hints
                    .brands
                    .iter()
                    .map(|b| b.brand.as_str())
                    .collect::<Vec<_>>()),
            ],
            "the brands of a browser a person uses name Chrome, not HeadlessChrome",
            "the user agent was changed and the brands still say HeadlessChrome",
        );
    }
}

// ---------------------------------------------------------------------------
// Voices, keyboard, WebGL parameters, storage, battery, engine
// ---------------------------------------------------------------------------

const APPLE_VOICES: [&str; 8] = [
    "Samantha", "Alex", "Daniel", "Karen", "Moira", "Tessa", "Fred", "Victoria",
];

fn voices(persona: &Persona, fp: &super::Fingerprint, findings: &mut Findings) {
    let Some(list) = fp.speech_voices().value() else {
        return;
    };
    let microsoft: Vec<&str> = list
        .iter()
        .filter(|voice| voice.name.starts_with("Microsoft "))
        .map(|voice| voice.name.as_str())
        .collect();
    let apple: Vec<&str> = list
        .iter()
        .filter(|voice| APPLE_VOICES.contains(&voice.name.as_str()))
        .map(|voice| voice.name.as_str())
        .collect();
    if !microsoft.is_empty() && persona.os != OsType::Windows {
        findings.push(
            "voices_vs_os",
            Severity::Medium,
            ["navigator.userAgent", "speechSynthesis.getVoices()"],
            [json!(persona.user_agent), json!(microsoft)],
            "the Microsoft voices are Windows's",
            format!("{} has no Microsoft voice", persona.os_name()),
        );
    }
    if !apple.is_empty() && !matches!(persona.os, OsType::Macos | OsType::Ios) {
        findings.push(
            "voices_vs_os",
            Severity::Medium,
            ["navigator.userAgent", "speechSynthesis.getVoices()"],
            [json!(persona.user_agent), json!(apple)],
            "Samantha, Alex and the other named voices are Apple's",
            format!("{} has no Apple voice", persona.os_name()),
        );
    }
}

/// The family of a keyboard layout from a few keys of its map.
fn layout_family(sample: &Map<String, Value>) -> Option<&'static str> {
    let key = |code: &str| sample.get(code).and_then(Value::as_str);
    match (key("KeyQ"), key("KeyY"), key("KeyZ"), key("KeyA")) {
        (Some("a"), _, _, Some("q")) => Some("azerty"),
        (Some("q"), Some("z"), Some("y"), _) => Some("qwertz"),
        (Some("q"), Some("y"), Some("z"), _) => Some("qwerty"),
        _ => None,
    }
}

fn keyboard(fp: &super::Fingerprint, readings: &Map<String, Value>, findings: &mut Findings) {
    let (Some(sample), Some(language)) = (
        at(readings, "keyboard.sample").and_then(Value::as_object),
        fp.locale().as_str(),
    ) else {
        return;
    };
    let Some(found) = layout_family(sample) else {
        return;
    };
    let expected = match language_of(language).as_str() {
        "de" | "cs" | "sk" | "hu" | "sl" | "hr" => Some("qwertz"),
        "fr" if !language.ends_with("CA") && !language.ends_with("CH") => Some("azerty"),
        "en" | "es" | "it" | "pt" | "nl" | "pl" | "sv" | "da" | "nb" | "fi" | "ja" | "ko"
        | "zh" => Some("qwerty"),
        _ => None,
    };
    if let Some(expected) = expected
        && expected != found
    {
        findings.push(
            "keyboard_vs_locale",
            Severity::Low,
            ["navigator.language", "navigator.keyboard.getLayoutMap()"],
            [json!(language), json!(found)],
            format!("a {language} machine has a {expected} keyboard"),
            format!("the keyboard map is {found}"),
        );
    }
}

fn webgl_parameters(
    fp: &super::Fingerprint,
    readings: &Map<String, Value>,
    findings: &mut Findings,
) {
    let Some((_, renderer)) = webgl_strings(fp) else {
        return;
    };
    let mobile_gpu = ["Adreno", "Mali", "PowerVR"]
        .iter()
        .any(|needle| renderer.contains(needle));
    if !mobile_gpu {
        return;
    }
    if let Some(extensions) = at(readings, "webgl_info.extensions").and_then(Value::as_array)
        && extensions
            .iter()
            .any(|e| e.as_str() == Some("WEBGL_compressed_texture_s3tc"))
    {
        findings.push(
            "webgl_extensions_vs_renderer",
            Severity::Low,
            ["webgl.renderer", "getSupportedExtensions()"],
            [json!(renderer), json!("WEBGL_compressed_texture_s3tc")],
            "a mobile GPU compresses textures with ETC and ASTC, not S3TC",
            "the renderer string was replaced and the extension list was not",
        );
    }
}

fn storage(persona: &Persona, readings: &Map<String, Value>, findings: &mut Findings) {
    let Some(quota) = number(readings, "storage.quota") else {
        return;
    };
    if !persona.mobile && quota < 1024.0 * 1024.0 * 1024.0 {
        findings.push(
            "storage_quota_vs_device",
            Severity::Low,
            ["navigator.userAgent", "navigator.storage.estimate().quota"],
            [json!(persona.user_agent), json!(quota)],
            "a desktop disk gives a site tens of gigabytes of quota",
            "a quota under a gigabyte is an incognito context or a tiny disk",
        );
    }
}

fn battery(fp: &super::Fingerprint, findings: &mut Findings) {
    let Some(battery) = fp.battery().value() else {
        return;
    };
    if battery.charging && battery.level < 1.0 && battery.charging_time == 0.0 {
        findings.push(
            "battery_shape",
            Severity::Low,
            ["battery.level", "battery.chargingTime"],
            [json!(battery.level), json!(0)],
            "a battery that is still charging has a time to full; zero is what a full one reports",
            "the level is below 100 % while the charging time is 0",
        );
    }
}

fn engine(
    persona: &Persona,
    fp: &super::Fingerprint,
    readings: &Map<String, Value>,
    findings: &mut Findings,
) {
    let ua = &persona.user_agent;
    if let Some(vendor) = fp.vendor().as_str() {
        let expected = match persona.browser {
            BrowserType::Chromium => "Google Inc.",
            BrowserType::Firefox => "",
            BrowserType::MobileSafari => "Apple Computer, Inc.",
        };
        if vendor != expected {
            findings.push(
                "vendor_vs_user_agent",
                Severity::High,
                ["navigator.userAgent", "navigator.vendor"],
                [json!(ua), json!(vendor)],
                format!("{} reports the vendor '{expected}'", persona.browser_name()),
                "the user agent and navigator.vendor name different browsers",
            );
        }
    }
    if let Some(product_sub) = fp.product_sub().as_str() {
        let expected = match persona.browser {
            BrowserType::Firefox => "20100101",
            _ => "20030107",
        };
        if product_sub != expected {
            findings.push(
                "product_sub_vs_user_agent",
                Severity::High,
                ["navigator.userAgent", "navigator.productSub"],
                [json!(ua), json!(product_sub)],
                format!("{} reports productSub {expected}", persona.browser_name()),
                "the user agent and navigator.productSub name different browsers",
            );
        }
    }
    if let Some(etsl) = number(readings, "engine.etsl") {
        let expected = match persona.browser {
            BrowserType::Chromium => 33.0,
            _ => 37.0,
        };
        if (etsl - expected).abs() > 0.5 {
            findings.push(
                "engine_vs_user_agent",
                Severity::High,
                ["navigator.userAgent", "eval.toString().length"],
                [json!(ua), json!(etsl)],
                "V8 prints eval in 33 characters; SpiderMonkey and JavaScriptCore in 37",
                "the user agent claims one engine and the script runs on another; no spoof can change this",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Windows preset applied in JavaScript alone to a headless Chrome on a
    /// Mac: every surface the settings reach says Windows, every other one
    /// says Mac. Written by hand from a real capture of each half.
    const WINDOWS_ON_MAC: &str =
        include_str!("../../../tests/fixtures/fingerprint/probe_chrome_windows_on_mac.json");
    /// A Chrome on a Mac that agrees with itself.
    const CHROME_MACOS: &str =
        include_str!("../../../tests/fixtures/fingerprint/probe_chrome_macos.json");
    const ANDROID_UA: &str = "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 \
                              (KHTML, like Gecko) Chrome/155.0.0.0 Mobile Safari/537.36";

    fn probe() -> Value {
        serde_json::from_str(WINDOWS_ON_MAC).unwrap()
    }

    fn found(raw: &Value) -> Vec<Contradiction> {
        CapturedFingerprint::from_probe(raw, "2026-11-06T12:00:00Z")
            .unwrap()
            .contradictions()
    }

    fn rules(raw: &Value) -> Vec<&'static str> {
        found(raw).into_iter().map(|c| c.rule).collect()
    }

    /// Sets the reading at the dotted `path` of a raw probe.
    fn set(raw: &mut Value, path: &str, value: Value) {
        let mut current = raw;
        let mut parts = path.split('.').peekable();
        while let Some(part) = parts.next() {
            if parts.peek().is_none() {
                current[part] = value;
                return;
            }
            current = &mut current[part];
        }
    }

    fn without(raw: &mut Value, keys: &[&str]) {
        let object = raw.as_object_mut().unwrap();
        for key in keys {
            object.remove(*key);
        }
    }

    /// The Windows persona, made a phone: the user agent, the platform and the
    /// Client Hints all say Android, every other reading stays the desktop's.
    fn android_persona(raw: &mut Value) {
        set(raw, "navigator.userAgent", json!(ANDROID_UA));
        set(
            raw,
            "navigator.appVersion",
            json!(ANDROID_UA.trim_start_matches("Mozilla/")),
        );
        set(raw, "navigator.platform", json!("Linux armv8l"));
        set(raw, "navigator.maxTouchPoints", json!(5));
        set(raw, "userAgentData.platform", json!("Android"));
        set(raw, "userAgentData.mobile", json!(true));
        set(raw, "userAgentData.architecture", json!(""));
        set(raw, "userAgentData.bitness", json!(""));
        set(raw, "userAgentData.formFactors", json!(["Mobile"]));
    }

    #[test]
    fn a_windows_persona_on_a_mac_is_given_away_by_the_surfaces_the_settings_do_not_reach() {
        let found = found(&probe());
        let rules: Vec<&str> = found.iter().map(|c| c.rule).collect();

        for expected in [
            // The WebGPU adapter is Apple's and runs Metal.
            "webgpu_vs_webgl",
            "webgpu_vs_os",
            // The worker has the Mac's cores and GPU.
            "worker_vs_window",
            // Menlo and Helvetica Neue, and no Segoe UI.
            "fonts_vs_os",
            // BarcodeDetector is on macOS and not on Windows.
            "api_presence_vs_persona",
            // Samantha speaks.
            "voices_vs_os",
            // A Retina display's P3 gamut.
            "color_gamut_vs_os",
            "battery_shape",
        ] {
            assert!(
                rules.contains(&expected),
                "{expected} is missing from {rules:?}"
            );
        }
        for unexpected in [
            "frame_vs_window",
            "platform_vs_user_agent",
            "client_hints_vs_user_agent",
            "intl_vs_language",
            "language_vs_languages",
            "offset_vs_zone",
            "codecs_disagree",
            "codecs_vs_browser",
            "media_pointer_vs_device",
            "media_query_lie",
            "plugins_vs_device",
            "pdf_viewer_vs_plugins",
            "touch_vs_device",
            "engine_vs_user_agent",
            "vendor_vs_user_agent",
            "keyboard_vs_locale",
            "no_taskbar",
            "viewport_is_screen",
            "window_vs_screen",
            "headless_screen",
            "storage_quota_vs_device",
            "webgl_vs_os",
        ] {
            assert!(
                !rules.contains(&unexpected),
                "{unexpected} fired on surfaces that agree: {found:#?}"
            );
        }
    }

    #[test]
    fn the_loudest_contradiction_comes_first_and_names_both_surfaces() {
        let found = found(&probe());

        let first = &found[0];
        assert_eq!(first.rule, "webgpu_vs_webgl");
        assert_eq!(first.severity, Severity::High);
        assert_eq!(first.surfaces, ["webgl.renderer", "webgpu.vendor"]);
        assert_eq!(first.values[1], json!("apple"));
        assert!(
            found
                .windows(2)
                .all(|pair| pair[0].severity >= pair[1].severity),
            "loudest first: {found:#?}"
        );
        let worker = found
            .iter()
            .filter(|c| c.rule == "worker_vs_window")
            .count();
        assert_eq!(
            worker, 3,
            "the cores, the WebGL vendor and the WebGL renderer: {found:#?}"
        );
        let text = first.to_string();
        assert!(
            text.starts_with("webgpu_vs_webgl (high): webgl.renderer is "),
            "{text}"
        );
        assert!(text.contains("but webgpu.vendor is \"apple\": "), "{text}");
    }

    #[test]
    fn a_contradiction_serializes_with_its_rule_severity_surfaces_and_values() {
        let first = serde_json::to_value(&found(&probe())[0]).unwrap();

        assert_eq!(first["rule"], "webgpu_vs_webgl");
        assert_eq!(first["severity"], "high");
        assert_eq!(
            first["surfaces"],
            json!(["webgl.renderer", "webgpu.vendor"])
        );
        assert!(first["values"][0].as_str().unwrap().contains("NVIDIA"));
        assert!(first["persona"].is_string() && first["reason"].is_string());
    }

    #[test]
    fn a_browser_that_agrees_with_itself_has_no_contradictions() {
        let raw: Value = serde_json::from_str(CHROME_MACOS).unwrap();

        assert_eq!(found(&raw), Vec::new());
    }

    #[test]
    fn a_rule_whose_readings_the_capture_lacks_says_nothing() {
        let mut raw = probe();
        without(
            &mut raw,
            &[
                "webgpu",
                "worker",
                "fonts",
                "features",
                "speech",
                "battery",
                "mediaFeatures",
                "cssMedia",
            ],
        );

        assert_eq!(found(&raw), Vec::new(), "the surfaces that are left agree");
    }

    #[test]
    fn a_phone_persona_on_a_desktop_shows_a_fine_pointer_plugins_and_no_touch() {
        let mut raw = probe();
        android_persona(&mut raw);

        let found = found(&raw);
        let rules: Vec<&str> = found.iter().map(|c| c.rule).collect();
        for expected in [
            "media_pointer_vs_device",
            "plugins_vs_device",
            "touch_vs_device",
            "pixel_ratio_vs_device",
            // navigator.hid is a desktop API.
            "api_presence_vs_persona",
            // A Direct3D renderer on Android.
            "webgl_vs_os",
        ] {
            assert!(
                rules.contains(&expected),
                "{expected} is missing from {rules:?}"
            );
        }
        let pointer = found
            .iter()
            .find(|c| c.rule == "media_pointer_vs_device")
            .unwrap();
        assert_eq!(pointer.surfaces[1], "matchMedia ((pointer))");
        assert_eq!(pointer.values[1], json!("fine"));
        assert!(
            !rules.contains(&"client_hints_vs_user_agent"),
            "the hints were made to agree: {found:#?}"
        );
    }

    #[test]
    fn each_rule_fires_on_the_reading_it_watches() {
        let cases: Vec<(&str, Vec<(&str, Value)>)> = vec![
            ("intl_vs_language", vec![("intl.locale", json!("de-DE"))]),
            (
                "intl_vs_language",
                vec![("intl.locales.NumberFormat", json!("fr-FR"))],
            ),
            (
                "language_vs_languages",
                vec![("navigator.languages", json!(["fr-FR", "en-US"]))],
            ),
            ("offset_vs_zone", vec![("intl.januaryOffsetMinutes", json!(0))]),
            ("media_query_lie", vec![("cssMedia.pointer", json!("coarse"))]),
            (
                "codecs_disagree",
                vec![("codecs.isTypeSupported.h264", json!(false))],
            ),
            (
                "codecs_disagree",
                vec![("codecs.decodingInfo.vp9.supported", json!(false))],
            ),
            (
                "headless_screen",
                vec![("screen.width", json!(800)), ("screen.height", json!(600))],
            ),
            ("window_vs_screen", vec![("window.outerWidth", json!(2560))]),
            ("window_vs_screen", vec![("window.innerWidth", json!(1921))]),
            ("no_taskbar", vec![("screen.availHeight", json!(1080))]),
            (
                "viewport_is_screen",
                vec![("window.innerHeight", json!(1080))],
            ),
            ("frame_vs_window", vec![("frame.hardwareConcurrency", json!(10))]),
            ("frame_vs_window", vec![("frame.chrome", json!(false))]),
            ("engine_vs_user_agent", vec![("engine.etsl", json!(37))]),
            ("vendor_vs_user_agent", vec![("navigator.vendor", json!(""))]),
            (
                "product_sub_vs_user_agent",
                vec![("navigator.productSub", json!("20100101"))],
            ),
            (
                "platform_vs_user_agent",
                vec![("navigator.platform", json!("MacIntel"))],
            ),
            (
                "client_hints_vs_user_agent",
                vec![("userAgentData.platform", json!("macOS"))],
            ),
            (
                "client_hints_vs_user_agent",
                vec![("userAgentData.uaFullVersion", json!("154.0.7000.1"))],
            ),
            (
                "pdf_viewer_vs_plugins",
                vec![("navigator.pluginsLength", json!(0))],
            ),
            (
                "keyboard_vs_locale",
                vec![
                    ("navigator.language", json!("de-DE")),
                    ("navigator.languages", json!(["de-DE", "de"])),
                ],
            ),
            (
                "storage_quota_vs_device",
                vec![("storage.quota", json!(120_000_000))],
            ),
            (
                "forced_colors_vs_os",
                vec![
                    ("mediaFeatures.forced-colors", json!("active")),
                    ("cssMedia.forced-colors", json!("active")),
                    ("navigator.userAgent", json!("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36")),
                    ("navigator.platform", json!("Linux x86_64")),
                    ("userAgentData.platform", json!("Linux")),
                ],
            ),
            (
                "touch_vs_device",
                vec![("features.onTouchStart", json!(true))],
            ),
        ];

        for (rule, changes) in cases {
            let mut raw = probe();
            let described = format!("{changes:?}");
            for (path, value) in changes {
                set(&mut raw, path, value);
            }
            let rules = rules(&raw);
            assert!(
                rules.contains(&rule),
                "{rule} did not fire on {described}: {rules:?}"
            );
        }
    }

    #[test]
    fn a_webgl_renderer_from_another_system_than_the_user_agent_is_a_contradiction() {
        let mut raw = probe();
        set(
            &mut raw,
            "webgl.unmaskedRenderer",
            json!("ANGLE (Apple, ANGLE Metal Renderer: Apple M1, Unspecified Version)"),
        );
        set(
            &mut raw,
            "webgl.unmaskedVendor",
            json!("Google Inc. (Apple)"),
        );

        let found = found(&raw);
        let gpu = found.iter().find(|c| c.rule == "webgl_vs_os").unwrap();
        assert_eq!(gpu.surfaces[0], "navigator.userAgent");
        assert!(gpu.reason.contains("Windows"), "{gpu}");
        assert!(
            !found.iter().any(|c| c.rule == "webgpu_vs_webgl"),
            "WebGL and WebGPU now name the same maker: {found:#?}"
        );
    }

    #[test]
    fn a_safari_persona_that_plays_ogg_and_prints_eval_like_v8_claims_another_engine() {
        let mut raw = probe();
        set(
            &mut raw,
            "navigator.userAgent",
            json!("Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1"),
        );
        set(&mut raw, "navigator.platform", json!("iPhone"));
        set(&mut raw, "navigator.vendor", json!("Apple Computer, Inc."));
        without(&mut raw, &["userAgentData"]);

        let rules = rules(&raw);
        for expected in ["codecs_vs_browser", "engine_vs_user_agent"] {
            assert!(
                rules.contains(&expected),
                "{expected} is missing from {rules:?}"
            );
        }
    }
}
