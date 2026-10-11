//! High-level helpers for the optional Playwright-compatible automation mode.
//!
//! These helpers are thin wrappers around [`crate::browser::playwright::PlaywrightSession`]
//! and are intended to be called from [`crate::BaseCase`] when
//! `activate_playwright_mode` has been used. The session is powered by the
//! [`rustwright`](https://github.com/Skyvern-AI/rustwright) native Rust
//! CDP engine rather than the Node Playwright driver.
//!
//! Every helper has a function for the whole life of a session: launch it,
//! drive it, and close it with [`pw_close`].
//!
//! # Examples
//!
//! ```no_run
//! use seleniumbase_rs::api::playwright::{
//!     launch_playwright_headless, pw_click, pw_close, pw_get_text, pw_goto,
//! };
//!
//! # async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! let session = launch_playwright_headless().await?;
//! pw_goto(&session, "https://seleniumbase.io/simple/login").await?;
//! pw_click(&session, "button").await?;
//! println!("{}", pw_get_text(&session, "h1").await?);
//! pw_close(&session).await?;
//! # Ok(())
//! # }
//! ```

use std::path::Path;

pub use crate::browser::playwright::PlaywrightSession;
use crate::error::SeleniumBaseError;
use serde_json::Value;

/// Convenience wrapper that launches a headed stealth Chromium session.
///
/// Prefer using [`BaseCase::activate_playwright_mode`](crate::BaseCase::activate_playwright_mode)
/// in test code so that the session lifetime is tied to the test case.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::BrowserLaunch`] if Chromium cannot be started.
pub async fn launch_playwright() -> Result<PlaywrightSession, SeleniumBaseError> {
    PlaywrightSession::launch().await
}

/// Convenience wrapper that launches a headless stealth Chromium session.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::BrowserLaunch`] if Chromium cannot be started.
pub async fn launch_playwright_headless() -> Result<PlaywrightSession, SeleniumBaseError> {
    PlaywrightSession::launch_headless().await
}

/// Navigates the active Playwright page to `url`.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::Navigation`] if the page cannot be loaded.
pub async fn pw_goto(session: &PlaywrightSession, url: &str) -> Result<(), SeleniumBaseError> {
    session.goto(url).await
}

/// Opens a new page in the session and makes it the active one.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::Playwright`] if the page cannot be created.
pub async fn pw_new_page(session: &mut PlaywrightSession) -> Result<(), SeleniumBaseError> {
    session.new_page().await
}

/// Clicks an element in the active Playwright page.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::WaitTimeout`] if the element does not become
/// clickable in time, [`SeleniumBaseError::ElementNotFound`] if nothing
/// matches, or [`SeleniumBaseError::ElementNotInteractable`] if it is hidden.
pub async fn pw_click(
    session: &PlaywrightSession,
    selector: &str,
) -> Result<(), SeleniumBaseError> {
    session.click(selector).await
}

/// Types text into an element in the active Playwright page.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::WaitTimeout`] if the field does not become
/// fillable in time, [`SeleniumBaseError::ElementNotFound`] if nothing
/// matches, or [`SeleniumBaseError::ElementNotInteractable`] if it is hidden.
pub async fn pw_type_text(
    session: &PlaywrightSession,
    selector: &str,
    text: &str,
) -> Result<(), SeleniumBaseError> {
    session.type_text(selector, text).await
}

/// Reads the visible text of an element in the active Playwright page.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::WaitTimeout`] if the element does not appear
/// in time, or [`SeleniumBaseError::ElementNotFound`] if nothing matches.
pub async fn pw_get_text(
    session: &PlaywrightSession,
    selector: &str,
) -> Result<String, SeleniumBaseError> {
    session.get_text(selector).await
}

/// Evaluates a JavaScript expression in the active Playwright page.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::Playwright`] if the script throws.
pub async fn pw_evaluate(
    session: &PlaywrightSession,
    expression: &str,
) -> Result<Value, SeleniumBaseError> {
    session.evaluate(expression).await
}

/// Takes a screenshot of the active Playwright page.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::Screenshot`] if the page cannot be captured
/// or the file cannot be written.
pub async fn pw_screenshot(
    session: &PlaywrightSession,
    path: &Path,
) -> Result<(), SeleniumBaseError> {
    session.screenshot(path).await
}

/// Closes the browser and ends the session.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::Playwright`] if the browser does not close
/// cleanly.
pub async fn pw_close(session: &PlaywrightSession) -> Result<(), SeleniumBaseError> {
    session.close().await
}
