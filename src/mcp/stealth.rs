//! This crate's own stealth tools, which need no browser session.
//!
//! They expose the fingerprint presets, the evasion registry and the
//! chromedriver patcher, so a model can inspect and prepare a profile before
//! starting a browser.

use serde_json::json;

use super::support::{json_output, nonempty};
use super::{Args, Ctx, Effect, Output, Prop, Schema, ToolDef, ToolError};
use crate::stealth::fingerprint::Fingerprint;
use crate::{ChromedriverPatcher, EnginePatch, EvasionContext};

const PRESETS: [&str; 5] = ["windows", "macos", "linux", "android", "ios"];

fn preset_prop() -> Prop {
    Prop::choice(&PRESETS, "Which built-in fingerprint profile")
}

fn preset(args: &Args) -> Result<Fingerprint, ToolError> {
    Ok(match args.choice("preset", &PRESETS, "windows")? {
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
    vec![
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
        ToolDef::new(
            "build_fingerprint",
            "Build Fingerprint",
            "Build a fingerprint profile from a preset, optionally overriding its user agent \
             and screen size.",
            Effect::Inspect,
            by_preset()
                .optional(
                    "user_agent",
                    Prop::string("Replace the preset's user agent"),
                )
                .optional(
                    "screen_width",
                    Prop::integer("Replace the screen width").min(1.0),
                )
                .optional(
                    "screen_height",
                    Prop::integer("Replace the screen height").min(1.0),
                ),
            build_fingerprint::<S>,
        ),
        ToolDef::new(
            "get_stealth_bootstrap_script",
            "Get Stealth Bootstrap Script",
            "Get the JavaScript evasion bootstrap for a fingerprint preset.",
            Effect::Inspect,
            by_preset(),
            get_stealth_bootstrap_script::<S>,
        ),
        ToolDef::new(
            "list_evasion_providers",
            "List Evasion Providers",
            "List the built-in stealth evasion providers in the order they are applied.",
            Effect::Inspect,
            Schema::new(),
            list_evasion_providers::<S>,
        ),
        ToolDef::new(
            "build_stealth_bootstrap",
            "Build Stealth Bootstrap",
            "Assemble the combined stealth bootstrap script for a preset from the evasion \
             provider registry.",
            Effect::Inspect,
            by_preset(),
            build_stealth_bootstrap::<S>,
        ),
        ToolDef::new(
            "validate_fingerprint",
            "Validate Fingerprint",
            "Check a preset's fingerprint for internal contradictions and report errors and \
             warnings.",
            Effect::Inspect,
            by_preset(),
            validate_fingerprint::<S>,
        ),
    ]
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

async fn build_fingerprint<S>(_ctx: Ctx<S>, args: Args) -> Result<Output, ToolError> {
    let mut fingerprint = preset(&args)?;
    if let Some(user_agent) = nonempty(&args, "user_agent")? {
        fingerprint.user_agent = Some(user_agent.to_owned());
    }
    for (name, field) in [
        ("screen_width", &mut fingerprint.screen_width),
        ("screen_height", &mut fingerprint.screen_height),
    ] {
        if let Some(value) = args.opt_usize(name)? {
            *field = Some(
                u32::try_from(value)
                    .ok()
                    .ok_or_else(|| ToolError::invalid(name, "too large"))?,
            );
        }
    }
    json_output(fingerprint)
}

async fn get_stealth_bootstrap_script<S>(_ctx: Ctx<S>, args: Args) -> Result<Output, ToolError> {
    Ok(crate::stealth::evasions::bootstrap_script(&preset(&args)?).into())
}

async fn list_evasion_providers<S>(_ctx: Ctx<S>, _args: Args) -> Result<Output, ToolError> {
    json_output(crate::default_registry().provider_names())
}

async fn build_stealth_bootstrap<S>(_ctx: Ctx<S>, args: Args) -> Result<Output, ToolError> {
    let fingerprint = preset(&args)?;
    let context = EvasionContext::new(&fingerprint);
    Ok(crate::default_registry().bootstrap(&context).into())
}

async fn validate_fingerprint<S>(_ctx: Ctx<S>, args: Args) -> Result<Output, ToolError> {
    let report = preset(&args)?.validate();
    Ok(json!({
        "coherent": report.is_coherent(),
        "errors": report.errors,
        "warnings": report.warnings,
    })
    .into())
}
