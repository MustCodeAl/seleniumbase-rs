//! The built-in steps: SeleniumBase's behave steps, ready for a feature file.
//!
//! Python's `behave` package ships a library of steps (`sbase/steps.py`) so a
//! feature file can drive a browser without any step code. This module is the
//! same library for [rstest-bdd](https://docs.rs/rstest-bdd): the 98 actions
//! of the Python library, each under every wording it has there.
//!
//! ```text
//! # tests/features/login.feature
//! Feature: Login
//!   Scenario: Valid login
//!     Given Open "https://example.com/login"
//!     When Type "demo" into "#user"
//!     And Click "button#sign-in"
//!     Then Assert text "Welcome" in "h1"
//! ```
//!
//! ```rust,ignore
//! // tests/login.rs: no step code at all.
//! use rstest_bdd_macros::scenario;
//! use seleniumbase_rs::testing::bdd::{sb, SbCase};
//!
//! #[scenario(path = "tests/features/login.feature", name = "Valid login")]
//! #[tokio::test(flavor = "multi_thread")]
//! async fn valid_login(#[future(awt)] sb: SbCase) {}
//! ```
//!
//! The steps use the `sb` fixture, so they need nothing else: launching an
//! [`SbCase`] links them into the test binary (see [`link`]). Your own steps
//! can sit beside them in the same test crate.
//!
//! # Wording
//!
//! As in Python, a step has no keyword of its own: `Given`, `When`, `Then`,
//! `And` and `But` all reach the same step. A quoted argument is written in
//! double or single quotes (`{name:string}` in the patterns), so
//! `Open "https://a.test"` and `Open 'https://a.test'` are the same step, and
//! so is `User opens URL "https://a.test"`. Text arguments understand the
//! escapes Python's `normalize_text` does: `\n`, `\t`, `\"`, `\'` and `\\`.
//!
//! # The actions
//!
//! | Module | Actions |
//! |---|---|
//! | `nav` | open, refresh, back and forward, new windows and drivers, alerts, cookies and storage, downloads |
//! | `elements` | click, type, select, hover, drag, check boxes, keys, JavaScript and jQuery variants |
//! | `asserts` | assertions about elements, text, the title and the URL, and the waits |
//! | `deferred` | deferred assertions and `Process deferred asserts` |
//! | `frames` | iframes and their content |
//! | `misc` | sleep, screenshots, page source and PDF, demo mode, CDP mode |
//!
//! [`builtin_steps`] lists every action with its patterns.
//!
//! # Differences from Python
//!
//! - The steps act on an [`SbCase`] (WebDriver). A pure CDP scenario
//!   (`SbCdpCase`) has no built-in steps.
//! - A waiting step waits [`WAIT_SECONDS`], Python's `LARGE_TIMEOUT`.
//! - `Download '...' to downloads` takes the file name from the end of the
//!   URL, because the browser saves it under that name.

use std::path::Path;
use std::sync::LazyLock;

use rstest_bdd::{extract_placeholders, StepPattern};

#[cfg(doc)]
use super::SbCase;

/// Defines built-in steps: one function per action, registered for `Given`,
/// `When` and `Then` under every pattern of the action. The action is named as
/// in Python's `steps.py`, so the tests can compare them.
///
/// A module calls this once: it also defines the module's `STEPS` table and
/// its `LINK` marker.
macro_rules! sb_steps {
    // The action a function stands for: its own name, or the one after `as`.
    (@action $name:ident) => {
        stringify!($name)
    };
    (@action $name:ident $action:ident) => {
        stringify!($action)
    };

    ($(
        $(#[$meta:meta])*
        fn $name:ident $(as $action:ident)? ($sb:ident $(, $arg:ident : $ty:ty)* $(,)?) $body:block
        $($pat:tt),+ $(,)?;
    )*) => {
        $(
            $(#[$meta])*
            $(
                #[::rstest_bdd_macros::given($pat)]
                #[::rstest_bdd_macros::when($pat)]
                #[::rstest_bdd_macros::then($pat)]
            )+
            async fn $name($sb: &mut $crate::testing::bdd::SbCase $(, $arg: $ty)*)
                -> $crate::Result<()> $body
        )*

        /// The actions of this module and their patterns.
        pub(super) const STEPS: &[(&str, &[&str])] =
            &[$((sb_steps!(@action $name $($action)?), &[$($pat),+])),*];

        /// A symbol of this module's object file, for [`link`](super::link).
        pub(super) static LINK: u8 = 0;
    };
}

mod asserts;
mod deferred;
mod elements;
mod frames;
mod misc;
mod nav;

/// Seconds a waiting step waits: Python's `LARGE_TIMEOUT`.
pub const WAIT_SECONDS: u64 = 10;

/// Seconds a download step waits for the file to appear.
const DOWNLOAD_SECONDS: u64 = 30;

/// Makes sure the built-in steps are part of the test binary.
///
/// The steps register themselves through `inventory`, and a linker may drop
/// an object file of a library that nothing refers to. Launching an
/// [`SbCase`] (the `sb` fixture does) calls this function, so a scenario that
/// uses the fixture needs nothing more; a test that looks the steps up
/// without a fixture can call it first.
#[inline(never)]
pub fn link() {
    std::hint::black_box([
        &nav::LINK,
        &elements::LINK,
        &asserts::LINK,
        &deferred::LINK,
        &frames::LINK,
        &misc::LINK,
    ]);
}

/// Every built-in step as `(action, pattern)`, in the order of the modules.
///
/// An action is a function of Python's `steps.py` and has one pattern for each
/// of its wordings once the quote styles are folded into `{name:string}`.
pub fn builtin_steps() -> impl Iterator<Item = (&'static str, &'static str)> {
    [
        nav::STEPS,
        elements::STEPS,
        asserts::STEPS,
        deferred::STEPS,
        frames::STEPS,
        misc::STEPS,
    ]
    .into_iter()
    .flatten()
    .flat_map(|&(action, patterns)| patterns.iter().map(move |&pattern| (action, pattern)))
}

/// The actions whose patterns match the text of a step, without its keyword.
///
/// A well-formed step matches exactly one action; an empty list means no
/// built-in step reads it, and more than one that its wording is ambiguous.
///
/// ```
/// # #[cfg(feature = "bdd")] {
/// use seleniumbase_rs::testing::bdd::steps::matching_actions;
///
/// assert_eq!(matching_actions("Click 'button#go'"), ["click_element"]);
/// assert!(matching_actions("Make coffee").is_empty());
/// # }
/// ```
pub fn matching_actions(text: &str) -> Vec<&'static str> {
    static PATTERNS: LazyLock<Vec<(&'static str, StepPattern)>> = LazyLock::new(|| {
        builtin_steps()
            .map(|(action, pattern)| (action, StepPattern::new(pattern)))
            .collect()
    });
    let text = text.trim();
    PATTERNS
        .iter()
        .filter(|(_, pattern)| extract_placeholders(pattern, text.into()).is_ok())
        .map(|&(action, _)| action)
        .collect()
}

/// Python's `normalize_text` of the behave steps: the escapes a feature file
/// may use in a text argument.
fn normalize_text(text: &str) -> String {
    text.replace("\\\\", "\\")
        .replace("\\t", "\t")
        .replace("\\n", "\n")
        .replace("\\\"", "\"")
        .replace("\\'", "'")
}

/// `value` without one quote at each end: the steps with a bare `{option}` or
/// `{attribute}` placeholder accept it quoted or not.
fn trim_quotes(value: &str) -> &str {
    let value = value.strip_prefix(['\'', '"']).unwrap_or(value);
    value.strip_suffix(['\'', '"']).unwrap_or(value)
}

/// `name` with the extension of a screenshot.
fn with_png(name: &str) -> String {
    if name.ends_with(".png") {
        name.to_owned()
    } else {
        format!("{name}.png")
    }
}

/// The file name a URL ends with, without its query or fragment.
fn file_name_of(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    Path::new(path).file_name().map_or_else(
        || "download".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_text_reads_the_escapes_of_python() {
        assert_eq!(normalize_text(r"a\nb\tc"), "a\nb\tc");
        assert_eq!(
            normalize_text(r#"say \"hi\" and \'bye\'"#),
            "say \"hi\" and 'bye'"
        );
        assert_eq!(normalize_text(r"back\\slash"), r"back\slash");
        assert_eq!(normalize_text("plain"), "plain");
    }

    #[test]
    fn a_bare_placeholder_loses_one_quote_at_each_end() {
        assert_eq!(trim_quotes("'class'"), "class");
        assert_eq!(trim_quotes("\"class\""), "class");
        assert_eq!(trim_quotes("class"), "class");
    }

    #[test]
    fn a_download_is_named_after_the_end_of_its_url() {
        assert_eq!(
            file_name_of("https://a.test/files/report.pdf?v=2#top"),
            "report.pdf"
        );
        assert_eq!(file_name_of("https://a.test/"), "a.test");
        assert_eq!(with_png("shot"), "shot.png");
        assert_eq!(with_png("shot.png"), "shot.png");
    }

    #[test]
    fn every_action_has_a_pattern_and_no_pattern_is_listed_twice() {
        let steps: Vec<_> = builtin_steps().collect();
        let mut patterns: Vec<_> = steps.iter().map(|&(_, pattern)| pattern).collect();
        patterns.sort_unstable();
        let before = patterns.len();
        patterns.dedup();
        assert_eq!(before, patterns.len(), "a pattern is listed twice");
        let mut actions: Vec<_> = steps.iter().map(|&(action, _)| action).collect();
        actions.dedup();
        assert_eq!(actions.len(), 98, "the actions of Python's steps.py");
    }
}
