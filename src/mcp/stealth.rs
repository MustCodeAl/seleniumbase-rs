//! This crate's own stealth tools, which need no browser session.
//!
//! They expose the fingerprint presets, the evasion registry and the
//! chromedriver patcher, so a model can inspect and prepare a profile before
//! starting a browser.

use serde_json::json;

use super::support::json_output;
use super::{Args, Ctx, Effect, Output, Prop, Schema, ToolDef, ToolError};
use crate::stealth::fingerprint::Fingerprint;
use crate::{ChromedriverPatcher, EnginePatch};

const PRESETS: [&str; 6] = ["windows", "edge", "macos", "linux", "android", "ios"];

fn preset_prop() -> Prop {
    Prop::choice(&PRESETS, "Which built-in fingerprint profile")
}

fn preset(args: &Args) -> Result<Fingerprint, ToolError> {
    Ok(match args.choice("preset", &PRESETS, "windows")? {
        "edge" => Fingerprint::edge_desktop(),
        "macos" => Fingerprint::macos_desktop(),
        "linux" => Fingerprint::linux_desktop(),
        "android" => Fingerprint::android_mobile(),
        "ios" => Fingerprint::ios_mobile_safari(),
        _ => Fingerprint::windows_desktop(),
    })
}

/// The stealth tools, for a server whose session type is `S`.
pub(super) fn tools<S: Send + 'static>() -> Vec<ToolDef<S>> {
    let by_preset = || Schema::new().required("preset", preset_prop());
    Vec::from([
        ToolDef::new(
            "patch_chromedriver",
            "Patch Chromedriver",
            "Patch a chromedriver binary in place to remove the markers that identify it as \
             automation.",
            Effect::Overwrite,
            Schema::new()
                .required("path", Prop::string("Path of the chromedriver binary"))
                .optional(
                    "backup",
                    Prop::boolean("Keep a backup copy first").default(true),
                ),
            patch_chromedriver::<S>,
        ),
        ToolDef::new(
            "list_engine_spoofing_args",
            "List Engine Spoofing Args",
            "List the Chromium flags that reduce engine-level automation fingerprints.",
            Effect::Inspect,
            Schema::new(),
            list_engine_spoofing_args::<S>,
        ),
        ToolDef::new(
            "list_fingerprint_presets",
            "List Fingerprint Presets",
            "List the names of the built-in fingerprint presets.",
            Effect::Inspect,
            Schema::new(),
            list_fingerprint_presets::<S>,
        ),
        ToolDef::new("build_fingerprint", "Build Fingerprint", "Build a fingerprint profile from a preset or a random identity (random, seed, os), optionally overriding its user agent \
         and screen size.", Effect::Inspect, super::fingerprint::build_schema(), build_fingerprint::<S>),
        ToolDef::new(
            "get_stealth_bootstrap_script",
            "Get Stealth Bootstrap Script",
            "Get the JavaScript evasion bootstrap that a browser session runs for a fingerprint \
             preset: the evasion providers listed by list_evasion_providers, assembled in the \
             order they are applied.",
            Effect::Inspect,
            by_preset(),
            get_stealth_bootstrap_script::<S>,
        ),
        ToolDef::new(
            "list_evasion_providers",
            "List Evasion Providers",
            "List the stealth evasion providers a browser session runs, in the order they are \
             applied. Chrome answers for its own plugins and network report, so those are not \
             among them.",
            Effect::Inspect,
            Schema::new(),
            list_evasion_providers::<S>,
        ),
        ToolDef::new("validate_fingerprint", "Validate Fingerprint", "Check a fingerprint (given, from a file or a preset) for internal contradictions and report errors and \
         warnings.", Effect::Inspect, super::fingerprint::source_schema(), validate_fingerprint::<S>),
    ])
}

async fn patch_chromedriver<S>(_ctx: Ctx<S>, args: Args) -> Result<Output, ToolError> {
    let path = args.str("path")?;
    let mut spec = EnginePatch::all();
    spec.backup = args.bool_or("backup", true)?;
    ChromedriverPatcher::new(path).patch(spec)?;
    Ok(format!("Patched chromedriver at {path}").into())
}

async fn list_engine_spoofing_args<S>(_ctx: Ctx<S>, _args: Args) -> Result<Output, ToolError> {
    json_output(crate::engine_spoofing_args())
}

async fn list_fingerprint_presets<S>(_ctx: Ctx<S>, _args: Args) -> Result<Output, ToolError> {
    json_output(PRESETS)
}

async fn build_fingerprint<S: Send + 'static>(
    ctx: Ctx<S>,
    args: Args,
) -> Result<Output, ToolError> {
    super::fingerprint::build(ctx.settings(), &args)
}

async fn get_stealth_bootstrap_script<S>(_ctx: Ctx<S>, args: Args) -> Result<Output, ToolError> {
    Ok(crate::stealth::evasions::session_script(&preset(&args)?).into())
}

async fn list_evasion_providers<S>(_ctx: Ctx<S>, _args: Args) -> Result<Output, ToolError> {
    json_output(crate::session_registry(&Fingerprint::default()).provider_names())
}

async fn validate_fingerprint<S: Send + 'static>(
    ctx: Ctx<S>,
    args: Args,
) -> Result<Output, ToolError> {
    let fingerprint = super::fingerprint::fingerprint_arg(&args, ctx.settings())?;
    Ok(coherence_report(&fingerprint).into())
}

/// What `Fingerprint::validate` finds in `fingerprint`: whether it is
/// coherent, and the errors and warnings. The `sb` server's
/// `validate_fingerprint` and the `dev` server's `check_fingerprint` both
/// answer with it.
pub(super) fn coherence_report(fingerprint: &Fingerprint) -> serde_json::Value {
    let report = fingerprint.validate();
    json!({
        "coherent": report.is_coherent(),
        "errors": report.errors,
        "warnings": report.warnings,
    })
}
