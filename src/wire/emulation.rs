//! The measured browser tables (TLS, HTTP/2, headers, client hints), embedded from JSON and looked up by browser and OS.
//!
//! Every wire value this crate imitates was measured from a real browser by a capture tool
//! (`capture_tls`, `capture_hints`, `capture_h2`) and stored as JSON under `src/wire/measured`.
//! Nothing here is typed by hand. The files are embedded with `include_str!`, parsed once, and
//! looked up by [`BrowserKind`] and [`OsType`].
//!
//! Only what was measured exists. [`get`] answers an unmeasured browser and system pair with
//! [`EmulationError::NotMeasured`]; there is no fallback to a different system's values, because
//! a made-up persona is worse than a refusal. Today that is Chrome on macOS only: Windows,
//! Linux, Android, iOS and Safari stay refused until someone runs the capture tools there.
//!
//! A measured file carries its raw evidence under `capture` (the bytes the tools read and the
//! fingerprints computed from them). Parsing ignores that key, so the tables stay small, and a
//! tool that rewrites a file keeps it.
//!
//! Custom tables are first-class: [`Emulation::from_json`] reads any file in the same format,
//! for a browser the crate does not ship.
//!
//! ```
//! use seleniumbase_rs::stealth::fingerprint::OsType;
//! use seleniumbase_rs::wire::emulation::{self, EmulationError};
//! use seleniumbase_rs::wire::BrowserKind;
//!
//! let chrome = emulation::get(BrowserKind::Chrome, OsType::Macos)?;
//! assert!(chrome.user_agent.contains(&format!("Chrome/{}", chrome.major)));
//!
//! let windows = emulation::get(BrowserKind::Chrome, OsType::Windows);
//! assert!(matches!(windows, Err(EmulationError::NotMeasured { .. })));
//! # Ok::<(), EmulationError>(())
//! ```

use std::collections::BTreeSet;
use std::sync::OnceLock;

use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::h2_spec::Http2Spec;
use super::header_spec::{HeaderSpec, HeaderSpecError};
pub use super::tls_spec::KnownGap;
use super::tls_spec::TlsSpec;
use super::BrowserKind;
use crate::stealth::fingerprint::{ClientHints, OsType};

/// The measured files this crate ships: the file name and its text.
///
/// Add a file to `src/wire/measured` and to this list together. The test
/// `tests/wire_emulation.rs` fails when a file there is missing from here.
const SOURCES: [(&str, &str); 1] = [(
    "chrome-155-macos.json",
    include_str!("measured/chrome-155-macos.json"),
)];

/// The largest measured file [`Emulation::from_json`] reads, in bytes.
pub const MAX_MEASURED_JSON_BYTES: usize = 16 * 1024 * 1024;

/// Why a measured table could not be read or looked up.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum EmulationError {
    /// No capture exists for this browser on this operating system.
    #[error(
        "{kind:?} on {os:?} has not been measured; run the capture tools on that system to add it"
    )]
    NotMeasured {
        /// The browser that was asked for.
        kind: BrowserKind,
        /// The operating system that was asked for.
        os: OsType,
    },
    /// The browser is measured on this system, but not in this major version.
    #[error("{kind:?} {major} on {os:?} has not been measured")]
    VersionNotMeasured {
        /// The browser that was asked for.
        kind: BrowserKind,
        /// The major version that was asked for.
        major: u32,
        /// The operating system that was asked for.
        os: OsType,
    },
    /// A measured file is not valid JSON of the expected shape.
    #[error("measured file `{name}` cannot be read: {reason}")]
    Parse {
        /// The file.
        name: String,
        /// What the JSON parser said.
        reason: String,
    },
    /// A measured file is larger than [`MAX_MEASURED_JSON_BYTES`].
    #[error("measured file `{name}` is larger than {MAX_MEASURED_JSON_BYTES} bytes")]
    TooLarge {
        /// The file.
        name: String,
    },
    /// A measured file contradicts itself.
    #[error("measured file `{name}` is inconsistent: {reason}")]
    Inconsistent {
        /// The file.
        name: String,
        /// What disagrees.
        reason: &'static str,
    },
    /// The header spec of a measured file is invalid.
    #[error("measured file `{name}` has an invalid header spec: {source}")]
    Headers {
        /// The file.
        name: String,
        /// Why the header spec was refused.
        #[source]
        source: HeaderSpecError,
    },
    /// Two measured files describe the same browser, major version and system.
    #[error(
        "measured files `{first}` and `{second}` describe the same browser, version and system"
    )]
    Duplicate {
        /// The earlier file.
        first: String,
        /// The later file.
        second: String,
    },
}

/// Where and how a measured entry was captured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    /// The date of the last capture, `YYYY-MM-DD`.
    pub captured_on: String,
    /// The tools that wrote the entry, such as `capture_h2 0.1.0; capture_tls 0.1.0`.
    pub tool: String,
    /// The browser that was measured, such as `Chrome 155.0.8059.40`.
    pub browser: String,
    /// What a `BoringSSL` client cannot reproduce of this browser's hello.
    #[serde(default)]
    pub known_gaps: Vec<KnownGap>,
}

/// One measured browser on one operating system: everything a client must send to look like it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Emulation {
    /// The browser.
    pub kind: BrowserKind,
    /// The major version, such as `155`.
    pub major: u32,
    /// The full version, such as `155.0.8059.40`.
    pub full_version: String,
    /// The operating system the browser ran on.
    pub os: OsType,
    /// The `User-Agent` header the browser sends.
    pub user_agent: String,
    /// The `ClientHello` of its ordinary HTTPS connections.
    pub tls: TlsSpec,
    /// The `ClientHello` of its WebSocket connections, which offer only HTTP/1.1.
    pub tls_websocket: Option<TlsSpec>,
    /// The start of its HTTP/2 connections, when the HTTP/2 capture has run.
    pub http2: Option<Http2Spec>,
    /// The header names, order and values of its requests; empty until the HTTP/2 and
    /// header capture has run.
    pub headers: HeaderSpec,
    /// The client hints it reports, when the hints capture has run.
    pub client_hints: Option<ClientHints>,
    /// Where the entry came from.
    pub provenance: Provenance,
}

/// The browser name as the JSON spells it (`BrowserKind` has no serde of its own).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
enum KindName {
    Chrome,
}

impl From<KindName> for BrowserKind {
    fn from(name: KindName) -> Self {
        match name {
            KindName::Chrome => BrowserKind::Chrome,
        }
    }
}

/// The name a browser has in the JSON, and the token its user agent carries.
fn kind_names(kind: BrowserKind) -> (KindName, &'static str) {
    match kind {
        BrowserKind::Chrome => (KindName::Chrome, "Chrome"),
    }
}

/// The JSON shape of a measured file, read strictly: an unknown key is an error, except the
/// raw evidence under `capture`, which is skipped.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileIn {
    kind: KindName,
    major: u32,
    full_version: String,
    os: OsType,
    user_agent: String,
    tls: TlsSpec,
    #[serde(default)]
    tls_websocket: Option<TlsSpec>,
    #[serde(default)]
    http2: Option<Http2Spec>,
    #[serde(default)]
    headers: HeaderSpec,
    #[serde(default)]
    client_hints: Option<ClientHints>,
    provenance: Provenance,
    #[serde(default)]
    #[allow(dead_code, reason = "the raw evidence is skipped on purpose")]
    capture: Option<IgnoredAny>,
}

/// The JSON shape [`Emulation::to_json`] writes.
#[derive(Serialize)]
struct FileOut<'a> {
    kind: KindName,
    major: u32,
    full_version: &'a str,
    os: OsType,
    user_agent: &'a str,
    tls: &'a TlsSpec,
    tls_websocket: Option<&'a TlsSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    http2: Option<&'a Http2Spec>,
    headers: &'a HeaderSpec,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_hints: Option<&'a ClientHints>,
    provenance: &'a Provenance,
}

impl Emulation {
    /// Reads one measured file. `name` only labels errors.
    ///
    /// The raw evidence under `capture` is skipped. The entry is checked to agree with itself:
    /// the full version starts with the major version, the user agent names the browser at that
    /// major version, and the header spec is valid.
    ///
    /// # Errors
    ///
    /// Returns [`EmulationError::Parse`] when the text is not a measured file,
    /// [`EmulationError::TooLarge`] above [`MAX_MEASURED_JSON_BYTES`],
    /// [`EmulationError::Inconsistent`] when the entry contradicts itself and
    /// [`EmulationError::Headers`] when the header spec is invalid.
    pub fn from_json(name: &str, json: &str) -> Result<Self, EmulationError> {
        if json.len() > MAX_MEASURED_JSON_BYTES {
            return Err(EmulationError::TooLarge {
                name: name.to_owned(),
            });
        }
        let file: FileIn = serde_json::from_str(json).map_err(|error| EmulationError::Parse {
            name: name.to_owned(),
            reason: error.to_string(),
        })?;
        let entry = Self {
            kind: file.kind.into(),
            major: file.major,
            full_version: file.full_version,
            os: file.os,
            user_agent: file.user_agent,
            tls: file.tls,
            tls_websocket: file.tls_websocket,
            http2: file.http2,
            headers: file.headers,
            client_hints: file.client_hints,
            provenance: file.provenance,
        };
        entry.validate(name)?;
        Ok(entry)
    }

    /// The entry as pretty-printed JSON, without the raw evidence.
    ///
    /// # Errors
    ///
    /// Returns [`EmulationError::Parse`] when serialization fails, which a valid entry never does.
    pub fn to_json(&self) -> Result<String, EmulationError> {
        let (name, _) = kind_names(self.kind);
        let out = FileOut {
            kind: name,
            major: self.major,
            full_version: &self.full_version,
            os: self.os,
            user_agent: &self.user_agent,
            tls: &self.tls,
            tls_websocket: self.tls_websocket.as_ref(),
            http2: self.http2.as_ref(),
            headers: &self.headers,
            client_hints: self.client_hints.as_ref(),
            provenance: &self.provenance,
        };
        serde_json::to_string_pretty(&out).map_err(|error| EmulationError::Parse {
            name: self.file_stem(),
            reason: error.to_string(),
        })
    }

    /// The stem the capture tools give this entry's files, such as `chrome-155-macos`.
    #[must_use]
    pub fn file_stem(&self) -> String {
        let browser = match self.kind {
            BrowserKind::Chrome => "chrome",
        };
        format!("{browser}-{}-{}", self.major, os_slug(self.os))
    }

    /// Checks that the entry agrees with itself.
    fn validate(&self, name: &str) -> Result<(), EmulationError> {
        let inconsistent = |reason| EmulationError::Inconsistent {
            name: name.to_owned(),
            reason,
        };
        let major_prefix = format!("{}.", self.major);
        if !self.full_version.starts_with(&major_prefix) {
            return Err(inconsistent("full_version does not start with major"));
        }
        let (_, token) = kind_names(self.kind);
        if !self.user_agent.contains(&format!("{token}/{}", self.major)) {
            return Err(inconsistent(
                "user_agent does not name the browser at major",
            ));
        }
        self.headers
            .validate()
            .map_err(|source| EmulationError::Headers {
                name: name.to_owned(),
                source,
            })
    }
}

/// The lowercase word the capture tools use for an operating system in file names.
fn os_slug(os: OsType) -> &'static str {
    match os {
        OsType::Windows => "windows",
        OsType::Macos => "macos",
        OsType::Linux => "linux",
        OsType::Android => "android",
        OsType::Ios => "ios",
    }
}

/// Reads every embedded file and checks that no two describe the same entry.
fn parse_all(sources: &[(&str, &str)]) -> Result<Vec<Emulation>, EmulationError> {
    let mut entries: Vec<(&str, Emulation)> = Vec::with_capacity(sources.len());
    for (name, text) in sources {
        let entry = Emulation::from_json(name, text)?;
        if let Some((first, _)) = entries.iter().find(|(_, known)| {
            known.kind == entry.kind && known.major == entry.major && known.os == entry.os
        }) {
            return Err(EmulationError::Duplicate {
                first: (*first).to_owned(),
                second: (*name).to_owned(),
            });
        }
        entries.push((name, entry));
    }
    Ok(entries.into_iter().map(|(_, entry)| entry).collect())
}

/// The parsed tables, or why the embedded files are broken.
fn table() -> &'static Result<Vec<Emulation>, EmulationError> {
    static TABLE: OnceLock<Result<Vec<Emulation>, EmulationError>> = OnceLock::new();
    TABLE.get_or_init(|| parse_all(&SOURCES))
}

/// The names of the embedded measured files.
#[must_use]
pub fn source_names() -> Vec<&'static str> {
    SOURCES.iter().map(|(name, _)| *name).collect()
}

/// Every measured entry, parsed once from the embedded JSON.
///
/// # Panics
///
/// Panics if an embedded file is broken. The unit tests parse every file, so a broken file
/// cannot reach a release; use [`try_measured`] to see the error instead.
#[must_use]
pub fn measured() -> &'static [Emulation] {
    try_measured().expect("the embedded measured files parse; the emulation unit tests prove it")
}

/// Every measured entry, or the reason an embedded file could not be read.
///
/// # Errors
///
/// Returns the first [`EmulationError`] met while parsing the embedded files.
pub fn try_measured() -> Result<&'static [Emulation], &'static EmulationError> {
    table().as_ref().map(Vec::as_slice)
}

/// The entry with the highest major version among `candidates`; the first of equals wins.
fn highest_major<'a>(candidates: impl Iterator<Item = &'a Emulation>) -> Option<&'a Emulation> {
    candidates.fold(None, |best: Option<&Emulation>, entry| match best {
        Some(known) if known.major >= entry.major => Some(known),
        _ => Some(entry),
    })
}

/// The newest entry (highest major version) of `kind` in `entries`; the first of equals wins.
#[must_use]
pub fn newest(entries: &[Emulation], kind: BrowserKind) -> Option<&Emulation> {
    highest_major(entries.iter().filter(|entry| entry.kind == kind))
}

/// The newest entry of `kind` on `os` in `entries`.
#[must_use]
pub fn newest_on(entries: &[Emulation], kind: BrowserKind, os: OsType) -> Option<&Emulation> {
    highest_major(
        entries
            .iter()
            .filter(|entry| entry.kind == kind && entry.os == os),
    )
}

/// The newest measured entry of a browser, whatever the system.
///
/// Every [`BrowserKind`] has an entry; the unit test `every_browser_kind_has_an_entry` fails
/// when a variant is added without a measurement.
///
/// # Panics
///
/// Panics if `kind` has no entry, which that test rules out.
#[must_use]
pub fn for_kind(kind: BrowserKind) -> &'static Emulation {
    newest(measured(), kind)
        .expect("every BrowserKind has a measured entry; a unit test enforces it")
}

/// The newest measured entry of a browser on an operating system.
///
/// # Errors
///
/// Returns [`EmulationError::NotMeasured`] when the pair has no capture.
pub fn get(kind: BrowserKind, os: OsType) -> Result<&'static Emulation, EmulationError> {
    newest_on(measured(), kind, os).ok_or(EmulationError::NotMeasured { kind, os })
}

/// The measured entry of a browser at one major version on an operating system.
///
/// # Errors
///
/// Returns [`EmulationError::NotMeasured`] when the pair has no capture at all and
/// [`EmulationError::VersionNotMeasured`] when it has captures, but not at `major`.
pub fn get_version(
    kind: BrowserKind,
    os: OsType,
    major: u32,
) -> Result<&'static Emulation, EmulationError> {
    get(kind, os)?;
    measured()
        .iter()
        .find(|entry| entry.kind == kind && entry.os == os && entry.major == major)
        .ok_or(EmulationError::VersionNotMeasured { kind, major, os })
}

/// The operating systems with a measured entry for `kind`, without repeats.
#[must_use]
pub fn measured_systems(kind: BrowserKind) -> Vec<OsType> {
    let mut seen = BTreeSet::new();
    measured()
        .iter()
        .filter(|entry| entry.kind == kind)
        .map(|entry| entry.os)
        .filter(|os| seen.insert(os_slug(*os)))
        .collect()
}

#[cfg(test)]
mod tests;
