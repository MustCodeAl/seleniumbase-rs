//! The dual-mode `StealthSession`, with no Chrome.
//!
//! The HTTP leg is the real one: an `HttpSession` against a loopback TLS shop that trusts only its
//! own ephemeral authority. The browser leg is scripted (`Browser::new_mocked`), so the traffic of
//! a browser is not made, but everything around it is: the MITM proxy is started and the browser
//! is launched behind it with the fingerprint of the identity. `tests/stealth_session_chrome.rs`
//! runs the same story with the real Chrome.
//!
//! - `flows`: every transition of the state machine and the events that tell of it.
//! - `identity`: export and restore, rotating the profile and the egress, and secrets.
//! - `rotation`: the order of a rotation in the browser (the old Chrome is closed first) and a
//!   rotation whose new Chrome cannot start.
#![cfg(all(feature = "impersonate", feature = "test-util"))]

#[path = "stealth_session/browser.rs"]
mod browser;
#[path = "stealth_session/flows.rs"]
mod flows;
#[path = "stealth_session/identity.rs"]
mod identity;
#[path = "stealth_session/rotation.rs"]
mod rotation;
#[path = "stealth_session/site.rs"]
mod site;
#[path = "stealth_session/world.rs"]
mod world;
