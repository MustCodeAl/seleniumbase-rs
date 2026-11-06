---
title: "codec::measurement"
sidebarTitle: "WireMeasurement"
description: "The measured browser tables of seleniumbase-rs-codec: WireMeasurement, Provenance, WireMeasurementError and the lookups get, get_version, for_kind, measured, try_measured, newest and measured_systems."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: codec</Badge>

`seleniumbase_rs_stealth::codec::measurement` holds every wire value the stealth layer
imitates. Each one was measured from a real browser by a capture tool
(`capture_tls`, `capture_hints`, `capture_h2`) and stored as JSON under
`crates/codec/src/measured`. The files are embedded with `include_str!`, parsed
once, and looked up by `BrowserKind` and `OsType`. Only what was measured
exists: an unmeasured pair is an error, never a nearby guess. Today the table
holds `chrome-155-macos.json`. How the files are made and refreshed is in
[Wire formats](/stealth/advanced/wire-formats#re-measuring-when-chrome-updates).

<RequestExample>

```rust Look up a measured browser
use seleniumbase_rs_stealth::evasion::fingerprint::OsType;
use seleniumbase_rs_stealth::codec::measurement::{self, WireMeasurementError};
use seleniumbase_rs_stealth::codec::BrowserKind;

fn main() -> Result<(), WireMeasurementError> {
    let chrome = measurement::get(BrowserKind::Chrome, OsType::Macos)?;
    println!("{} {}", chrome.major, chrome.full_version);
    println!("{}", chrome.provenance.captured_on);
    println!("{:?}", chrome.provenance.known_gaps);
    println!("{}", chrome.file_stem());
    Ok(())
}
```

```rust An unmeasured system is refused
use seleniumbase_rs_stealth::evasion::fingerprint::OsType;
use seleniumbase_rs_stealth::codec::measurement::{self, WireMeasurementError};
use seleniumbase_rs_stealth::codec::BrowserKind;

let windows = measurement::get(BrowserKind::Chrome, OsType::Windows);
assert!(matches!(windows, Err(WireMeasurementError::NotMeasured { .. })));
```

```rust A table of your own
use seleniumbase_rs_stealth::codec::measurement::WireMeasurement;

fn load(text: &str) -> Result<WireMeasurement, Box<dyn std::error::Error>> {
    Ok(WireMeasurement::from_json("my-browser.json", text)?)
}
```

</RequestExample>

<ResponseExample>

```text Look up a measured browser
155 155.0.8059.40
2026-10-11
[TrustAnchorsExtension, MlDsaSignatureAlgorithms]
chrome-155-macos
```

</ResponseExample>

## `WireMeasurement`

<Badge color="blue" shape="pill">struct</Badge>

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireMeasurement {
    pub kind: BrowserKind,
    pub major: u32,
    pub full_version: String,
    pub os: OsType,
    pub user_agent: String,
    pub tls: TlsSpec,
    pub tls_websocket: Option<TlsSpec>,
    pub http2: Option<H2Spec>,
    pub headers: HeaderSpec,
    pub client_hints: Option<ClientHints>,
    pub provenance: Provenance,
}
```

One measured browser on one operating system: everything a client must send to
look like it. `OsType` and `ClientHints` are the core's
(`seleniumbase_rs_stealth::evasion::fingerprint`).

<ResponseField name="kind" type="BrowserKind" required>

The browser.

</ResponseField>

<ResponseField name="major" type="u32" required>

The major version, such as `155`.

</ResponseField>

<ResponseField name="full_version" type="String" required>

The full version, such as `155.0.8059.40`.

</ResponseField>

<ResponseField name="os" type="OsType" required>

The operating system the browser ran on.

</ResponseField>

<ResponseField name="user_agent" type="String" required>

The `User-Agent` header the browser sends.

</ResponseField>

<ResponseField name="tls" type="TlsSpec" required>

The `ClientHello` of its ordinary HTTPS connections. See [TlsSpec](/stealth/api/codec-tls-spec).

</ResponseField>

<ResponseField name="tls_websocket" type="Option<TlsSpec>">

The `ClientHello` of its WebSocket connections, which offer only HTTP/1.1.

</ResponseField>

<ResponseField name="http2" type="Option<H2Spec>">

The start of its HTTP/2 connections, when the HTTP/2 capture has run. See
[H2Spec](/stealth/api/codec-h2-spec).

</ResponseField>

<ResponseField name="headers" type="HeaderSpec" required>

The header names, order and values of its requests; empty until the HTTP/2 and
header capture has run. See [HeaderSpec](/stealth/api/codec-header-spec).

</ResponseField>

<ResponseField name="client_hints" type="Option<ClientHints>">

The client hints it reports, when the hints capture has run.

</ResponseField>

<ResponseField name="provenance" type="Provenance" required>

Where the entry came from.

</ResponseField>

### `WireMeasurement::from_json`

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn from_json(name: &str, json: &str) -> Result<Self, WireMeasurementError>
```

Reads one measured file, for a browser the crate does not ship as well as for the
embedded ones. `name` only labels errors. The raw evidence under the `capture`
key is skipped; any other unknown key is an error. The entry is checked to agree
with itself: the full version starts with the major version, the user agent names
the browser at that major version (`Chrome/155`), and the header spec is valid.

<ParamField body="name" type="&str" required>

A label for errors, such as the file name.

</ParamField>

<ParamField body="json" type="&str" required>

The text of the file, at most `MAX_MEASURED_JSON_BYTES` (16 MiB).

</ParamField>

<ResponseField name="returns" type="Result<WireMeasurement, WireMeasurementError>">

`Parse` when the text is not a measured file, `TooLarge` above the limit,
`Inconsistent` when the entry contradicts itself, `Headers` when the header spec
is invalid.

</ResponseField>

### `WireMeasurement::to_json`

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn to_json(&self) -> Result<String, WireMeasurementError>
```

The entry as pretty-printed JSON, without the raw evidence. Returns
`WireMeasurementError::Parse` when serialization fails, which a valid entry never does.

### `WireMeasurement::file_stem`

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn file_stem(&self) -> String
```

The stem the capture tools give this entry's files, such as `chrome-155-macos`.
An operating system without measured files is spelled `other`.

## `Provenance`

<Badge color="blue" shape="pill">struct</Badge>

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub captured_on: String,
    pub tool: String,
    pub browser: String,
    #[serde(default)]
    pub known_gaps: Vec<KnownGap>,
}
```

Where and how a measured entry was captured.

<ResponseField name="captured_on" type="String" required>

The date of the last capture, `YYYY-MM-DD` (`2026-10-11` for the Chrome 155 entry).

</ResponseField>

<ResponseField name="tool" type="String" required>

The tools that wrote the entry, such as
`capture_h2 0.1.0; capture_hints 0.1.0; capture_tls 0.1.0`.

</ResponseField>

<ResponseField name="browser" type="String" required>

The browser that was measured, such as `Chrome 155.0.8059.40`.

</ResponseField>

<ResponseField name="known_gaps" type="Vec<KnownGap>" default="[]">

What a BoringSSL client cannot reproduce of this browser's hello. `KnownGap` is
re-exported here from [`tls_spec`](/stealth/api/codec-tls-spec#knowngap).

</ResponseField>

## `WireMeasurementError`

<Badge color="purple" shape="pill">enum</Badge>

```rust
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum WireMeasurementError {
    NotMeasured { kind: BrowserKind, os: OsType },
    VersionNotMeasured { kind: BrowserKind, major: u32, os: OsType },
    Parse { name: String, reason: String },
    TooLarge { name: String },
    Inconsistent { name: String, reason: &'static str },
    Headers { name: String, source: HeaderSpecError },
    Duplicate { first: String, second: String },
}
```

Why a measured table could not be read or looked up.

<ResponseField name="WireMeasurementError" type="enum">

<Expandable title="variants" defaultOpen>

<ResponseField name="NotMeasured { kind, os }">

No capture exists for this browser on this operating system. The message says to
run the capture tools on that system.

</ResponseField>

<ResponseField name="VersionNotMeasured { kind, major, os }">

The browser is measured on this system, but not in this major version.

</ResponseField>

<ResponseField name="Parse { name, reason }">

A measured file is not valid JSON of the expected shape; `reason` is what the
JSON parser said.

</ResponseField>

<ResponseField name="TooLarge { name }">

A measured file is larger than `MAX_MEASURED_JSON_BYTES`.

</ResponseField>

<ResponseField name="Inconsistent { name, reason }">

A measured file contradicts itself, such as `full_version does not start with major`.

</ResponseField>

<ResponseField name="Headers { name, source }">

The header spec of a measured file is invalid.

</ResponseField>

<ResponseField name="Duplicate { first, second }">

Two measured files describe the same browser, major version and system.

</ResponseField>

</Expandable>

</ResponseField>

## `get`

<Badge color="yellow" shape="pill">fn</Badge>

```rust
pub fn get(kind: BrowserKind, os: OsType) -> Result<&'static WireMeasurement, WireMeasurementError>
```

The newest measured entry of a browser on an operating system.

<ResponseField name="returns" type="Result<&'static WireMeasurement, WireMeasurementError>">

`WireMeasurementError::NotMeasured { kind, os }` when the pair has no capture.

</ResponseField>

## `get_version`

<Badge color="yellow" shape="pill">fn</Badge>

```rust
pub fn get_version(
    kind: BrowserKind,
    os: OsType,
    major: u32,
) -> Result<&'static WireMeasurement, WireMeasurementError>
```

The measured entry of a browser at one major version on an operating system.
`NotMeasured` when the pair has no capture at all, `VersionNotMeasured` when it
has captures but not at `major`.

## `for_kind`

<Badge color="yellow" shape="pill">fn</Badge>

```rust
pub fn for_kind(kind: BrowserKind) -> &'static WireMeasurement
```

The newest measured entry of a browser, whatever the system. It panics if `kind`
has no entry; the unit test `every_browser_kind_has_an_entry` rules that out.

## `all`

<Badge color="yellow" shape="pill">fn</Badge>

```rust
pub fn all() -> &'static [WireMeasurement]
```

Every measured entry, parsed once from the embedded JSON. It panics if an embedded
file is broken; the unit tests parse every file, so a broken file cannot reach a
release. Use `try_all` to see the error instead.

## `try_all`

<Badge color="yellow" shape="pill">fn</Badge>

```rust
pub fn try_all() -> Result<&'static [WireMeasurement], &'static WireMeasurementError>
```

Every measured entry, or the first error met while parsing the embedded files.

## `newest`, `newest_on`

<Badge color="yellow" shape="pill">fn</Badge>

```rust
pub fn newest(entries: &[WireMeasurement], kind: BrowserKind) -> Option<&WireMeasurement>
pub fn newest_on(entries: &[WireMeasurement], kind: BrowserKind, os: OsType) -> Option<&WireMeasurement>
```

The entry with the highest major version of `kind` (on `os`) among `entries`; the
first of equals wins. They work on any slice, such as a table of your own read
with `WireMeasurement::from_json`.

## `measured_systems`

<Badge color="yellow" shape="pill">fn</Badge>

```rust
pub fn measured_systems(kind: BrowserKind) -> Vec<OsType>
```

The operating systems with a measured entry for `kind`, without repeats.

## `source_names`

<Badge color="yellow" shape="pill">fn</Badge>

```rust
pub fn source_names() -> Vec<&'static str>
```

The names of the embedded measured files: today `["chrome-155-macos.json"]`. A
file added to `src/measured` must be added to the crate's list of sources too; the
test `tests/wire_measurement.rs` fails when one is missing.

## Constants

<Badge color="gray" shape="pill">const</Badge>

```rust
pub const MAX_MEASURED_JSON_BYTES: usize = 16 * 1024 * 1024;
```

The largest measured file `WireMeasurement::from_json` reads, in bytes.

## Related

<Columns cols={2}>

<Card color="#693acf" title="TlsSpec" icon="lock" href="/stealth/api/codec-tls-spec">

The `tls` and `tls_websocket` fields, and `KnownGap`.

</Card>

<Card color="#1a85d4" title="Wire formats guide" icon="book" href="/stealth/advanced/wire-formats">

What is measured, what is refused, and how to re-measure.

</Card>

</Columns>
