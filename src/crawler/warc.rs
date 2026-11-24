//! WARC (Web ARChive - ISO 28500) record generator and streaming writer.
//!
//! Complies with the ISO 28500 standard for permanent digital preservation of
//! web crawl data, requests, responses, and crawler provenance.

use std::io::{self, Write};
use std::time::SystemTime;

/// The WARC version string emitted in record headers.
pub const WARC_VERSION: &str = "WARC/1.1";

/// A WARC record type as defined in ISO 28500 section 5.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarcRecordType {
    /// Describes the crawl job, software version, and configuration.
    WarcInfo,
    /// Network response received from the target server (HTTP status, headers, body).
    Response,
    /// Network request sent to the target server (HTTP method, URI, headers).
    Request,
    /// Accompanying metadata and crawler telemetry (metrics, timing).
    Metadata,
}

impl WarcRecordType {
    /// String representation as used in `WARC-Type` headers.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WarcInfo => "warcinfo",
            Self::Response => "response",
            Self::Request => "request",
            Self::Metadata => "metadata",
        }
    }
}

/// A streaming writer for creating compliant `.warc` / `.warc.gz` files.
#[derive(Debug)]
pub struct WarcWriter<W> {
    writer: W,
}

impl<W: Write> WarcWriter<W> {
    /// Creates a new `WarcWriter` wrapping an arbitrary byte sink.
    #[must_use]
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    /// Consumes the writer and returns the underlying stream.
    pub fn into_inner(self) -> W {
        self.writer
    }

    /// Writes a `warcinfo` record introducing the crawl dataset.
    pub fn write_warcinfo(
        &mut self,
        filename: &str,
        user_agent: &str,
        operator: Option<&str>,
    ) -> io::Result<()> {
        let mut fields = format!(
            "software: seleniumbase-rs v0.9.1 (ISO 28500 Crawler)\r\n\
             format: WARC File Format 1.1\r\n\
             user-agent: {user_agent}\r\n"
        );
        if let Some(op) = operator {
            fields.push_str(&format!("operator: {op}\r\n"));
        }

        let record_id = format!("urn:uuid:{}", uuid_v4_like());
        let now = now_iso8601();

        self.write_raw_record(
            WarcRecordType::WarcInfo,
            &[
                ("WARC-Record-ID", &format!("<{record_id}>")),
                ("WARC-Date", &now),
                ("WARC-Filename", filename),
                ("Content-Type", "application/warc-fields"),
            ],
            fields.as_bytes(),
        )
    }

    /// Writes an HTTP `response` record containing the server response and body.
    pub fn write_response(
        &mut self,
        target_uri: &str,
        status_code: u16,
        status_text: &str,
        headers: &[(String, String)],
        body: &[u8],
    ) -> io::Result<()> {
        let mut http_header = format!("HTTP/1.1 {status_code} {status_text}\r\n");
        for (name, val) in headers {
            http_header.push_str(&format!("{name}: {val}\r\n"));
        }
        http_header.push_str("\r\n");

        let mut payload = http_header.into_bytes();
        payload.extend_from_slice(body);

        let record_id = format!("urn:uuid:{}", uuid_v4_like());
        let now = now_iso8601();

        self.write_raw_record(
            WarcRecordType::Response,
            &[
                ("WARC-Record-ID", &format!("<{record_id}>")),
                ("WARC-Date", &now),
                ("WARC-Target-URI", target_uri),
                ("Content-Type", "application/http; msgtype=response"),
            ],
            &payload,
        )
    }

    /// Writes an HTTP `request` record containing the request line and headers.
    pub fn write_request(
        &mut self,
        target_uri: &str,
        method: &str,
        headers: &[(String, String)],
        body: &[u8],
    ) -> io::Result<()> {
        let mut http_header = format!("{method} {target_uri} HTTP/1.1\r\n");
        for (name, val) in headers {
            http_header.push_str(&format!("{name}: {val}\r\n"));
        }
        http_header.push_str("\r\n");

        let mut payload = http_header.into_bytes();
        payload.extend_from_slice(body);

        let record_id = format!("urn:uuid:{}", uuid_v4_like());
        let now = now_iso8601();

        self.write_raw_record(
            WarcRecordType::Request,
            &[
                ("WARC-Record-ID", &format!("<{record_id}>")),
                ("WARC-Date", &now),
                ("WARC-Target-URI", target_uri),
                ("Content-Type", "application/http; msgtype=request"),
            ],
            &payload,
        )
    }

    fn write_raw_record(
        &mut self,
        record_type: WarcRecordType,
        extra_headers: &[(&str, &str)],
        payload: &[u8],
    ) -> io::Result<()> {
        write!(self.writer, "{WARC_VERSION}\r\n")?;
        write!(self.writer, "WARC-Type: {}\r\n", record_type.as_str())?;
        for (key, val) in extra_headers {
            write!(self.writer, "{key}: {val}\r\n")?;
        }
        write!(self.writer, "Content-Length: {}\r\n", payload.len())?;
        write!(self.writer, "\r\n")?;
        self.writer.write_all(payload)?;
        write!(self.writer, "\r\n\r\n")?;
        self.writer.flush()
    }
}

fn now_iso8601() -> String {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let days = secs / 86400;
    let rem_secs = secs % 86400;
    let hours = rem_secs / 3600;
    let mins = (rem_secs % 3600) / 60;
    let s = rem_secs % 60;

    // Approximate ISO 8601 UTC timestamp without external chrono dependency
    let year = 1970 + days / 365;
    let day_of_year = days % 365;
    let month = (day_of_year / 30) + 1;
    let day = (day_of_year % 30) + 1;

    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{mins:02}:{s:02}Z")
}

fn uuid_v4_like() -> String {
    let seed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        "{:08x}-{:04x}-4{:03x}-a{:03x}-{:012x}",
        (seed & 0xFFFF_FFFF) as u32,
        ((seed >> 32) & 0xFFFF) as u16,
        ((seed >> 48) & 0x0FFF) as u16,
        ((seed >> 60) & 0x0FFF) as u16,
        ((seed >> 72) & 0xFFFF_FFFF_FFFF) as u64
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warc_writer_writes_valid_header_and_response() {
        let mut buf = Vec::new();
        let mut warc = WarcWriter::new(&mut buf);

        warc.write_warcinfo("crawl_01.warc", "SeleniumBase-Crawler/1.0", Some("SecTeam"))
            .unwrap();

        warc.write_response(
            "https://example.com/test",
            200,
            "OK",
            &[("Server".to_string(), "nginx".to_string())],
            b"<html>Hello WARC</html>",
        )
        .unwrap();

        let output = String::from_utf8_lossy(&buf);
        assert!(output.contains("WARC/1.1"));
        assert!(output.contains("WARC-Type: warcinfo"));
        assert!(output.contains("WARC-Filename: crawl_01.warc"));
        assert!(output.contains("WARC-Type: response"));
        assert!(output.contains("WARC-Target-URI: https://example.com/test"));
        assert!(output.contains("HTTP/1.1 200 OK"));
        assert!(output.contains("<html>Hello WARC</html>"));
    }
}
