//! One list of refused browser switches (ADR 005, follow-up 7).
//!
//! `evasion::evasions` refuses some switches from a fingerprint payload, and
//! the plugin API refuses a list of its own to every plugin
//! (`switch_policy::REFUSED_SWITCHES`). A switch the payload code refuses and
//! the plugin code lets through would be a way around the first list, so the
//! first must be a subset of the second. The payload list is a private
//! constant in `src/evasion/evasions.rs`; these tests read it from the source
//! text, which they do not change.

#![cfg(feature = "plugins")]

use seleniumbase_rs::evasion::evasions::permitted_switch;
use seleniumbase_rs::plugin_api::switch_policy::REFUSED_SWITCHES;

/// The names in the `FORBIDDEN_SWITCHES` constant of `source`.
///
/// # Panics
///
/// When `source` has no such constant or it does not end: the file changed
/// shape, and this test must be updated rather than pass on nothing.
fn forbidden_in(source: &str) -> Vec<String> {
    let declaration = source
        .find("const FORBIDDEN_SWITCHES")
        .expect("evasions.rs declares FORBIDDEN_SWITCHES");
    let rest = &source[declaration..];
    let open = rest.find("= &[").expect("the list opens with `= &[`") + "= &[".len();
    let close = rest[open..].find("];").expect("the list ends with `];`");
    let names = regex::Regex::new(r#""([^"\\]+)""#).expect("a valid pattern");
    rest[open..open + close]
        .lines()
        .map(|line| line.split("//").next().unwrap_or_default())
        .flat_map(|line| {
            names
                .captures_iter(line)
                .map(|found| found[1].to_owned())
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn the_payload_list_is_read_whole() {
    let found = forbidden_in(include_str!("../src/evasion/evasions.rs"));

    assert!(
        found.len() >= 15,
        "the payload list was read as {} names: {found:?}",
        found.len()
    );
    assert!(found.iter().any(|name| name == "renderer-cmd-prefix"));
    assert!(found.iter().any(|name| name == "host-resolver-rules"));
    for name in &found {
        assert!(
            !permitted_switch(name, ""),
            "`{name}` is read from the list, so the payload code must refuse it"
        );
    }
}

#[test]
fn every_switch_a_payload_may_not_set_is_refused_to_plugins_too() {
    let found = forbidden_in(include_str!("../src/evasion/evasions.rs"));

    let missing: Vec<&String> = found
        .iter()
        .filter(|name| !REFUSED_SWITCHES.contains(&name.as_str()))
        .collect();

    assert!(
        missing.is_empty(),
        "refused for a fingerprint payload but not for plugins: {missing:?}"
    );
}

#[test]
fn the_reader_follows_a_list_and_a_check_would_catch_a_stray_entry() {
    let source = r#"
        /// Docs with "quoted" words that are not in the list.
        const FORBIDDEN_SWITCHES: &[&str] = &[
            "no-sandbox",
            // "commented-out",
            "invented-switch", // trailing "remark"
        ];
        const OTHER: &[&str] = &["not-this-one"];
    "#;

    let found = forbidden_in(source);

    assert_eq!(found, ["no-sandbox", "invented-switch"]);
    assert!(
        found
            .iter()
            .any(|name| !REFUSED_SWITCHES.contains(&name.as_str())),
        "the subset check would fail for this list"
    );
}
