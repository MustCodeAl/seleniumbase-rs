//! Public test API: `BaseCase` and helpers for tours, charts, presentations,
//! deferred assertions, scenario management, and action recording.

pub mod base_case;
pub mod cdp_driver;
pub mod cdp_page;
pub mod chart;
pub mod deferred;
pub mod dialog;
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
