//! Wire-level fingerprints: TLS and HTTP/2 specs measured from a local browser, and the `BrowserKind` they come from.

#![forbid(unsafe_code)]

pub mod akamai;
pub mod client_hello;
pub mod emulation;
pub mod h2_frames;
pub mod h2_spec;
pub mod header_spec;
pub mod ids;
pub mod ja3;
pub mod ja4;
pub mod tls_spec;

/// A browser whose wire behaviour the measured tables describe.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserKind {
    /// Google Chrome, the first browser measured (on macOS).
    Chrome,
}
