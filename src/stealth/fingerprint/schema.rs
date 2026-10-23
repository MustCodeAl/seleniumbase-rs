//! The JSON Schema of the v2 fingerprint file, `schemas/fingerprint.schema.json`.
//!
//! The schema is written from the types (`schemars`, derived only in these tests),
//! so it cannot drift from them without a test failing. Other tools, such as the
//! companion browser extension, validate files against the committed copy.
//!
//! `SB_UPDATE_SCHEMAS=1 cargo test -p seleniumbase-rs --lib stealth::fingerprint::schema`
//! rewrites the file after a change to a setting.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{json, Value};

use super::compare::SETTINGS;
use super::Fingerprint;

/// The settings that have an off state; `Fingerprint::build` refuses `off` for the rest.
const OFF_SETTINGS: [&str; 8] = [
    "geolocation",
    "webrtc",
    "media_devices",
    "battery",
    "bluetooth",
    "speech_voices",
    "connection",
    "client_hints",
];

/// The settings with no value to draw; `build` refuses `random` for them.
const NO_RANDOM: [&str; 3] = ["proxy", "oscpu", "build_id"];

/// What a string value of a setting may not be: the three keywords, in any case, because
/// the file reads a string spelled like one as the keyword.
const RESERVED_WORDS: &str =
    "^([Nn][Aa][Tt][Uu][Rr][Aa][Ll]|[Rr][Aa][Nn][Dd][Oo][Mm]|[Oo][Ff][Ff])$";

/// Where the committed schema lives.
fn schema_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("schemas")
        .join("fingerprint.schema.json")
}

/// The first paragraph of a doc comment on one line, without Markdown links.
fn first_paragraph(text: &str) -> String {
    let paragraph = text.split("\n\n").next().unwrap_or(text);
    let line = paragraph.split_whitespace().collect::<Vec<_>>().join(" ");
    let links = regex::Regex::new(r"\[([^\]]+)\]\([^)]*\)").expect("a valid pattern");
    let bare = regex::Regex::new(r"\[(`[^`\]]+`)\]").expect("a valid pattern");
    let line = links.replace_all(&line, "$1");
    bare.replace_all(&line, "$1").into_owned()
}

/// The schema with its keys in a fixed order, its descriptions short, and the
/// rules the types cannot state themselves: which settings accept `off` and which
/// accept `random`.
fn tidy(value: Value, key: Option<&str>) -> Value {
    match value {
        Value::Object(map) => {
            let sorted: BTreeMap<String, Value> = map
                .into_iter()
                .map(|(name, inner)| {
                    let inner = tidy(inner, Some(&name));
                    (name, inner)
                })
                .collect();
            Value::Object(sorted.into_iter().collect())
        }
        Value::Array(items) => Value::Array(items.into_iter().map(|v| tidy(v, None)).collect()),
        Value::String(text) if key == Some("description") => Value::String(first_paragraph(&text)),
        other => other,
    }
}

/// The schema of `Fingerprint` as the file holds it. It is made once: every test reads it.
fn generated() -> &'static Value {
    static SCHEMA: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    SCHEMA.get_or_init(|| {
        let schema = schemars::schema_for!(Fingerprint);
        let mut root = serde_json::to_value(&schema).expect("a schema is JSON");
        restrict_keywords(&mut root);
        tidy(root, None)
    })
}

/// Takes `off` away from the settings that have none, and `random` from those with
/// nothing to draw, so the schema refuses what `Fingerprint::build` refuses, and keeps a
/// string value from being spelled like a keyword, which the file would read as one.
fn restrict_keywords(root: &mut Value) {
    let Some(fields) = find_properties(root) else {
        return;
    };
    for (name, setting) in fields.iter_mut() {
        let allowed: Vec<&str> = ["natural", "random", "off"]
            .into_iter()
            .filter(|word| match *word {
                "off" => OFF_SETTINGS.contains(&name.as_str()),
                "random" => !NO_RANDOM.contains(&name.as_str()),
                _ => true,
            })
            .collect();
        let Some(alternatives) = setting.get_mut("anyOf").and_then(Value::as_array_mut) else {
            continue;
        };
        if let Some(keywords) = alternatives
            .first_mut()
            .filter(|first| first.get("enum").is_some())
        {
            keywords["enum"] = json!(allowed);
        }
        for alternative in alternatives.iter_mut().skip(2) {
            if *alternative == json!({ "type": "string" }) {
                *alternative = json!({ "type": "string", "not": { "pattern": RESERVED_WORDS } });
            }
        }
    }
}

/// The `properties` of the schema of the settings.
fn find_properties(root: &mut Value) -> Option<&mut serde_json::Map<String, Value>> {
    // The root holds them, or a definition it points at does.
    if root
        .get("properties")
        .is_some_and(|p| p.get("user_agent").is_some())
    {
        return root.get_mut("properties")?.as_object_mut();
    }
    root.get_mut("$defs")?
        .as_object_mut()?
        .values_mut()
        .find(|definition| {
            definition
                .get("properties")
                .is_some_and(|p| p.get("user_agent").is_some())
        })?
        .get_mut("properties")?
        .as_object_mut()
}

#[test]
fn the_committed_schema_is_the_schema_of_the_types() {
    let mut text = serde_json::to_string_pretty(generated()).expect("a schema is JSON");
    text.push('\n');
    let path = schema_path();
    if std::env::var_os("SB_UPDATE_SCHEMAS").is_some() {
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("make schemas/");
        std::fs::write(&path, &text).expect("write the schema");
        return;
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed == text,
        "schemas/fingerprint.schema.json is out of date with the types. Rewrite it with:\n  \
         SB_UPDATE_SCHEMAS=1 cargo test -p seleniumbase-rs --lib stealth::fingerprint::schema"
    );
}

// ---------------------------------------------------------------------------
// A validator for the part of JSON Schema the file uses.
// ---------------------------------------------------------------------------

/// Why `instance` is not valid against `schema`, one line per violation.
fn violations(root: &Value, schema: &Value, instance: &Value, at: &str) -> Vec<String> {
    let mut found = Vec::new();
    check(root, schema, instance, at, &mut found);
    found
}

fn check(root: &Value, schema: &Value, instance: &Value, at: &str, found: &mut Vec<String>) {
    if let Some(Value::Bool(allowed)) = Some(schema) {
        if !allowed {
            found.push(format!("{at}: nothing is allowed here"));
        }
        return;
    }
    if let Some(excluded) = schema.get("not") {
        if violations(root, excluded, instance, at).is_empty() {
            found.push(format!("{at}: {instance} is not allowed"));
        }
    }
    if let (Some(pattern), Some(text)) = (
        schema.get("pattern").and_then(Value::as_str),
        instance.as_str(),
    ) {
        let matches = regex::Regex::new(pattern).is_ok_and(|pattern| pattern.is_match(text));
        if !matches {
            found.push(format!("{at}: {text:?} does not match {pattern}"));
        }
    }
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        let target = reference
            .strip_prefix('#')
            .and_then(|pointer| root.pointer(pointer));
        match target {
            Some(target) => check(root, target, instance, at, found),
            None => found.push(format!("{at}: cannot follow {reference}")),
        }
        return;
    }
    if let Some(options) = schema.get("anyOf").and_then(Value::as_array) {
        if !options
            .iter()
            .any(|option| violations(root, option, instance, at).is_empty())
        {
            found.push(format!(
                "{at}: matches none of the {} alternatives",
                options.len()
            ));
        }
        return;
    }
    if let Some(options) = schema.get("oneOf").and_then(Value::as_array) {
        let matching = options
            .iter()
            .filter(|option| violations(root, option, instance, at).is_empty())
            .count();
        if matching != 1 {
            found.push(format!(
                "{at}: matches {matching} of the alternatives, not one"
            ));
        }
        return;
    }
    if let Some(constant) = schema.get("const") {
        if constant != instance {
            found.push(format!("{at}: is not {constant}"));
        }
    }
    if let Some(words) = schema.get("enum").and_then(Value::as_array) {
        if !words.contains(instance) {
            found.push(format!("{at}: {instance} is not one of {words:?}"));
        }
    }
    if let Some(kinds) = schema.get("type") {
        let kinds: Vec<&str> = match kinds {
            Value::String(kind) => vec![kind.as_str()],
            Value::Array(kinds) => kinds.iter().filter_map(Value::as_str).collect(),
            _ => Vec::new(),
        };
        if !kinds.iter().any(|kind| is_of_type(instance, kind)) {
            found.push(format!("{at}: {instance} is not of type {kinds:?}"));
            return;
        }
    }
    if let (Some(minimum), Some(number)) = (
        schema.get("minimum").and_then(Value::as_f64),
        instance.as_f64(),
    ) {
        if number < minimum {
            found.push(format!("{at}: {number} is below {minimum}"));
        }
    }
    if let (Some(maximum), Some(number)) = (
        schema.get("maximum").and_then(Value::as_f64),
        instance.as_f64(),
    ) {
        if number > maximum {
            found.push(format!("{at}: {number} is above {maximum}"));
        }
    }
    if let Some(object) = instance.as_object() {
        let properties = schema.get("properties").and_then(Value::as_object);
        for required in schema
            .get("required")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            if !object.contains_key(required) {
                found.push(format!("{at}: {required} is required"));
            }
        }
        for (name, value) in object {
            let place = format!("{at}/{name}");
            match properties.and_then(|properties| properties.get(name)) {
                Some(property) => check(root, property, value, &place, found),
                None => match schema.get("additionalProperties") {
                    Some(Value::Bool(false)) => {
                        found.push(format!("{place}: is not a known field"))
                    }
                    Some(rule @ Value::Object(_)) => check(root, rule, value, &place, found),
                    _ => {}
                },
            }
        }
    }
    if let (Some(items), Some(list)) = (schema.get("items"), instance.as_array()) {
        for (index, item) in list.iter().enumerate() {
            check(root, items, item, &format!("{at}/{index}"), found);
        }
    }
}

fn is_of_type(instance: &Value, kind: &str) -> bool {
    match kind {
        "string" => instance.is_string(),
        "boolean" => instance.is_boolean(),
        "null" => instance.is_null(),
        "object" => instance.is_object(),
        "array" => instance.is_array(),
        "number" => instance.is_number(),
        "integer" => instance
            .as_f64()
            .is_some_and(|n| instance.is_number() && n.fract() == 0.0),
        _ => false,
    }
}

fn valid(file: &Value) -> bool {
    problems(file).is_empty()
}

fn problems(file: &Value) -> Vec<String> {
    let schema = generated();
    violations(schema, schema, file, "")
}

#[test]
fn every_preset_a_drawn_identity_and_a_capture_is_valid_against_the_schema() {
    use crate::stealth::fingerprint::OsType;

    let mut files = vec![
        Fingerprint::windows_desktop(),
        Fingerprint::edge_desktop(),
        Fingerprint::macos_desktop(),
        Fingerprint::linux_desktop(),
        Fingerprint::android_mobile(),
        Fingerprint::ios_mobile_safari(),
        Fingerprint::builder().build().unwrap(),
    ];
    for os in [
        OsType::Windows,
        OsType::Macos,
        OsType::Linux,
        OsType::Android,
        OsType::Ios,
    ] {
        files.push(Fingerprint::randomized(os, 17));
    }
    for fixture in [
        include_str!("../../../tests/fixtures/fingerprint/probe_chrome_macos.json"),
        include_str!("../../../tests/fixtures/fingerprint/probe_firefox_linux.json"),
        include_str!("../../../tests/fixtures/fingerprint/probe_safari_ios.json"),
        include_str!("../../../tests/fixtures/fingerprint/probe_chrome_real_headless.json"),
    ] {
        let raw: Value = serde_json::from_str(fixture).unwrap();
        let captured =
            crate::stealth::capture::CapturedFingerprint::from_probe(&raw, "2026-10-18T12:00:00Z")
                .unwrap();
        files.push(captured.fingerprint);
    }

    for fingerprint in files {
        let file = serde_json::to_value(&fingerprint).unwrap();

        let reasons = problems(&file);

        assert!(reasons.is_empty(), "{reasons:?} in {file}");
    }
}

#[test]
fn the_schema_accepts_the_words_and_the_values_of_a_setting() {
    for file in [
        json!({}),
        json!({ "timezone": "Europe/Berlin" }),
        json!({ "timezone": "random", "seed": 7 }),
        json!({ "timezone": "natural" }),
        json!({ "timezone": null }),
        json!({ "geolocation": "off", "webrtc": "off", "battery": "off" }),
        json!({ "geolocation": { "latitude": 52.5, "longitude": 13.4, "accuracy": 20 } }),
        json!({ "screen": { "width": 1920, "height": 1080 }, "pixel_ratio": 2 }),
        json!({ "fonts": ["Arial", "Calibri"], "bluetooth": true }),
        json!({ "media_devices": { "audio_inputs": 1, "audio_outputs": 1, "video_inputs": 0 } }),
        json!({ "policy": { "canvas_noise": "Persistent", "quic": "Disabled" } }),
        json!({ "browser_type": "mobile_safari", "os_type": "ios", "core_version": 155 }),
        json!({ "battery": { "charging": true, "level": 0.9, "charging_time": null } }),
    ] {
        let reasons = problems(&file);

        assert!(reasons.is_empty(), "{file} was refused: {reasons:?}");
    }
}

#[test]
fn the_schema_refuses_what_a_fingerprint_cannot_hold() {
    for file in [
        json!({ "screen_width": 1920 }),
        json!({ "timezone": 5 }),
        json!({ "timezone": "off" }),
        json!({ "proxy": "random" }),
        json!({ "hardware_concurrency": "many" }),
        json!({ "hardware_concurrency": -2 }),
        json!({ "screen": { "width": -1, "height": 5 } }),
        json!({ "screen": { "width": 1920 } }),
        json!({ "policy": { "canvas_noise": "loud" } }),
        json!({ "browser_type": "netscape" }),
        json!({ "fonts": "Arial" }),
        json!({ "seed": "seven" }),
        json!([]),
        json!("natural"),
    ] {
        assert!(!valid(&file), "{file} was accepted");
    }
}

#[test]
fn the_schema_and_the_loader_agree_on_which_settings_take_off_and_random() {
    for setting in SETTINGS {
        let off = json!({ setting: "off" });
        assert_eq!(
            valid(&off),
            Fingerprint::from_json(&off.to_string()).is_ok(),
            "off for {setting}: the schema says {:?}",
            problems(&off)
        );
        assert_eq!(valid(&off), OFF_SETTINGS.contains(&setting), "{setting}");
    }
    for setting in NO_RANDOM {
        let random = json!({ setting: "random" });
        assert!(!valid(&random), "{setting}");
        assert!(
            Fingerprint::from_json(&random.to_string()).is_err(),
            "{setting}"
        );
    }
}

#[test]
fn every_setting_of_the_file_is_in_the_schema_with_a_description() {
    let schema = generated();
    let mut root = schema.clone();
    let properties = find_properties(&mut root).expect("the schema has the settings");

    for setting in SETTINGS {
        let property = properties
            .get(setting)
            .unwrap_or_else(|| panic!("{setting} is missing"));
        let description = property
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("");
        assert!(!description.is_empty(), "{setting} has no description");
        assert!(
            !description.contains("](") && !description.contains('\n'),
            "{description}"
        );
    }
    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(schema["title"], "Fingerprint");
}
