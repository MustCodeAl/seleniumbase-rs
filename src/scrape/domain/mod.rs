//! The pure domain layer: no tokio, no `std::fs`, `std::net` or `std::process`, and no clock reads.

#![forbid(unsafe_code)]

pub mod challenge;
pub mod cookie_jar;
pub mod egress;
pub mod events;
pub mod geo;
pub mod identity;
pub mod markers;
pub mod mitigation;
pub mod proxy_pool;
pub mod redact;
pub mod session_policy;
pub mod set_cookie;
pub mod state;
pub mod wire;
