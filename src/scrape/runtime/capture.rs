//! Reads a browser's `ClientHello` and HTTP/2 frames off a local socket, for the measurement tools.
//!
//! The measured tables under `src/wire/measured` are never typed by hand. These modules produce them:
//!
//! - [`stream`]: a loopback listener ([`Capture`]) and the readers for the first bytes a client sends,
//!   whether that is a TLS `ClientHello`, an HTTP/1 request or an HTTP/2 connection preface.
//! - [`hints`]: a small web site ([`HintsServer`]) that makes a browser report its client hints, and
//!   the parsers that turn the report and the request headers into
//!   [`ClientHints`](crate::stealth::fingerprint::ClientHints).
//! - [`record`]: the JSON the tools write ([`MeasuredFile`]), with its provenance, fingerprints and raw bytes.
//! - [`chrome`]: drives a local Chrome through `sb_cdp` and returns what it sent.
//! - [`args`]: the command line the `capture_tls` and `capture_hints` examples share.
//!
//! A loopback listener cannot complete a TLS handshake without a certificate, so the TLS capture records
//! the `ClientHello` and drops the connection. The HTTP/2 capture that needs a certificate belongs to the
//! `impersonate` feature.

pub mod args;
pub mod chrome;
pub mod hints;
pub mod record;
pub mod stream;

use std::io;

use thiserror::Error;

use crate::error::SeleniumBaseError;
use crate::wire::client_hello::ClientHelloError;
use crate::wire::h2_frames::H2CaptureError;
use crate::wire::tls_spec::SpecError;

pub use chrome::{measure_hints, measure_tls, ChromeOptions};
pub use hints::{HintsMeasurement, HintsServer};
pub use record::{BrowserVersion, EntryId, HelloRecord, MeasuredFile, TlsMeasurement};
pub use stream::{Capture, CapturedH2c, CapturedHello, CapturedHttp1, Http1Request};

/// Why a capture failed.
///
/// Messages name the thing that was wrong and never carry captured header values, cookies or keys.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CaptureError {
    /// A socket or file operation failed.
    #[error("capture I/O failed: {0}")]
    Io(#[from] io::Error),
    /// The client sent nothing usable before the time ran out.
    #[error("timed out waiting for {what}")]
    Timeout {
        /// What was awaited.
        what: &'static str,
    },
    /// The client closed the connection before the first message was complete.
    #[error("the client closed the connection after {have} bytes")]
    Closed {
        /// The bytes that had arrived.
        have: usize,
    },
    /// The client sent more than the capture reads.
    #[error("the client sent more than {limit} bytes")]
    TooLarge {
        /// The limit that was exceeded.
        limit: usize,
    },
    /// The bytes are not a `ClientHello`.
    #[error("not a ClientHello: {0}")]
    Hello(#[from] ClientHelloError),
    /// The bytes are not an HTTP/2 connection start.
    #[error("not an HTTP/2 connection start: {0}")]
    H2(#[from] H2CaptureError),
    /// The bytes are not an HTTP/1 request.
    #[error("not an HTTP/1 request: {reason}")]
    Http {
        /// What was malformed.
        reason: &'static str,
    },
    /// Several hellos meant to show one client disagree in more than extension order.
    #[error("the captured hellos do not show one client: {0}")]
    Spec(#[from] SpecError),
    /// The hellos of one browser have different JA4 values, so the capture is not stable.
    #[error("the captured hellos have different JA4 values: {first} and {other}")]
    Unstable {
        /// The JA4 of the first hello.
        first: String,
        /// The JA4 of the hello that differs.
        other: String,
    },
    /// No hello arrived for a probe.
    #[error("no ClientHello arrived for {what}")]
    NoHello {
        /// The probe that sent nothing.
        what: &'static str,
    },
    /// A hex string is not valid hexadecimal.
    #[error("invalid hexadecimal in the captured record")]
    Hex,
    /// A JSON document could not be read or written.
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// A measured file exists but is not a JSON object.
    #[error("{path} is not a JSON object")]
    NotAnObject {
        /// The file.
        path: String,
    },
    /// The browser reported a product string this tool cannot read.
    #[error("unsupported browser product `{0}`")]
    Product(String),
    /// The browser reports an operating system this tool does not know.
    #[error("cannot tell the operating system from the user agent")]
    UnknownOs,
    /// The browser runs headless, so its user agent and hints are not a real browser's.
    #[error("the browser is headless; run it headed to measure its identity")]
    Headless,
    /// The hints the page reported disagree with the request headers the browser sent.
    #[error("the reported client hints disagree with the request header `{header}`")]
    HintsDisagree {
        /// The request header that disagrees.
        header: String,
    },
    /// The page reported nothing, or reported something that is not the expected JSON.
    #[error("the hints page reported {reason}")]
    HintsReport {
        /// What was wrong.
        reason: &'static str,
    },
    /// The browser does not offer `navigator.userAgentData`, so there are no client hints to measure.
    #[error("the browser has no navigator.userAgentData")]
    NoUserAgentData,
    /// The browser could not be started or driven.
    #[error("browser error: {0}")]
    Browser(#[from] SeleniumBaseError),
    /// The command line of a capture tool is wrong.
    #[error("{0}")]
    Usage(String),
}
