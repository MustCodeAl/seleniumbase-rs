//! The MITM proxy against loopback sites, an egress proxy and raw clients; no browser.
//!
//! - [`access`]: the proxy token (`407`), targets it cannot intercept (empty `502`, never a blind
//!   tunnel), the SNI check, the leaf cache, and the bounds on heads, connections and idle time;
//! - [`destinations`]: the addresses a direct proxy will not connect to (loopback, private,
//!   link-local, metadata), named by a page or reached through a DNS answer;
//! - [`forwarding`]: what reaches the origin (the measured JA4 and Akamai string, the browser's
//!   header order without hop-by-hop or proxy headers, the measured `priority`), the refusals
//!   (untrusted upstream certificate, version mismatch, non-WebSocket upgrades, `Host` mismatch,
//!   body limit), streaming of a 64 MiB body, and absolute-form `http://` requests;
//! - [`websocket`]: a WebSocket through the proxy against the [`echo`] site (the echo, an
//!   independent client's handshake, the measured WebSocket JA4 upstream, the browser's order
//!   with only the upgrade pair of hop fields, an egress), and the hostile inputs (half
//!   upgrades, other protocols, a body, an oversize frame, an idle peer, loopback without the
//!   switch, a handshake outside a tunnel, a stopping proxy);
//! - [`socks`]: a SOCKS5 egress gets the host name to resolve, not an address looked up here;
//! - [`lifecycle`]: swapping the egress between requests on one listener, shutdown and drop
//!   leaving no task, and the static check that the module writes no files.
//!
//! The real-Chrome proof is `tests/impersonate_mitm_chrome.rs`.
#![cfg(feature = "impersonate")]

#[path = "impersonate_mitm/access.rs"]
mod access;
#[path = "impersonate_mitm/destinations.rs"]
mod destinations;
#[path = "impersonate_mitm/echo.rs"]
mod echo;
#[path = "impersonate_mitm/forwarding.rs"]
mod forwarding;
#[path = "impersonate_mitm/lifecycle.rs"]
mod lifecycle;
#[path = "impersonate_mitm/socks.rs"]
mod socks;
#[path = "impersonate_mitm/support.rs"]
mod support;
#[path = "impersonate_mitm/websocket.rs"]
mod websocket;
