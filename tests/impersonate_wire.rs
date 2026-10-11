//! The wire proof of the impersonating client, and its capability probe.
//!
//! Every claim of `seleniumbase_rs::impersonate::client` is checked on a real TLS connection to a
//! loopback server of this crate (the ephemeral CA of `impersonate::tls_server`, trusted through
//! `tls_cert_store`; verification is never turned off). What arrives is read back with the crate's
//! own parsers (`wire::client_hello`, `wire::h2_frames`) and compared with the measurement:
//!
//! - [`measured`]: the profile built from the measured Chrome 155 on macOS sends a `ClientHello`
//!   whose spec equals the measured one minus exactly the known gaps (a gap that disappears fails
//!   the test), an HTTP/2 start whose Akamai string equals the measured one, and the headers in
//!   the measured order and spelling;
//! - [`custom`]: a custom JA3, `JA4_r`, `JA4_ro`, peetprint, Akamai string and header set each
//!   arrive exactly as given, and what cannot be sent never builds a client;
//! - [`probe`]: the capability probe. It builds a client for each id the seed lists in
//!   `wire::ids` claim and for the ids they refuse, and fails when a list says something the
//!   wire does not.
//!
//! The tests run on loopback only and need no browser, so they are not `#[ignore]`d.
#![cfg(feature = "impersonate")]

#[path = "impersonate_wire/custom.rs"]
mod custom;
#[path = "impersonate_wire/exchange.rs"]
mod exchange;
#[path = "impersonate_wire/measured.rs"]
mod measured;
#[path = "impersonate_wire/probe.rs"]
mod probe;
#[path = "impersonate_wire/server.rs"]
mod server;
