//! From what the browser sent to the specs: the evidence the tool stores, and the pure functions
//! that turn it into an [`Http2Spec`] and a [`HeaderSpec`].
//!
//! Everything here works on bytes and header lists already in memory. The evidence is what the
//! measured JSON keeps under `capture.h2` and `capture.h1`, so the specs can be derived again from
//! the file alone: [`derive`] is the one function both the tool and the tests use.
//!
//! # Where each spec comes from
//!
//! - The HTTP/2 spec is the connection start of the navigation. The other probes' connections
//!   must agree with it on SETTINGS, window update, PRIORITY frames and pseudo-header order, or
//!   the measurement is refused. The priority fields of HEADERS belong to the navigation only,
//!   because the browser sets them per request type; the others are kept as evidence.
//! - The header lists come from the HTTP/1 pass, since HTTP/1 shows the capitalisation the
//!   browser writes and HTTP/2 lowercases it. The HTTP/2 pass supplies the `priority` header of
//!   each `Sec-Fetch-Dest` and is compared with HTTP/1: any difference in order or value is listed
//!   in [`Derived::differences`] instead of being hidden.
//! - `Origin` and `Referer` are left out of the lists: their values name the probe's own host,
//!   and a caller adds them for the site it is talking to.

use serde::{Deserialize, Serialize};

use super::probes::{Probe, DOCUMENT, FETCH, PROBES};
use super::site::{Observed, Seen};
use super::CaptureH2Error;
use crate::scrape::runtime::capture::record::{from_hex, to_hex};
use crate::wire::h2_frames::{read_preface, H2Preface, HeadersPriority};
use crate::wire::h2_spec::Http2Spec;
use crate::wire::header_spec::{HeaderSpec, CREDENTIAL_HEADERS, TRANSPORT_HEADERS};

/// Headers whose values name the probe's own host, so they are not part of a profile.
const REQUEST_SPECIFIC: [&str; 2] = ["origin", "referer"];
/// Headers that carry a credential; a probe request that has one is refused.
const CREDENTIALS: [&str; 3] = ["cookie", "authorization", "proxy-authorization"];
/// The header that carries the request's destination.
const FETCH_DEST: &str = "sec-fetch-dest";
/// The HTTP/2 header that carries the request priority.
const PRIORITY: &str = "priority";

/// One HTTP/2 connection start, as stored under `capture.h2.probes`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct H2Evidence {
    /// The probe's name.
    pub probe: String,
    /// The server name the browser asked for.
    pub sni: String,
    /// The bytes from the connection preface to the end of the first request, in hexadecimal.
    pub raw_hex: String,
    /// The Akamai string of the connection start.
    pub akamai: String,
    /// The stream id of the request.
    pub stream_id: u32,
    /// The priority fields of the request's HEADERS frame, when it had them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers_priority: Option<HeadersPriority>,
    /// The request's headers, pseudo-headers first, in wire order.
    pub headers: Vec<(String, String)>,
}

/// One HTTP/1 request, as stored under `capture.h1.probes`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct H1Evidence {
    /// The probe's name.
    pub probe: String,
    /// The server name the browser asked for.
    pub sni: String,
    /// The request method.
    pub method: String,
    /// The protocol version on the request line.
    pub version: String,
    /// The headers in wire order, names as the browser wrote them.
    pub headers: Vec<(String, String)>,
}

/// What the two passes measured, derived from the evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Derived {
    /// The start of the browser's HTTP/2 connections.
    pub http2: Http2Spec,
    /// The header lists and the `priority` header per destination.
    pub headers: HeaderSpec,
    /// Where HTTP/1 and HTTP/2 disagree on header order or values; empty when they agree.
    pub differences: Vec<String>,
}

impl H2Evidence {
    /// Records the first request of a connection to `probe`.
    ///
    /// # Errors
    ///
    /// Returns a [`CaptureH2Error`] when the pseudo-headers are not a permutation of the four
    /// request pseudo-headers.
    pub fn of(probe: Probe, raw: &[u8], preface: &H2Preface) -> Result<Self, CaptureH2Error> {
        Ok(Self {
            probe: probe.name.to_owned(),
            sni: probe.host(),
            raw_hex: to_hex(raw),
            akamai: preface.akamai()?.to_string(),
            stream_id: preface.request_stream_id,
            headers_priority: preface.headers_priority,
            headers: preface
                .headers
                .iter()
                .map(|field| (field.name.clone(), field.value.clone()))
                .collect(),
        })
    }

    /// Decodes the stored bytes again and checks that they say what the record says.
    ///
    /// # Errors
    ///
    /// Returns a [`CaptureH2Error`] when the bytes are not an HTTP/2 connection start or the
    /// stored Akamai string, stream id or headers differ from what the bytes decode to.
    pub fn preface(&self) -> Result<H2Preface, CaptureH2Error> {
        let raw = from_hex(&self.raw_hex)?;
        let preface = read_preface(&raw)?.ok_or(CaptureH2Error::Evidence {
            probe: self.probe.clone(),
            reason: "the stored bytes end before the first request is complete",
        })?;
        let decoded: Vec<(String, String)> = preface
            .headers
            .iter()
            .map(|field| (field.name.clone(), field.value.clone()))
            .collect();
        let agrees = decoded == self.headers
            && preface.request_stream_id == self.stream_id
            && preface.akamai()?.to_string() == self.akamai;
        if agrees {
            Ok(preface)
        } else {
            Err(CaptureH2Error::Evidence {
                probe: self.probe.clone(),
                reason: "the stored fields differ from what the stored bytes decode to",
            })
        }
    }

    /// The value of the first header named `name`, compared without regard to ASCII case.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(known, _)| known.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

impl H1Evidence {
    /// The value of the first header named `name`, compared without regard to ASCII case.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(known, _)| known.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// The evidence of the first HTTP/2 connection to each probe, in probe order.
///
/// # Errors
///
/// Returns [`CaptureH2Error::MissingProbe`] when a probe has no HTTP/2 connection,
/// [`CaptureH2Error::WrongDestination`] when a request's `Sec-Fetch-Dest` is not the probe's, and
/// the errors of [`H2Evidence::of`].
pub fn h2_evidence(observed: &[Observed]) -> Result<Vec<H2Evidence>, CaptureH2Error> {
    PROBES
        .into_iter()
        .map(|probe| {
            let found = observed.iter().find_map(|one| match &one.seen {
                Seen::H2 { raw, preface } if one.probe == probe => Some((raw, preface)),
                _ => None,
            });
            let (raw, preface) = found.ok_or(CaptureH2Error::MissingProbe { probe: probe.name })?;
            check_destination(probe, preface.header(FETCH_DEST))?;
            reject_credentials(preface.headers.iter().map(|field| field.name.as_str()))?;
            H2Evidence::of(probe, raw, preface)
        })
        .collect()
}

/// The evidence of the first HTTP/1 request to each probe, in probe order.
///
/// # Errors
///
/// Returns [`CaptureH2Error::MissingProbe`] when a probe has no HTTP/1 request and
/// [`CaptureH2Error::WrongDestination`] when a request's `Sec-Fetch-Dest` is not the probe's.
pub fn h1_evidence(observed: &[Observed]) -> Result<Vec<H1Evidence>, CaptureH2Error> {
    PROBES
        .into_iter()
        .map(|probe| {
            let request = observed
                .iter()
                .find_map(|one| match &one.seen {
                    Seen::Http1 { request } if one.probe == probe => Some(request),
                    _ => None,
                })
                .ok_or(CaptureH2Error::MissingProbe { probe: probe.name })?;
            check_destination(probe, request.header(FETCH_DEST))?;
            reject_credentials(request.headers.iter().map(|(name, _)| name.as_str()))?;
            Ok(H1Evidence {
                probe: probe.name.to_owned(),
                sni: probe.host(),
                method: request.method.clone(),
                version: request.version.clone(),
                headers: request.headers.clone(),
            })
        })
        .collect()
}

/// Checks that a request carries the `Sec-Fetch-Dest` its probe was meant to produce.
fn check_destination(probe: Probe, found: Option<&str>) -> Result<(), CaptureH2Error> {
    match found {
        Some(dest) if dest == probe.dest => Ok(()),
        other => Err(CaptureH2Error::WrongDestination {
            probe: probe.name,
            found: other.unwrap_or("none").to_owned(),
        }),
    }
}

/// Refuses a request that carries a credential, so none can reach the stored evidence.
fn reject_credentials<'a>(names: impl Iterator<Item = &'a str>) -> Result<(), CaptureH2Error> {
    for name in names {
        if let Some(header) = CREDENTIALS
            .into_iter()
            .find(|credential| credential.eq_ignore_ascii_case(name))
        {
            return Err(CaptureH2Error::Credential { header });
        }
    }
    Ok(())
}

/// The evidence of the probe called `name`.
fn find<'a, T>(items: &'a [T], name: &str, key: fn(&T) -> &str) -> Result<&'a T, CaptureH2Error> {
    items
        .iter()
        .find(|item| key(item) == name)
        .ok_or(CaptureH2Error::Evidence {
            probe: name.to_owned(),
            reason: "the probe has no evidence",
        })
}

/// Derives the HTTP/2 spec and the header spec from the evidence of both passes.
///
/// # Errors
///
/// Returns a [`CaptureH2Error`] when a probe is missing, the stored bytes do not decode to what
/// the record says, the probes' connections disagree, or a header list is refused by
/// [`HeaderSpec::from_captured`].
pub fn derive(h2: &[H2Evidence], h1: &[H1Evidence]) -> Result<Derived, CaptureH2Error> {
    let prefaces: Vec<(&H2Evidence, H2Preface)> = h2
        .iter()
        .map(|evidence| Ok((evidence, evidence.preface()?)))
        .collect::<Result<_, CaptureH2Error>>()?;
    let http2 = http2_spec(&prefaces)?;
    let document = find(h1, DOCUMENT.name, |item| item.probe.as_str())?;
    let fetch = find(h1, FETCH.name, |item| item.probe.as_str())?;
    let mut headers =
        HeaderSpec::from_captured(&profile_headers(document), &profile_headers(fetch))?;
    for (evidence, _) in &prefaces {
        if let (Some(dest), Some(priority)) =
            (evidence.header(FETCH_DEST), evidence.header(PRIORITY))
        {
            headers = headers.with_priority(dest, priority)?;
        }
    }
    let differences = header_differences(h1, h2);
    Ok(Derived {
        http2,
        headers,
        differences,
    })
}

/// The spec of the navigation's connection, once every other probe's connection agrees with it.
fn http2_spec(prefaces: &[(&H2Evidence, H2Preface)]) -> Result<Http2Spec, CaptureH2Error> {
    let (_, document) = prefaces
        .iter()
        .find(|(evidence, _)| evidence.probe == DOCUMENT.name)
        .ok_or(CaptureH2Error::MissingProbe {
            probe: DOCUMENT.name,
        })?;
    let spec = Http2Spec::from_preface(document)?;
    for (evidence, preface) in prefaces {
        if Http2Spec::from_preface(preface)?.akamai() != spec.akamai() {
            return Err(CaptureH2Error::ProbesDisagree {
                probe: evidence.probe.clone(),
            });
        }
    }
    Ok(spec)
}

/// The headers of a request that belong in a profile: no connection-level headers, no
/// credentials and nothing that names the probe's host.
fn profile_headers(request: &H1Evidence) -> Vec<(String, String)> {
    request
        .headers
        .iter()
        .filter(|(name, _)| !REQUEST_SPECIFIC.contains(&name.to_ascii_lowercase().as_str()))
        .cloned()
        .collect()
}

/// The comparable form of a header list: lowercase names, without what only one protocol has.
fn comparable(headers: &[(String, String)]) -> Vec<(String, String)> {
    headers
        .iter()
        .map(|(name, value)| (name.to_ascii_lowercase(), value.clone()))
        .filter(|(name, _)| {
            !name.starts_with(':')
                && name != PRIORITY
                && !REQUEST_SPECIFIC.contains(&name.as_str())
                && !TRANSPORT_HEADERS.contains(&name.as_str())
                && !CREDENTIAL_HEADERS.contains(&name.as_str())
        })
        .collect()
}

/// Lists where the HTTP/1 and HTTP/2 requests of the same probe differ.
fn header_differences(h1: &[H1Evidence], h2: &[H2Evidence]) -> Vec<String> {
    let mut found = Vec::new();
    for probe in PROBES {
        let one = h1.iter().find(|item| item.probe == probe.name);
        let two = h2.iter().find(|item| item.probe == probe.name);
        let (Some(one), Some(two)) = (one, two) else {
            continue;
        };
        let (left, right) = (comparable(&one.headers), comparable(&two.headers));
        let names = |list: &[(String, String)]| -> Vec<String> {
            list.iter().map(|(name, _)| name.clone()).collect()
        };
        if names(&left) != names(&right) {
            found.push(format!(
                "{}: HTTP/1.1 sends {:?} but HTTP/2 sends {:?}",
                probe.name,
                names(&left),
                names(&right)
            ));
        } else if let Some(((name, _), _)) = left.iter().zip(&right).find(|(a, b)| a.1 != b.1) {
            found.push(format!(
                "{}: the value of `{name}` differs between HTTP/1.1 and HTTP/2",
                probe.name
            ));
        }
    }
    found
}
