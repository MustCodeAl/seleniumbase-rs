//! Writing the HTTP/2 and header measurement into the measured JSON, and the raw frame fixtures.
//!
//! The file belongs to three tools in turn: `capture_tls` writes the hello, `capture_hints` writes
//! the identity and `capture_h2` adds `http2` and `headers`. [`MeasuredDoc`] reads the file as
//! plain JSON, changes only the keys this tool owns (`http2`, `headers`, `capture.h2`,
//! `capture.h1`, `capture.differences` and the provenance stamp) and keeps every other key as it
//! found it.
//!
//! A file is only extended when it already holds a TLS measurement of the same browser version.
//! Joining an HTTP/2 measurement of one Chrome to the hello of another would describe a browser
//! that never existed.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde_json::{json, Map, Value};

use super::assemble::{Derived, H1Evidence, H2Evidence};
use super::CaptureH2Error;
use crate::scrape::runtime::capture::record::{from_hex, merge_tool, BrowserVersion, EntryId};
use crate::scrape::runtime::capture::CaptureError;
use crate::scrape::runtime::json_store::atomic_write;

/// The largest measured file that is read back, in bytes.
const MAX_MEASURED_FILE_BYTES: u64 = 16 * 1024 * 1024;

/// A measured JSON file being extended by `capture_h2`.
#[derive(Debug)]
pub struct MeasuredDoc {
    path: PathBuf,
    doc: Map<String, Value>,
}

impl MeasuredDoc {
    /// Opens `<dir>/<id file name>`, which must already hold the TLS measurement of `version`.
    ///
    /// # Errors
    ///
    /// Returns [`CaptureH2Error::NoBase`] when the file does not exist or has no `tls`,
    /// [`CaptureH2Error::VersionMismatch`] when it was measured from another browser version,
    /// and [`CaptureH2Error::Store`] when it cannot be read or is not a JSON object.
    pub fn open(dir: &Path, id: EntryId, version: &BrowserVersion) -> Result<Self, CaptureH2Error> {
        let path = dir.join(id.file_name());
        let too_large = fs::metadata(&path).is_ok_and(|meta| meta.len() > MAX_MEASURED_FILE_BYTES);
        if too_large {
            return Err(CaptureH2Error::Store {
                reason: "the measured file is larger than 16 MiB",
            });
        }
        let text = fs::read_to_string(&path).map_err(|_missing| CaptureH2Error::NoBase {
            path: path.display().to_string(),
        })?;
        let Value::Object(doc) = serde_json::from_str(&text)? else {
            return Err(CaptureH2Error::Store {
                reason: "the measured file is not a JSON object",
            });
        };
        if !doc.contains_key("tls") {
            return Err(CaptureH2Error::NoBase {
                path: path.display().to_string(),
            });
        }
        let measured = doc.get("full_version").and_then(Value::as_str);
        if measured != Some(version.full_version.as_str()) {
            return Err(CaptureH2Error::VersionMismatch {
                file: measured.unwrap_or("unknown").to_owned(),
                browser: version.full_version.clone(),
            });
        }
        Ok(Self { path, doc })
    }

    /// The file this document is saved to.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The HTTP/2 evidence a previous run stored, when there is any.
    ///
    /// # Errors
    ///
    /// Returns [`CaptureH2Error::Json`] when the stored evidence has the wrong shape.
    pub fn h2_evidence(&self) -> Result<Option<Vec<H2Evidence>>, CaptureH2Error> {
        self.probes("h2")
    }

    /// The HTTP/1 evidence a previous run stored, when there is any.
    ///
    /// # Errors
    ///
    /// Returns [`CaptureH2Error::Json`] when the stored evidence has the wrong shape.
    pub fn h1_evidence(&self) -> Result<Option<Vec<H1Evidence>>, CaptureH2Error> {
        self.probes("h1")
    }

    /// The `capture.<pass>.probes` list, when present.
    fn probes<T: serde::de::DeserializeOwned>(
        &self,
        pass: &str,
    ) -> Result<Option<Vec<T>>, CaptureH2Error> {
        let probes = self
            .doc
            .get("capture")
            .and_then(|capture| capture.get(pass))
            .and_then(|pass| pass.get("probes"));
        probes
            .map(|probes| serde_json::from_value(probes.clone()))
            .transpose()
            .map_err(CaptureH2Error::from)
    }

    /// Puts `value` at `capture.<key>`.
    fn put_capture(&mut self, key: &str, value: Value) {
        let slot = self
            .doc
            .entry("capture".to_owned())
            .or_insert_with(|| Value::Object(Map::new()));
        if let Value::Object(map) = slot {
            map.insert(key.to_owned(), value);
        } else {
            *slot = json!({ key: value });
        }
    }

    /// Stores the evidence of the HTTP/2 pass.
    pub fn set_h2_evidence(&mut self, evidence: &[H2Evidence]) {
        self.put_capture("h2", json!({ "probes": evidence }));
    }

    /// Stores the evidence of the HTTP/1 pass.
    pub fn set_h1_evidence(&mut self, evidence: &[H1Evidence]) {
        self.put_capture("h1", json!({ "probes": evidence }));
    }

    /// Stores the specs derived from the evidence of both passes, and where the passes disagree.
    ///
    /// # Errors
    ///
    /// Returns [`CaptureH2Error::Json`] when a spec cannot be serialized.
    pub fn set_derived(&mut self, derived: &Derived) -> Result<(), CaptureH2Error> {
        self.doc
            .insert("http2".to_owned(), serde_json::to_value(&derived.http2)?);
        self.doc.insert(
            "headers".to_owned(),
            serde_json::to_value(&derived.headers)?,
        );
        self.put_capture("differences", json!(derived.differences));
        Ok(())
    }

    /// Updates the provenance: the date, the browser, and the tool merged with the tools that
    /// already wrote the file. The known gaps are kept.
    pub fn stamp(&mut self, tool: &str, version: &BrowserVersion, now: DateTime<Utc>) {
        let existing = self.doc.get("provenance");
        let known_gaps = existing
            .and_then(|provenance| provenance.get("known_gaps"))
            .cloned()
            .unwrap_or_else(|| json!([]));
        let tools = merge_tool(
            existing
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
    /// Returns [`CaptureH2Error::Json`] when serialization fails.
    pub fn to_pretty_json(&self) -> Result<String, CaptureH2Error> {
        let mut text = serde_json::to_string_pretty(&self.doc)?;
        text.push('\n');
        Ok(text)
    }

    /// Writes the file in place, through a temporary file and a rename.
    ///
    /// # Errors
    ///
    /// Returns [`CaptureH2Error::Json`] or [`CaptureH2Error::Capture`] when it cannot be written.
    pub fn save(&self) -> Result<(), CaptureH2Error> {
        let text = self.to_pretty_json()?;
        atomic_write(&self.path, text.as_bytes()).map_err(CaptureError::from)?;
        Ok(())
    }
}

/// Writes the raw bytes of each HTTP/2 connection start as `<stem>.h2.<probe>.bin`.
///
/// Returns the paths written.
///
/// # Errors
///
/// Returns a [`CaptureH2Error`] when a file cannot be written or the stored hexadecimal is bad.
pub fn write_h2_fixtures(
    dir: &Path,
    id: EntryId,
    evidence: &[H2Evidence],
) -> Result<Vec<PathBuf>, CaptureH2Error> {
    fs::create_dir_all(dir).map_err(CaptureError::from)?;
    let stem = id.stem();
    let mut written = Vec::with_capacity(evidence.len());
    for probe in evidence {
        let path = dir.join(format!("{stem}.h2.{}.bin", probe.probe));
        fs::write(&path, from_hex(&probe.raw_hex)?).map_err(CaptureError::from)?;
        written.push(path);
    }
    Ok(written)
}
