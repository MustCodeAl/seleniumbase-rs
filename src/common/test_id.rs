//! The id of the running test, which names its folder of logs and baselines.
//!
//! Python SeleniumBase calls a test by its pytest node id. Rust has no such
//! thing, but the test harness names the thread that runs a test after it
//! (`module::path::function`, and `function::case_1` for an `rstest` case), and
//! cargo names a test binary after its crate or test file. [`current_test_id`]
//! joins both into one plain file name: `cart.checkout.renders` for
//! `checkout::renders` in `tests/cart.rs`.
//!
//! The thread is read where the id is asked for, so ask on the thread the
//! harness started, before the test spawns any task: the `sb` fixture of
//! `seleniumbase_rs::testing` does that when it is built.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::common::test_id::test_id;
//!
//! assert_eq!(
//!     test_id("checkout::renders", Some("cart-0123456789abcdef")),
//!     "cart.checkout.renders"
//! );
//! assert_eq!(test_id("../../etc/passwd", None), "______etc_passwd");
//! ```

use std::ffi::OsStr;
use std::path::Path;

use crate::artifacts::replace_unsafe;

/// The longest test id; a longer one keeps its end, the most specific part.
const MAX_TEST_ID_LEN: usize = 120;

/// What an id is when nothing is left of a thread name.
const FALLBACK: &str = "default";

/// The id of the test running on the calling thread: the test binary's name
/// when it is one cargo built, then the module path and the test function, all
/// joined with `.`.
///
/// Each part keeps only ASCII letters, digits, `-` and `_`, so the id is one
/// plain file name, at most 120 characters (the end is kept), and `default`
/// when nothing is left, such as on a thread that has no name.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::common::test_id::current_test_id;
///
/// // Under `cargo test` this names the doctest's thread; anywhere it is a file name.
/// let id = current_test_id();
/// assert!(seleniumbase_rs::artifacts::bare_file_name(&id).is_ok());
/// ```
#[must_use]
pub fn current_test_id() -> String {
    let thread = std::thread::current();
    let exe = std::env::current_exe().ok();
    let binary = exe
        .as_deref()
        .and_then(Path::file_stem)
        .and_then(OsStr::to_str);
    test_id(thread.name().unwrap_or_default(), binary)
}

/// The id for the test run by the thread named `thread` in the program
/// `binary` (its file name without the extension).
///
/// See [`current_test_id`] for the shape. `binary` counts only when cargo built
/// it, which is when its name ends in a dash and a 16-digit hexadecimal hash.
#[must_use]
pub fn test_id(thread: &str, binary: Option<&str>) -> String {
    let parts: Vec<String> = binary
        .and_then(cargo_test_binary)
        .into_iter()
        .chain(thread.split("::"))
        .map(replace_unsafe)
        .filter(|part| !part.is_empty())
        .collect();
    let id = parts.join(".");
    let start = id.len().saturating_sub(MAX_TEST_ID_LEN);
    // Only ASCII is left, so any byte index is a character boundary.
    let id = id[start..].trim_start_matches('.');
    if id.is_empty() {
        FALLBACK.to_owned()
    } else {
        id.to_owned()
    }
}

/// The crate or test-file name of a test binary cargo built, whose file name is
/// that name, a dash and a 16-digit hexadecimal hash.
fn cargo_test_binary(stem: &str) -> Option<&str> {
    let (name, hash) = stem.rsplit_once('-')?;
    let is_hash = hash.len() == 16 && hash.bytes().all(|byte| byte.is_ascii_hexdigit());
    (is_hash && !name.is_empty()).then_some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_test_id_is_one_plain_file_name() {
        let binary = Some("cart-0123456789abcdef");
        assert_eq!(
            test_id("checkout::renders", binary),
            "cart.checkout.renders"
        );
        assert_eq!(test_id("renders", binary), "cart.renders");
        assert_ne!(
            test_id("renders", Some("shop-0123456789abcdef")),
            test_id("renders", binary),
            "same-named tests of two test files"
        );
        // Not a test binary cargo built: no name from it.
        assert_eq!(test_id("a::renders", Some("mytool")), "a.renders");
        assert_eq!(test_id("main", Some("tool-v1")), "main");
        assert_eq!(test_id("", None), "default");
        assert_eq!(test_id("../../etc/passwd", None), "______etc_passwd");
        assert_eq!(test_id("a::::b::", None), "a.b");
    }

    #[test]
    fn an_rstest_case_keeps_its_case_name() {
        assert_eq!(
            test_id(
                "login::valid_login::case_2",
                Some("sb_fixture-0123456789abcdef")
            ),
            "sb_fixture.login.valid_login.case_2"
        );
    }

    #[test]
    fn a_long_id_keeps_its_end() {
        let long = format!("{}::renders", "m".repeat(200));
        let id = test_id(&long, Some("cart-0123456789abcdef"));
        assert_eq!(id.len(), MAX_TEST_ID_LEN);
        assert!(id.ends_with(".renders"), "{id}");
        assert!(crate::artifacts::bare_file_name(&id).is_ok(), "{id}");
    }

    #[test]
    fn the_running_test_names_itself() {
        let id = current_test_id();
        assert!(
            id.ends_with("the_running_test_names_itself"),
            "the thread is named after the test: {id}"
        );
        assert!(crate::artifacts::bare_file_name(&id).is_ok(), "{id}");
    }
}
