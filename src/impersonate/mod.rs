//! Browser-identical TLS and HTTP/2 through wreq and `BoringSSL`, behind the `impersonate` feature.

#![forbid(unsafe_code)]

pub mod capture_h2;
pub mod client;
pub mod convert;
pub mod http_session;
pub mod mitm;
pub mod stealth_session;
pub mod tls_server;
