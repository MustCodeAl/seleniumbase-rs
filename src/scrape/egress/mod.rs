//! Egress: the proxy route a request takes, the rotation pool, the geo locale, and URL redaction.

#![forbid(unsafe_code)]

pub mod geo;
pub mod pool;
pub mod redact;
pub mod route;
