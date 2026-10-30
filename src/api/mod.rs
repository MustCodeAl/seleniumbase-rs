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
//! | [`pdf`] | PDF text extraction (feature `pdf`) |
//! | [`chart`], [`presentation`] | Chart.js and reveal.js pages |
//! | [`tour`], [`master_qa`] | Website tours and manual verification sessions |
//! | [`call`] | [`CallFuture`](call::CallFuture), the future of a call that takes Python's keyword arguments |
//! | [`membership`] | [`Contains`](membership::Contains), what `assert_in` can look in: a list or a string |
//! | [`switching`] | The calls that switch into frames, windows and alerts: [`FrameRef`](switching::FrameRef), [`WindowRef`](switching::WindowRef) and the [`Alert`](switching::Alert) handle |
//! | [`demo`], [`messages`], [`jqc`] | Demo mode and the highlight family, Messenger alerts and jquery-confirm dialog boxes |
//!
//! Some modules are behind Cargo features, because they pull in platform
//! libraries or large dependency trees that a headless test run does not need:
//!
//! | Feature | Module | What it is |
//! |---------|--------|------------|
//! | `gui` | `dialog` | Native message, confirm, text-prompt and file dialogs |
//! | `gui` | `gui` | Mouse and keyboard input at the OS level |
//! | `playwright` | `playwright` | Helpers for the Playwright-compatible engine |
//! | `pdf` | `pdf` | PDF text extraction, and `BaseCase::get_pdf_text` / `assert_pdf_text` |

pub mod base_case;
pub mod call;
pub mod cdp_driver;
pub mod cdp_page;
pub mod chart;
pub mod deferred;
pub mod demo;
#[cfg(feature = "gui")]
pub mod dialog;
#[cfg(feature = "gui")]
pub mod gui;
pub mod html;
pub mod html_inspector;
pub mod jqc;
pub mod master_qa;
pub mod membership;
pub mod messages;
pub mod misc_calls;
pub(crate) mod page_scripts;
#[cfg(feature = "pdf")]
pub mod pdf;
pub mod presentation;
pub mod recorder;
pub mod run_settings;
pub mod runner;
pub mod scenario;
pub mod switching;

pub mod tour;
pub mod traits;

#[cfg(feature = "playwright")]
pub mod playwright;
