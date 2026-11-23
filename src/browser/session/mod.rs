//! `BrowserSession`: a WebDriver session, its optional CDP client and its
//! optional auto-started driver process.
//!
//! The struct and its accessors live here; the methods are grouped by concern
//! in the submodules, each an `impl BrowserSession` block:
//!
//! * `connect`: starting a session, a leased one, or a direct CDP attachment;
//! * `lifecycle`: `reconnect`, `quit` and the cleanup on drop;
//! * `launch`: what is done before a WebDriver session exists (the driver, the
//!   browser program, the virtual screen);
//! * `capabilities`: the capabilities each browser is asked for;
//! * `navigation`, `elements`, `waits`, `page`: what a test does in the page;
//! * `cdp`, `fingerprint`, `emulation`: DevTools, the fingerprint, the ad block
//!   and the phone emulation;
//! * `plugins`: browser plugins (feature `plugins`).

mod bidi;
mod browser_session;
mod capabilities;
mod cdp;
mod connect;
mod dialogs;
mod elements;
mod emulation;
mod fingerprint;
mod launch;
mod lifecycle;
mod navigation;
mod page;
#[cfg(feature = "plugins")]
mod plugins;
mod pure_cdp;
mod waits;

pub use bidi::{BidiNetworkEvent, DEFAULT_BIDI_BUFFER_SIZE};
pub use browser_session::BrowserSession;

#[cfg(feature = "plugins")]
pub(crate) use emulation::blocked_patterns;
pub(crate) use pure_cdp::PageSlot;

pub(crate) use dialogs::{DialogWatch, PureDialogs};

pub(crate) use launch::server_url_for_joins;
pub(crate) use waits::shown_text;
