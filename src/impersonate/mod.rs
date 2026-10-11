//! Browser-identical TLS and HTTP/2 through wreq and `BoringSSL`, behind the `impersonate` feature.
//!
//! The main types are re-exported here: the profile and the client (`ImpersonationProfile`,
//! `CustomProfileBuilder`, `impersonation_client`), plain HTTP that continues a browser's session
//! (`HttpSession`, `HttpOptions`, `HttpRequest`, `HttpReply`, `HttpError` and `Method`), the
//! loopback proxy for a real Chrome (`MitmProxy`, `MitmOptions`, `MitmError`, `MitmStats`), the
//! dual-mode session (`StealthSession`, `StealthSessionBuilder`, `StealthFetch`, `StealthError`)
//! and the in-memory CA (`EphemeralCa`).
//!
//! Five names of the HTTP session are generic, and are re-exported with a prefix so that they do
//! not read as something else at this level: `Attempt`, `Fetched`, `Rules` and `Verdict` of the
//! judging calls become `HttpAttempt`, `HttpFetched`, `HttpRules` and `HttpVerdict`, and the
//! reply `Headers` become `ReplyHeaders`. The original names stay in `http_session`.
//!
//! The stealth layer is experimental: its types may change between releases. It is measured on
//! macOS with Chrome 155 only.

#![forbid(unsafe_code)]

pub mod capture_h2;
pub mod client;
pub mod convert;
pub mod http_session;
pub mod mitm;
pub mod stealth_session;
pub mod tls_server;

pub use client::{
    impersonation_client, CustomProfileBuilder, ImpersonateError, ImpersonationProfile,
    WireProtocol,
};
pub use http_session::{
    Attempt as HttpAttempt, Fetched as HttpFetched, Headers as ReplyHeaders, HttpError,
    HttpOptions, HttpReply, HttpRequest, HttpSession, Method, ReplyProtocol, Rules as HttpRules,
    Verdict as HttpVerdict,
};
pub use mitm::{MitmError, MitmOptions, MitmProxy, MitmStats};
pub use stealth_session::{StealthError, StealthFetch, StealthSession, StealthSessionBuilder};
pub use tls_server::{CaOptions, EphemeralCa, Leaf, TlsServerError};
