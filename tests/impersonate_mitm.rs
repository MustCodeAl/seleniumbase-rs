//! The MITM proxy against loopback sites, an egress proxy and raw clients; no browser.
//!
//! - [`access`]: the proxy token (`407`), targets it cannot intercept (empty `502`, never a blind
//!   tunnel), the SNI check, the leaf cache, and the bounds on heads, connections and idle time;
//! - [`forwarding`]: what reaches the origin (the measured JA4 and Akamai string, the browser's
//!   header order without hop-by-hop or proxy headers, the measured `priority`), the refusals
//!   (untrusted upstream certificate, version mismatch, upgrades, `Host` mismatch, body limit),
//!   streaming of a 64 MiB body, and absolute-form `http://` requests;
//! - [`lifecycle`]: swapping the egress between requests on one listener, shutdown and drop
//!   leaving no task, and the static check that the module writes no files.
//!
//! The real-Chrome proof is `tests/impersonate_mitm_chrome.rs`.
#![cfg(feature = "impersonate")]

#[path = "impersonate_mitm/access.rs"]
mod access;
#[path = "impersonate_mitm/forwarding.rs"]
mod forwarding;
#[path = "impersonate_mitm/lifecycle.rs"]
mod lifecycle;
#[path = "impersonate_mitm/support.rs"]
mod support;
