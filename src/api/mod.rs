//! Public test API: `BaseCase` and helpers for tours, charts, presentations,
//! deferred assertions, scenario management, and action recording.
//!
//! | Module | What it is |
//! |--------|------------|
//! | [`base_case`] | [`BaseCase`](base_case::BaseCase), the WebDriver-backed test API |
//! | [`traits`] | The capability traits `BaseCase` implements |
//! | [`runner`] | Test lifecycle helpers |
//! | [`cdp_driver`], [`cdp_page`] | Small helpers for a browser driven over CDP; prefer [`crate::sb_cdp`] |
//! | [`deferred`] | Soft assertions, reported together |
//! | [`scenario`] | JSON scenarios, replayed against a `BaseCase` |
//! | [`recorder`] | Recording of actions, Rust test generation and replay |
//! | [`html`], [`html_inspector`] | Parsing page source, and static HTML/accessibility checks |
//! | [`pdf`] | PDF text extraction |
//! | [`chart`], [`presentation`] | Chart.js and reveal.js pages |
//! | [`tour`], [`master_qa`] | Website tours and manual verification sessions |
//!
//! Three modules are behind Cargo features, because they pull in platform
//! libraries that a headless test run does not need:
//!
//! | Feature | Module | What it is |
//! |---------|--------|------------|
//! | `gui` | `dialog` | Native message, confirm, text-prompt and file dialogs |
//! | `gui` | `gui` | Mouse and keyboard input at the OS level |
//! | `playwright` | `playwright` | Helpers for the Playwright-compatible engine |

pub mod base_case;
pub mod cdp_driver;
pub mod cdp_page;
pub mod chart;
pub mod deferred;
#[cfg(feature = "gui")]
pub mod dialog;
#[cfg(feature = "gui")]
pub mod gui;
pub mod html;
pub mod html_inspector;
pub mod master_qa;
pub mod pdf;
pub mod presentation;
pub mod recorder;
pub mod runner;
pub mod scenario;
pub mod tour;
pub mod traits;

#[cfg(feature = "playwright")]
pub mod playwright;
