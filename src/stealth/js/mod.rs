//! JavaScript that runs inside the page, kept as Rust strings.
//!
//! Each function returns a script written as the body of a function, which is
//! what WebDriver's Execute Script takes and what `return` needs. The pure CDP
//! engine evaluates expressions, so it wraps the body first:
//! `(function () { ... })()`.

pub mod active_css_js;
pub mod recorder_js;
