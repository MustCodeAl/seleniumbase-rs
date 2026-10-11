//! The JSON the measurement tools write: one file per browser, major version and operating system.
//!
//! A measured file holds the profile other code embeds (`tls`, `tls_websocket`, `client_hints`,
//! `user_agent`, `provenance`) and, under `capture`, the evidence it was derived from: the raw
//! bytes of each captured `ClientHello` and the fingerprints computed from them. The tools write
//! into the same file in turn, so [`MeasuredFile`] keeps every key it does not own.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use super::hints::HintsMeasurement;
use super::CaptureError;
use crate::scrape::runtime::json_store::atomic_write;
use crate::stealth::fingerprint::OsType;
use crate::wire::client_hello::{ext_id, ClientHello};
use crate::wire::ids::ML_DSA_SIGALGS;
use crate::wire::ja3::{hex_lower, Ja3, Peetprint};
use crate::wire::ja4::Ja4;
use crate::wire::tls_spec::{KnownGap, TlsSpec};
use crate::wire::BrowserKind;

/// The largest measured file that is read back.
const MAX_MEASURED_FILE_BYTES: u64 = 16 * 1024 * 1024;
/// Separates the tools named in the provenance of a file.
const TOOL_SEPARATOR: &str = "; ";
/// The product names a browser reports for Chrome and its headless mode.
const PRODUCTS: [&str; 2] = ["Chrome", "HeadlessChrome"];

/// The browser a capture ran against, as `Browser.getVersion` describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserVersion {
    /// The product string, such as `Chrome/155.0.8059.40`.
    pub product: String,
    /// The version after the slash, such as `155.0.8059.40`.
    pub full_version: String,
    /// The major version, such as `155`.
    pub major: u32,
    /// The user agent the browser reports.
    pub user_agent: String,
}

impl BrowserVersion {
    /// Reads the `product` and `userAgent` fields of a `Browser.getVersion` result.
    ///
    /// # Errors
    ///
    /// Returns [`CaptureError::Product`] when the product is not `Chrome/<version>` or
    /// `HeadlessChrome/<version>`.
    pub fn from_get_version(result: &Value) -> Result<Self, CaptureError> {
        let product = result["product"].as_str().unwrap_or_default();
        let unreadable = || CaptureError::Product(product.to_owned());
        let (name, full_version) = product.split_once('/').ok_or_else(unreadable)?;
        let major = full_version
            .split('.')
            .next()
            .and_then(|major| major.parse::<u32>().ok());
        let user_agent = result["userAgent"].as_str();
        match (PRODUCTS.contains(&name), major, user_agent) {
            (true, Some(major), Some(user_agent)) => Ok(Self {
                product: product.to_owned(),
                full_version: full_version.to_owned(),
                major,
                user_agent: user_agent.to_owned(),
            }),
            _ => Err(unreadable()),
        }
    }

    /// The operating system the user agent names.
    ///
    /// # Errors
    ///
    /// Returns [`CaptureError::UnknownOs`] when the user agent names none this tool knows.
    pub fn os(&self) -> Result<OsType, CaptureError> {
        // Order matters: an Android user agent says Linux, an iPhone's says Mac OS X.
        let agent = self.user_agent.as_str();
        let has = |token: &str| agent.contains(token);
        if has("Android") {
            Ok(OsType::Android)
        } else if has("iPhone") || has("iPad") {
            Ok(OsType::Ios)
        } else if has("Windows") {
            Ok(OsType::Windows)
        } else if has("Macintosh") || has("Mac OS X") {
            Ok(OsType::Macos)
        } else if has("Linux") || has("X11") {
            Ok(OsType::Linux)
        } else {
            Err(CaptureError::UnknownOs)
        }
    }

    /// Whether the user agent is a headless browser's.
    #[must_use]
    pub fn is_headless(&self) -> bool {
        self.user_agent.contains("Headless") || self.product.starts_with("Headless")
    }

    /// The browser as the provenance names it, such as `Chrome 155.0.8059.40`.
    #[must_use]
    pub fn label(&self) -> String {
        self.product.replace('/', " ")
    }
}

/// Which measured file a capture belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryId {
    /// The browser.
    pub kind: BrowserKind,
    /// The major version.
    pub major: u32,
    /// The operating system.
    pub os: OsType,
}

impl EntryId {
    /// The file name, such as `chrome-155-macos.json`.
    #[must_use]
    pub fn file_name(&self) -> String {
        format!("{}.json", self.stem())
    }

    /// The stem shared by the file and its raw fixtures, such as `chrome-155-macos`.
    #[must_use]
    pub fn stem(&self) -> String {
        let kind = match self.kind {
            BrowserKind::Chrome => "chrome",
        };
        let os = match self.os {
            OsType::Windows => "windows",
            OsType::Macos => "macos",
            OsType::Linux => "linux",
            OsType::Android => "android",
            OsType::Ios => "ios",
        };
        format!("{kind}-{}-{os}", self.major)
    }

    /// The name serde gives the browser in the file: `Chrome`.
    fn kind_name(self) -> &'static str {
        match self.kind {
            BrowserKind::Chrome => "Chrome",
        }
    }
}

/// One captured `ClientHello` and the fingerprints computed from its bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelloRecord {
    /// The server name the hello asked for.
    pub sni: Option<String>,
    /// The bytes of the TLS records that carry the hello, in lowercase hexadecimal.
    pub raw_hex: String,
    /// The JA3 string, GREASE removed, in the order the extensions were sent.
    pub ja3: String,
    /// The MD5 of [`HelloRecord::ja3`].
    pub ja3_hash: String,
    /// The JA4 fingerprint (sorted ciphers and extensions).
    pub ja4: String,
    /// The JA4 with the sorted ids written out (`JA4_r`).
    pub ja4_r: String,
    /// The JA4 with the ids written out in the order they were sent (`JA4_ro`).
    pub ja4_ro: String,
    /// The peetprint string.
    pub peetprint: String,
    /// The MD5 of [`HelloRecord::peetprint`].
    pub peetprint_hash: String,
}

impl HelloRecord {
    /// Parses `raw` as a hello and computes its fingerprints.
    ///
    /// # Errors
    ///
    /// Returns [`CaptureError::Hello`] when `raw` is not a `ClientHello`.
    pub fn of(raw: &[u8]) -> Result<Self, CaptureError> {
        Ok(Self::of_hello(raw, &ClientHello::parse(raw)?))
    }

    /// The record of a hello already parsed from `raw`.
    fn of_hello(raw: &[u8], hello: &ClientHello) -> Self {
        let ja3 = Ja3::of(hello);
        let peetprint = Peetprint::of(hello);
        Self {
            sni: hello.sni.clone(),
            raw_hex: to_hex(raw),
            ja3: ja3.to_string(),
            ja3_hash: ja3.hash(),
            ja4: Ja4::of(hello).to_string(),
            ja4_r: Ja4::raw(hello),
            ja4_ro: Ja4::raw_original_order(hello),
            peetprint: peetprint.to_string(),
            peetprint_hash: peetprint.hash(),
        }
    }

    /// The raw bytes.
    ///
    /// # Errors
    ///
    /// Returns [`CaptureError::Hex`] when the stored hexadecimal is malformed.
    pub fn raw(&self) -> Result<Vec<u8>, CaptureError> {
        from_hex(&self.raw_hex)
    }

    /// The parsed hello.
    ///
    /// # Errors
    ///
    /// Returns [`CaptureError::Hex`] or [`CaptureError::Hello`] when the stored bytes are not a hello.
    pub fn hello(&self) -> Result<ClientHello, CaptureError> {
        Ok(ClientHello::parse(&self.raw()?)?)
    }
}

/// What the TLS capture learned about a browser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsMeasurement {
    /// The profile of the browser's ordinary HTTPS connections.
    pub spec: TlsSpec,
    /// The profile of its WebSocket connections, when one was captured.
    pub websocket_spec: Option<TlsSpec>,
    /// The hellos the profile was derived from.
    pub samples: Vec<HelloRecord>,
    /// The WebSocket hello, when one was captured.
    pub websocket_sample: Option<HelloRecord>,
    /// What a `BoringSSL` client cannot reproduce of this browser's hello.
    pub known_gaps: Vec<KnownGap>,
}

impl TlsMeasurement {
    /// Builds a measurement from the raw bytes of several hellos of one browser and, optionally, a
    /// WebSocket hello.
    ///
    /// The profile comes from the first hello; the others show whether the browser shuffles its
    /// extensions (`permute_extensions`) and must have the same JA4.
    ///
    /// # Errors
    ///
    /// Returns [`CaptureError::NoHello`] when `page` is empty, [`CaptureError::Hello`] when a hello
    /// does not parse, [`CaptureError::Spec`] when two hellos differ in more than the order of
    /// their extensions and [`CaptureError::Unstable`] when their JA4 values differ.
    pub fn from_hellos(page: &[Vec<u8>], websocket: Option<&[u8]>) -> Result<Self, CaptureError> {
        let first_raw = page.first().ok_or(CaptureError::NoHello {
            what: "the ordinary connections",
        })?;
        let first = ClientHello::parse(first_raw)?;
        let mut spec = TlsSpec::from_client_hello(&first);
        let mut samples = vec![HelloRecord::of_hello(first_raw, &first)];
        for raw in page.iter().skip(1) {
            let other = ClientHello::parse(raw)?;
            spec = spec.with_observed_order(&other)?;
            let record = HelloRecord::of_hello(raw, &other);
            if let Some(reference) = samples.first().filter(|first| first.ja4 != record.ja4) {
                return Err(CaptureError::Unstable {
                    first: reference.ja4.clone(),
                    other: record.ja4,
                });
            }
            samples.push(record);
        }
        // One WebSocket hello cannot show a shuffle, so it takes the browser-wide setting.
        let websocket_hello = websocket.map(ClientHello::parse).transpose()?;
        let websocket_spec = websocket_hello.as_ref().map(|hello| TlsSpec {
            permute_extensions: spec.permute_extensions,
            ..TlsSpec::from_client_hello(hello)
        });
        let websocket_sample = match (websocket, &websocket_hello) {
            (Some(raw), Some(hello)) => Some(HelloRecord::of_hello(raw, hello)),
            _ => None,
        };
        let known_gaps = known_gaps(&spec, websocket_spec.as_ref());
        Ok(Self {
            spec,
            websocket_spec,
            samples,
            websocket_sample,
            known_gaps,
        })
    }
}

/// What the specs carry that a `BoringSSL` client cannot send.
fn known_gaps(spec: &TlsSpec, websocket: Option<&TlsSpec>) -> Vec<KnownGap> {
    let specs: Vec<&TlsSpec> = std::iter::once(spec).chain(websocket).collect();
    let mut gaps = Vec::new();
    if specs
        .iter()
        .any(|spec| spec.has_extension(ext_id::TRUST_ANCHORS))
    {
        gaps.push(KnownGap::TrustAnchorsExtension);
    }
    if specs.iter().any(|spec| {
        spec.signature_algorithms
            .iter()
            .any(|id| ML_DSA_SIGALGS.contains(id))
    }) {
        gaps.push(KnownGap::MlDsaSignatureAlgorithms);
    }
    gaps
}

/// A measured JSON file being filled in by the tools.
#[derive(Debug)]
pub struct MeasuredFile {
    path: PathBuf,
    doc: Map<String, Value>,
}

impl MeasuredFile {
    /// Opens `<dir>/<id file name>`, or starts an empty document when it does not exist.
    ///
    /// # Errors
    ///
    /// Returns [`CaptureError::Io`] when the file cannot be read, [`CaptureError::Json`] when it is
    /// not JSON and [`CaptureError::NotAnObject`] when it is JSON but not an object.
    pub fn open(dir: &Path, id: EntryId) -> Result<Self, CaptureError> {
        let path = dir.join(id.file_name());
        let doc = match fs::metadata(&path) {
            Ok(meta) if meta.len() > MAX_MEASURED_FILE_BYTES => {
                return Err(CaptureError::TooLarge {
                    limit: usize::try_from(MAX_MEASURED_FILE_BYTES).unwrap_or(usize::MAX),
                });
            }
            Ok(_) => match serde_json::from_str::<Value>(&fs::read_to_string(&path)?)? {
                Value::Object(map) => map,
                _ => {
                    return Err(CaptureError::NotAnObject {
                        path: path.display().to_string(),
                    })
                }
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => Map::new(),
            Err(error) => return Err(error.into()),
        };
        Ok(Self { path, doc })
    }

    /// The file this document is saved to.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The value stored under a top-level key.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.doc.get(key)
    }

    /// Sets `kind`, `major`, `full_version` and `os`.
    pub fn set_identity(&mut self, id: EntryId, version: &BrowserVersion) {
        self.doc.insert("kind".to_owned(), json!(id.kind_name()));
        self.doc.insert("major".to_owned(), json!(version.major));
        self.doc
            .insert("full_version".to_owned(), json!(version.full_version));
        self.doc.insert("os".to_owned(), json!(id.os));
    }

    /// Puts `value` at `section.key`, making `section` an object if it is not one.
    fn put(&mut self, section: &str, key: &str, value: Value) {
        let slot = self
            .doc
            .entry(section.to_owned())
            .or_insert_with(|| Value::Object(Map::new()));
        if let Value::Object(map) = slot {
            map.insert(key.to_owned(), value);
        } else {
            *slot = json!({ key: value });
        }
    }

    /// Sets `tls`, `tls_websocket` and the evidence under `capture.tls`, and records the known gaps.
    ///
    /// # Errors
    ///
    /// Returns [`CaptureError::Json`] when a spec cannot be serialized.
    pub fn set_tls(&mut self, tls: &TlsMeasurement) -> Result<(), CaptureError> {
        self.doc
            .insert("tls".to_owned(), serde_json::to_value(&tls.spec)?);
        let websocket = serde_json::to_value(&tls.websocket_spec)?;
        self.doc.insert("tls_websocket".to_owned(), websocket);
        let evidence = json!({
            "samples": tls.samples,
            "websocket_sample": tls.websocket_sample,
        });
        self.put("capture", "tls", evidence);
        self.put(
            "provenance",
            "known_gaps",
            serde_json::to_value(&tls.known_gaps)?,
        );
        Ok(())
    }

    /// Sets `user_agent`, `client_hints` and the evidence under `capture.hints`.
    ///
    /// # Errors
    ///
    /// Returns [`CaptureError::Json`] when the hints cannot be serialized.
    pub fn set_hints(&mut self, hints: &HintsMeasurement) -> Result<(), CaptureError> {
        self.doc
            .insert("user_agent".to_owned(), json!(hints.user_agent));
        self.doc.insert(
            "client_hints".to_owned(),
            serde_json::to_value(&hints.client_hints)?,
        );
        let evidence = json!({
            "accept_language": hints.accept_language,
            "languages": hints.languages,
            "first_visit_headers": hints.first_visit_headers,
            "request_headers": hints.request_headers,
        });
        self.put("capture", "hints", evidence);
        Ok(())
    }

    /// Updates `provenance`: the date, the browser and the tool, which is merged with the tools
    /// that already wrote the file.
    pub fn stamp(&mut self, tool: &str, version: &BrowserVersion, now: DateTime<Utc>) {
        let known_gaps = self
            .doc
            .get("provenance")
            .and_then(|provenance| provenance.get("known_gaps"))
            .cloned()
            .unwrap_or_else(|| json!([]));
        let tools = merge_tool(
            self.doc
                .get("provenance")
                .and_then(|provenance| provenance.get("tool"))
                .and_then(Value::as_str),
            tool,
        );
        self.doc.insert(
            "provenance".to_owned(),
            json!({
                "captured_on": now.format("%Y-%m-%d").to_string(),
                "tool": tools,
                "browser": version.label(),
                "known_gaps": known_gaps,
            }),
        );
    }

    /// The document as pretty-printed JSON with a final newline.
    ///
    /// # Errors
    ///
    /// Returns [`CaptureError::Json`] when serialization fails.
    pub fn to_pretty_json(&self) -> Result<String, CaptureError> {
        let mut text = serde_json::to_string_pretty(&self.doc)?;
        text.push('\n');
        Ok(text)
    }

    /// Writes the file, creating its directory if needed.
    ///
    /// # Errors
    ///
    /// Returns [`CaptureError::Io`] or [`CaptureError::Json`] when the file cannot be written.
    pub fn save(&self) -> Result<(), CaptureError> {
        let text = self.to_pretty_json()?;
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        atomic_write(&self.path, text.as_bytes())?;
        Ok(())
    }
}

/// Joins `tool` into the `; `-separated list in `existing`, replacing a tool of the same name.
#[must_use]
pub fn merge_tool(existing: Option<&str>, tool: &str) -> String {
    let name = tool.split(' ').next().unwrap_or(tool);
    let mut tools: Vec<&str> = existing
        .unwrap_or_default()
        .split(TOOL_SEPARATOR)
        .filter(|entry| !entry.is_empty() && entry.split(' ').next() != Some(name))
        .collect();
    tools.push(tool);
    tools.sort_unstable();
    tools.join(TOOL_SEPARATOR)
}

/// Writes the raw hello bytes as binary fixtures next to the measured file's tests.
///
/// Returns the paths written: `<stem>.client_hello.<n>.bin` per sample (from 1) and
/// `<stem>.websocket_hello.bin` for the WebSocket hello.
///
/// # Errors
///
/// Returns [`CaptureError::Io`] when a file cannot be written and [`CaptureError::Hex`] when a
/// record's bytes are malformed.
pub fn write_fixtures(
    dir: &Path,
    id: EntryId,
    tls: &TlsMeasurement,
) -> Result<Vec<PathBuf>, CaptureError> {
    fs::create_dir_all(dir)?;
    let stem = id.stem();
    let mut written = Vec::new();
    for (index, sample) in tls.samples.iter().enumerate() {
        let path = dir.join(format!("{stem}.client_hello.{}.bin", index + 1));
        fs::write(&path, sample.raw()?)?;
        written.push(path);
    }
    if let Some(sample) = &tls.websocket_sample {
        let path = dir.join(format!("{stem}.websocket_hello.bin"));
        fs::write(&path, sample.raw()?)?;
        written.push(path);
    }
    Ok(written)
}

/// Lowercase hexadecimal of `bytes`.
#[must_use]
pub fn to_hex(bytes: &[u8]) -> String {
    hex_lower(bytes)
}

/// Decodes hexadecimal (either case).
///
/// # Errors
///
/// Returns [`CaptureError::Hex`] for an odd length or a character that is not a hex digit.
pub fn from_hex(text: &str) -> Result<Vec<u8>, CaptureError> {
    let (pairs, rest) = text.as_bytes().as_chunks::<2>();
    if !text.is_ascii() || !rest.is_empty() {
        return Err(CaptureError::Hex);
    }
    pairs
        .iter()
        .map(|[high, low]| Ok((nibble(*high)? << 4) | nibble(*low)?))
        .collect()
}

/// The value of one hexadecimal digit.
fn nibble(digit: u8) -> Result<u8, CaptureError> {
    char::from(digit)
        .to_digit(16)
        .and_then(|value| u8::try_from(value).ok())
        .ok_or(CaptureError::Hex)
}

#[cfg(test)]
mod tests;
