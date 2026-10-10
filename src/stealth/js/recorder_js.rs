//! The script that records what happens in a page.

/// A script that records the clicks, changes and form submissions in the
/// page into `window.recordedActions`.
///
/// Each entry holds the kind of action, a CSS selector for the element (the
/// same one [`get_active_css_js`](super::active_css_js::get_active_css_js)
/// gives), the page's URL, the time, and the new value for a change. Typed
/// passwords are not kept. The list is also stored in `sessionStorage`, so
/// running the script again on the next page of the same site carries the
/// recording on.
///
/// Running it again in a document that is already being recorded does
/// nothing and keeps what was recorded. It is written as the body of a
/// function, the form WebDriver's Execute Script takes, and returns the list.
/// To run it where an expression is needed, wrap it: `(function () { ... })()`.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::stealth::js::recorder_js::get_recorder_js;
///
/// assert!(get_recorder_js().contains("window.recordedActions"));
/// ```
#[must_use]
pub fn get_recorder_js() -> &'static str {
    concat!(
        include_str!("best_selector.js"),
        "\n",
        include_str!("recorder.js")
    )
}
