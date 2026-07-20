//! High-level helpers for the optional Playwright-backed automation mode.
//!
//! These helpers are thin wrappers around [`crate::browser::playwright::PlaywrightSession`]
//! and are intended to be called from [`crate::BaseCase`] when
//! `activate_playwright_mode` has been used.

use std::path::Path;

pub use crate::browser::playwright::PlaywrightSession;
use crate::error::SeleniumBaseError;
use serde_json::Value;

/// Convenience wrapper that launches a headed stealth Chromium session.
///
/// Prefer using [`BaseCase::activate_playwright_mode`](crate::BaseCase::activate_playwright_mode)
/// in test code so that the session lifetime is tied to the test case.
pub async fn launch_playwright() -> Result<PlaywrightSession, SeleniumBaseError> {
    PlaywrightSession::launch().await
}

/// Convenience wrapper that launches a headless stealth Chromium session.
pub async fn launch_playwright_headless() -> Result<PlaywrightSession, SeleniumBaseError> {
    PlaywrightSession::launch_headless().await
}

/// Navigates the active Playwright page to `url`.
pub async fn pw_goto(session: &PlaywrightSession, url: &str) -> Result<(), SeleniumBaseError> {
    session.goto(url).await
}

/// Clicks an element in the active Playwright page.
pub async fn pw_click(
    session: &PlaywrightSession,
    selector: &str,
) -> Result<(), SeleniumBaseError> {
    session.click(selector).await
}

/// Types text into an element in the active Playwright page.
pub async fn pw_type_text(
    session: &PlaywrightSession,
    selector: &str,
    text: &str,
) -> Result<(), SeleniumBaseError> {
    session.type_text(selector, text).await
}

/// Reads the visible text of an element in the active Playwright page.
pub async fn pw_get_text(
    session: &PlaywrightSession,
    selector: &str,
) -> Result<String, SeleniumBaseError> {
    session.get_text(selector).await
}

/// Evaluates a JavaScript expression in the active Playwright page.
pub async fn pw_evaluate(
    session: &PlaywrightSession,
    expression: &str,
) -> Result<Value, SeleniumBaseError> {
    session.evaluate(expression).await
}

/// Takes a screenshot of the active Playwright page.
pub async fn pw_screenshot(
    session: &PlaywrightSession,
    path: &Path,
) -> Result<(), SeleniumBaseError> {
    session.screenshot(path).await
}
