//! The script that names the element a page has focused.

/// A script that returns a CSS selector for the element that has focus.
///
/// This is SeleniumBase's `get_active_element_css`: the answer is the shortest
/// selector that finds the element, such as `input#email` or
/// `button[aria-label="Close"]`, and `body` when nothing is focused. It is
/// written as the body of a function, the form WebDriver's Execute Script
/// takes, and ends in a `return`. To run it where an expression is needed,
/// wrap it: `(function () { ... })()`.
///
/// The selector is checked against the page before it is returned. Apart from
/// SeleniumBase's own `tag:contains("text")` form, handing it to
/// `document.querySelector` gives back the focused element.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::evasion::js::active_css_js::ACTIVE_CSS_JS;
///
/// let script = ACTIVE_CSS_JS;
/// assert!(script.trim_end().ends_with("return sbBestSelector(document.activeElement);"));
/// ```
pub const ACTIVE_CSS_JS: &str = concat!(
    include_str!("best_selector.js"),
    "\nreturn sbBestSelector(document.activeElement);\n"
);
