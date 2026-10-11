//! `HttpSession` against a loopback HTTPS site: the proof that the session sends what it claims
//! and refuses what it must.
//!
//! The site is the ephemeral CA and the TLS acceptor of `impersonate::tls_server` behind a small
//! HTTP/1.1 server, trusted by the client through `tls_cert_store`; certificate verification is
//! never turned off. The tests cover:
//!
//! - [`basics`]: the identity a request carries, the headers a caller may and may not set,
//!   bodies, events;
//! - [`redirects`]: cookies per hop, loops, cookies and headers that must not follow a redirect to
//!   another site, the methods a status keeps;
//! - [`cookies`]: public suffixes, other hosts, expiry on an injected clock, the identity's
//!   cookies in and out;
//! - [`limits`]: body size after decompression, time, pace, and failures that must not leak;
//! - [`judging`]: a challenge page, a rate limit and a clearance about to end, as the session
//!   policy sees them;
//! - [`h2`]: an origin that speaks HTTP/2 gets the headers of that protocol;
//! - [`proxy`]: the egress as a proxy, how its credentials travel, the bypass list;
//! - [`identity`]: an identity of another egress, another Chrome version, or an unmeasured system;
//! - [`page`]: a session continued from a page and the cookies written back, against the
//!   scripted browser of `sb_cdp` (needs the `test-util` feature).
//!
//! They run on loopback only and need no Chrome, so none is `#[ignore]`d. Run them with
//! `cargo test --features "impersonate test-util" --test impersonate_http`.
#![cfg(feature = "impersonate")]

#[path = "impersonate_http/basics.rs"]
mod basics;
#[path = "impersonate_http/cookies.rs"]
mod cookies;
#[path = "impersonate_http/h2.rs"]
mod h2;
#[path = "impersonate_http/identity.rs"]
mod identity;
#[path = "impersonate_http/judging.rs"]
mod judging;
#[path = "impersonate_http/limits.rs"]
mod limits;
#[cfg(feature = "test-util")]
#[path = "impersonate_http/page.rs"]
mod page;
#[path = "impersonate_http/proxy.rs"]
mod proxy;
#[path = "impersonate_http/redirects.rs"]
mod redirects;
#[path = "impersonate_http/site.rs"]
mod site;
