//! The `capture_h2` capture: records a real browser's HTTP/2 frames and header block behind a local TLS acceptor.
//!
//! The TLS capture can stop at the `ClientHello`, but HTTP/2 only starts once a handshake has
//! completed. This tool therefore serves a small HTTPS site with the ephemeral CA of
//! [`tls_server`](super::tls_server), starts the local Chrome so that it trusts exactly that CA
//! (`--ignore-certificate-errors-spki-list`) and resolves `*.capture.test` to the site, and reads
//! what the browser sends:
//!
//! - an **HTTP/2 pass** (ALPN `h2`) records, for each of five probes on a connection of its own,
//!   the SETTINGS in order, the connection `WINDOW_UPDATE`, the PRIORITY frames, the priority
//!   fields of HEADERS, the pseudo-header order, and every header name and value in order, decoded
//!   by [`H2Preface`](crate::wire::h2_frames::H2Preface);
//! - an **HTTP/1.1 pass** (ALPN `http/1.1`) records the same requests as plain text, which shows
//!   the capitalisation the browser writes and whether it orders headers differently.
//!
//! The probes are a navigation, a `fetch`, an image, a script and a stylesheet ([`probes`]).
//! [`assemble::derive`] turns the evidence into an
//! [`Http2Spec`](crate::wire::h2_spec::Http2Spec) and a
//! [`HeaderSpec`](crate::wire::header_spec::HeaderSpec). [`store::MeasuredDoc`] adds them to the
//! measured JSON of the browser, next to the TLS and client-hint measurements, together with the
//! evidence they were derived from, and the raw HTTP/2 bytes become fixtures under
//! `tests/fixtures/measured`.
//!
//! Everything binds `127.0.0.1` only. The CA lives in memory for one run; no key is written.
//! The browser runs headed (a headless Chrome reports a different identity) with a fresh profile,
//! so no cookie or credential can reach the evidence, and a request that carries one is refused.
//!
//! ```text
//! cargo run --features impersonate --example capture_h2
//! cargo run --features impersonate --example capture_h2 -- --h1   # only the HTTP/1.1 pass
//! ```

pub mod assemble;
pub mod browser;
pub mod probes;
pub mod site;
pub mod store;

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use thiserror::Error;

pub use assemble::{derive, h1_evidence, h2_evidence, Derived, H1Evidence, H2Evidence};
pub use probes::{Probe, PROBES};
pub use site::{Observed, Protocol, Seen, Site};
pub use store::{write_h2_fixtures, MeasuredDoc};

use crate::impersonate::tls_server::TlsServerError;
use crate::scrape::runtime::capture::args::ToolArgs;
use crate::scrape::runtime::capture::record::{BrowserVersion, EntryId};
use crate::scrape::runtime::capture::CaptureError;
use crate::wire::h2_frames::H2CaptureError;
use crate::wire::h2_spec::SpecError;
use crate::wire::header_spec::HeaderSpecError;
use crate::wire::BrowserKind;

/// The flag that runs only the HTTP/1.1 pass.
pub const FLAG_H1: &str = "--h1";
/// The flag that runs only the HTTP/2 pass.
pub const FLAG_H2: &str = "--h2";
/// What `--help` adds to the options the capture tools share.
pub const USAGE_PASSES: &str = "\
  --h1             run only the HTTP/1.1 pass (default: both passes)
  --h2             run only the HTTP/2 pass";

/// Why the HTTP/2 and header capture failed.
///
/// Messages name what was wrong and never carry header values, cookies or key material.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CaptureH2Error {
    /// A socket, the browser or the shared capture readers failed.
    #[error(transparent)]
    Capture(#[from] CaptureError),
    /// The CA, a leaf certificate or the handshake failed.
    #[error(transparent)]
    Tls(#[from] TlsServerError),
    /// The bytes are not an HTTP/2 connection start.
    #[error("not an HTTP/2 connection start: {0}")]
    H2(#[from] H2CaptureError),
    /// The HTTP/2 spec could not be built from the capture.
    #[error("the HTTP/2 spec cannot be built: {0}")]
    Spec(#[from] SpecError),
    /// The header spec could not be built from the capture.
    #[error("the header spec cannot be built: {0}")]
    Headers(#[from] HeaderSpecError),
    /// A JSON document could not be read or written.
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// The browser asked for a host that is not one of the probes.
    #[error("the browser asked for a host that is not a capture probe")]
    UnknownHost,
    /// The `ClientHello` names no server.
    #[error("the ClientHello names no server")]
    NoServerName,
    /// The handshake settled on another protocol than the pass measures.
    #[error("the handshake did not settle on the protocol of this pass ({expected:?})")]
    NotNegotiated {
        /// The protocol the pass needs.
        expected: Protocol,
    },
    /// The browser never made a request to a probe.
    #[error("the browser never requested the `{probe}` probe")]
    MissingProbe {
        /// The probe.
        probe: &'static str,
    },
    /// A probe's request carries another `Sec-Fetch-Dest` than the probe is meant to produce.
    #[error("the `{probe}` probe arrived with Sec-Fetch-Dest `{found}`")]
    WrongDestination {
        /// The probe.
        probe: &'static str,
        /// The destination that arrived.
        found: String,
    },
    /// The connections of two probes start differently, so there is no single HTTP/2 spec.
    #[error("the `{probe}` connection starts differently from the navigation's")]
    ProbesDisagree {
        /// The probe that differs.
        probe: String,
    },
    /// Stored evidence does not say what it claims to.
    #[error("the evidence for `{probe}` is not usable: {reason}")]
    Evidence {
        /// The probe.
        probe: String,
        /// What is wrong.
        reason: &'static str,
    },
    /// A request carried a credential, which the evidence must never hold.
    #[error("a probe request carried a `{header}` header; the capture needs a clean profile")]
    Credential {
        /// The header.
        header: &'static str,
    },
    /// The measured file does not exist yet or has no TLS measurement to extend.
    #[error("{path} has no TLS measurement; run capture_tls and capture_hints first")]
    NoBase {
        /// The file.
        path: String,
    },
    /// The measured file was captured from another browser version.
    #[error("the measured file holds Chrome {file}, but this browser is Chrome {browser}; run capture_tls and capture_hints again first")]
    VersionMismatch {
        /// The version in the file.
        file: String,
        /// The version of the running browser.
        browser: String,
    },
    /// The measured file is unusable.
    #[error("{reason}")]
    Store {
        /// What is wrong.
        reason: &'static str,
    },
    /// The two passes ran against different browser versions.
    #[error("the HTTP/2 pass saw Chrome {first} and the HTTP/1.1 pass saw Chrome {second}")]
    DifferentBrowsers {
        /// The version of the first pass.
        first: String,
        /// The version of the second pass.
        second: String,
    },
}

/// Which passes a run makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Passes {
    /// The HTTP/2 pass, then the HTTP/1.1 pass.
    Both,
    /// Only the HTTP/2 pass.
    H2Only,
    /// Only the HTTP/1.1 pass.
    H1Only,
}

/// Reads the command line of the tool: the options the capture tools share, and `--h1` or `--h2`.
///
/// # Errors
///
/// Returns [`CaptureError::Usage`] (as a [`CaptureH2Error`]) for an unknown argument, a flag
/// without its value, a value out of range, or `--h1` together with `--h2`.
pub fn parse_args<I>(args: I, root: &Path) -> Result<(ToolArgs, Passes), CaptureH2Error>
where
    I: IntoIterator<Item = String>,
{
    let (mut h1, mut h2) = (false, false);
    let rest: Vec<String> = args
        .into_iter()
        .filter(|arg| match arg.as_str() {
            FLAG_H1 => {
                h1 = true;
                false
            }
            FLAG_H2 => {
                h2 = true;
                false
            }
            _ => true,
        })
        .collect();
    let passes = match (h1, h2) {
        (true, true) => {
            return Err(CaptureError::Usage(format!(
                "`{FLAG_H1}` and `{FLAG_H2}` exclude each other"
            ))
            .into())
        }
        (true, false) => Passes::H1Only,
        (false, true) => Passes::H2Only,
        (false, false) => Passes::Both,
    };
    Ok((ToolArgs::parse(rest, root)?, passes))
}

/// What a run measured and wrote.
#[derive(Debug)]
pub struct Report {
    /// The browser that was measured.
    pub version: BrowserVersion,
    /// The measured file's identity.
    pub id: EntryId,
    /// The HTTP/2 evidence now in the file, when there is any.
    pub h2: Option<Vec<H2Evidence>>,
    /// The HTTP/1 evidence now in the file, when there is any.
    pub h1: Option<Vec<H1Evidence>>,
    /// The specs derived from both, when both passes have evidence.
    pub derived: Option<Derived>,
    /// The measured file that was written.
    pub path: PathBuf,
    /// The raw HTTP/2 fixtures that were written.
    pub fixtures: Vec<PathBuf>,
}

/// Runs the requested passes against the local Chrome and writes the result into the measured file
/// of that browser.
///
/// A pass that did not run keeps the evidence an earlier run stored, so the specs are derived
/// again from whatever both passes have. The measured file must already hold a TLS measurement of
/// the same browser version (run `capture_tls` and `capture_hints` first).
///
/// # Errors
///
/// Returns a [`CaptureH2Error`] when the browser cannot be measured, a probe is missing, the
/// passes disagree, or the file cannot be extended.
pub async fn measure_and_write(
    args: &ToolArgs,
    passes: Passes,
    tool: &str,
    now: DateTime<Utc>,
) -> Result<Report, CaptureH2Error> {
    let mut version = None;
    let mut h2 = None;
    let mut h1 = None;
    if passes != Passes::H1Only {
        let (seen_version, observed) = browser::run_pass(&args.chrome, Protocol::H2).await?;
        h2 = Some(h2_evidence(&observed)?);
        version = Some(seen_version);
    }
    if passes != Passes::H2Only {
        let (seen_version, observed) = browser::run_pass(&args.chrome, Protocol::Http1).await?;
        if let Some(first) = &version {
            same_browser(first, &seen_version)?;
        }
        h1 = Some(h1_evidence(&observed)?);
        version = Some(seen_version);
    }
    let version = version.ok_or(CaptureH2Error::Store {
        reason: "no pass was requested",
    })?;
    write_result(args, version, h2, h1, tool, now)
}

/// Checks that two passes measured the same browser build.
fn same_browser(first: &BrowserVersion, second: &BrowserVersion) -> Result<(), CaptureH2Error> {
    if first == second {
        Ok(())
    } else {
        Err(CaptureH2Error::DifferentBrowsers {
            first: first.full_version.clone(),
            second: second.full_version.clone(),
        })
    }
}

/// Adds the evidence of the passes that ran to the file, derives the specs when both passes have
/// evidence, and saves.
fn write_result(
    args: &ToolArgs,
    version: BrowserVersion,
    new_h2: Option<Vec<H2Evidence>>,
    new_h1: Option<Vec<H1Evidence>>,
    tool: &str,
    now: DateTime<Utc>,
) -> Result<Report, CaptureH2Error> {
    let id = EntryId {
        kind: BrowserKind::Chrome,
        major: version.major,
        os: version.os()?,
    };
    let mut doc = MeasuredDoc::open(&args.out_dir, id, &version)?;
    let mut fixtures = Vec::new();
    if let Some(evidence) = &new_h2 {
        doc.set_h2_evidence(evidence);
        fixtures = write_h2_fixtures(&args.fixtures_dir, id, evidence)?;
    }
    if let Some(evidence) = &new_h1 {
        doc.set_h1_evidence(evidence);
    }
    let h2 = new_h2.or(doc.h2_evidence()?);
    let h1 = new_h1.or(doc.h1_evidence()?);
    let derived = match (&h2, &h1) {
        (Some(h2), Some(h1)) => Some(derive(h2, h1)?),
        _ => None,
    };
    if let Some(derived) = &derived {
        doc.set_derived(derived)?;
    }
    doc.stamp(tool, &version, now);
    doc.save()?;
    Ok(Report {
        version,
        id,
        h2,
        h1,
        derived,
        path: doc.path().to_path_buf(),
        fixtures,
    })
}

#[cfg(test)]
mod tests;
