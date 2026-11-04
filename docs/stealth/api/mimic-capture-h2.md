---
title: "mimic::capture_h2"
sidebarTitle: "HTTP/2 capture"
description: "capture_h2 records a local Chrome's HTTP/2 frames and header block."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: mimic</Badge>

`capture_h2` serves a local HTTPS site, points the local Chrome at it, and records the HTTP/2 frames and the header block, plus an HTTP/1.1 pass, so a new browser can be added to the measured table. It is a measurement tool, not a client you send traffic through.

The programs are on [Runnable examples](/scraping/advanced-stealthy/examples).

<RequestExample>

```rust The pass flags
use seleniumbase_rs_stealth::mimic::capture_h2::{Passes, FLAG_H2};

fn main() {
    assert_eq!(FLAG_H2, "--h2");
    let _passes = Passes::Both;
}
```

</RequestExample>

## Constants and type aliases

```rust
// capture_h2.rs
pub const FLAG_H1: &str = "--h1";
pub const FLAG_H2: &str = "--h2";
pub const USAGE_PASSES: &str = "\
// capture_h2/probes.rs
pub const DOCUMENT: Probe = Probe {
pub const FETCH: Probe = Probe {
pub const IMAGE: Probe = Probe {
pub const SCRIPT: Probe = Probe {
pub const STYLE: Probe = Probe {
pub const PROBES: [Probe; 5] = [DOCUMENT, FETCH, IMAGE, SCRIPT, STYLE];
pub const DOCUMENT_BODY: &[u8] = b"<!doctype html><meta charset=\"utf-8\"><title>capture</title>";
// capture_h2/site.rs
pub const ALPN_H2: &[u8] = b"h2";
```


- `FLAG_H1`: The flag that runs only the HTTP/1.1 pass.
- `FLAG_H2`: The flag that runs only the HTTP/2 pass.
- `USAGE_PASSES`: What `--help` adds to the options the capture tools share.
- `DOCUMENT`: The navigation: the browser is pointed at the document probe's address.
- `FETCH`: A script-issued request, made with `fetch` from the document page.
- `IMAGE`: An image the page loads.
- `SCRIPT`: A script the page loads.
- `STYLE`: A stylesheet the page loads.
- `PROBES`: Every probe, the navigation first.
- `DOCUMENT_BODY`: The HTML the navigation is answered with, so the page has a document to run the probes in.
- `ALPN_H2`: The ALPN id of HTTP/2.

## `capture_h2`

### `CaptureH2Error`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum CaptureH2Error {
    Capture(#[from] CaptureError),
    Tls(#[from] TlsServerError),
    H2(#[from] H2CaptureError),
    Spec(#[from] SpecError),
    Headers(#[from] HeaderSpecError),
    Json(#[from] serde_json::Error),
    UnknownHost,
    NoServerName,
    NotNegotiated {
            /// The protocol the pass needs.
            expected: Protocol,
        },
    MissingProbe {
            /// The probe.
            probe: &'static str,
        },
    WrongDestination {
            /// The probe.
            probe: &'static str,
            /// The destination that arrived.
            found: String,
        },
    ProbesDisagree {
            /// The probe that differs.
            probe: String,
        },
    Evidence {
            /// The probe.
            probe: String,
            /// What is wrong.
            reason: &'static str,
        },
    Credential {
            /// The header.
            header: &'static str,
        },
    NoBase {
            /// The file.
            path: String,
        },
    VersionMismatch {
            /// The version in the file.
            file: String,
            /// The version of the running browser.
            browser: String,
        },
    Store {
            /// What is wrong.
            reason: &'static str,
        },
    DifferentBrowsers {
            /// The version of the first pass.
            first: String,
            /// The version of the second pass.
            second: String,
        },
}
```

Why the HTTP/2 and header capture failed.

### `Passes`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum Passes {
    Both,
    H2Only,
    H1Only,
}
```

Which passes a run makes.

### `Report`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct Report {
    pub version: BrowserVersion,
    pub id: EntryId,
    pub h2: Option<Vec<H2Evidence>>,
    pub h1: Option<Vec<H1Evidence>>,
    pub derived: Option<Derived>,
    pub path: PathBuf,
    pub fixtures: Vec<PathBuf>,
}
```

What a run measured and wrote.

## `capture_h2::assemble`

### `H2Evidence`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct H2Evidence {
    pub probe: String,
    pub sni: String,
    pub raw_hex: String,
    pub akamai: String,
    pub stream_id: u32,
    pub headers_priority: Option<HeadersPriority>,
    pub headers: Vec<(String, String)>,
}
```

One HTTP/2 connection start, as stored under `capture.h2.probes`.

### `H2Evidence` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn of(probe: Probe, raw: &[u8], preface: &H2Preface) -> Result<Self, CaptureH2Error>
pub fn preface(&self) -> Result<H2Preface, CaptureH2Error>
pub fn header(&self, name: &str) -> Option<&str>
```


- `of`: Records the first request of a connection to `probe`.
- `preface`: Decodes the stored bytes again and checks that they say what the record says.
- `header`: The value of the first header named `name`, compared without regard to ASCII case.

### `H1Evidence`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct H1Evidence {
    pub probe: String,
    pub sni: String,
    pub method: String,
    pub version: String,
    pub headers: Vec<(String, String)>,
}
```

One HTTP/1 request, as stored under `capture.h1.probes`.

### `H1Evidence` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn header(&self, name: &str) -> Option<&str>
```


- `header`: The value of the first header named `name`, compared without regard to ASCII case.

### `Derived`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct Derived {
    pub http2: Http2Spec,
    pub headers: HeaderSpec,
    pub differences: Vec<String>,
}
```

What the two passes measured, derived from the evidence.

## `capture_h2::probes`

### `Probe`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct Probe {
    pub name: &'static str,
    pub dest: &'static str,
}
```

What a probe asks the browser for.

### `Probe` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn host(&self) -> String
pub fn for_host(host: &str) -> Option<Probe>
pub fn url(&self, port: u16) -> String
```


- `host`: The probe's host name, such as `fetch.capture.test`.
- `for_host`: The probe whose host name is `host`, compared without regard to ASCII case.
- `url`: The address the probe is reached at on `port`.

## `capture_h2::site`

### `Protocol`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum Protocol {
    H2,
    Http1,
}
```

Which protocol a site speaks; the ALPN list is fixed to it.

### `Protocol` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn alpn(self) -> &'static [u8]
```


- `alpn`: The ALPN id the site accepts.

### `Seen`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum Seen {
    H2 {
            /// The bytes from the connection preface to the end of the first request's header block.
            raw: Vec<u8>,
            /// The decoded frames.
            preface: H2Preface,
        },
    Http1 {
            /// The request.
            request: Http1Request,
        },
}
```

What the first request on a connection looked like.

### `Observed`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct Observed {
    pub probe: Probe,
    pub seen: Seen,
}
```

A connection read to the end of its first request.

### `Site`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct Site
```

A loopback HTTPS site that records the first request on each connection.

### `Site` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub async fn bind(protocol: Protocol, timeout: Duration) -> Result<Self, CaptureH2Error>
pub fn port(&self) -> u16
pub fn chrome_arg(&self) -> String
pub fn ca(&self) -> &EphemeralCa
pub async fn collect_until(
        &self,
        within: Duration,
        mut done: impl FnMut(&[Observed]) -> bool,
    ) -> Vec<Observed>
```


- `bind`: Binds `127.0.0.1` on a port the system picks and creates a CA for this site alone.
- `port`: The port the site listens on.
- `chrome_arg`: The Chrome switch that trusts this site's CA and its leaves, and nothing else.
- `ca`: The site's certificate authority, for a test client that must trust it.
- `collect_until`: Accepts connections until `done` accepts the observations so far or `within` has passed, and returns them in arrival order.

## `capture_h2::store`

### `MeasuredDoc`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct MeasuredDoc
```

A measured JSON file being extended by `capture_h2`.

### `MeasuredDoc` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn open(dir: &Path, id: EntryId, version: &BrowserVersion) -> Result<Self, CaptureH2Error>
pub fn path(&self) -> &Path
pub fn h2_evidence(&self) -> Result<Option<Vec<H2Evidence>>, CaptureH2Error>
pub fn h1_evidence(&self) -> Result<Option<Vec<H1Evidence>>, CaptureH2Error>
pub fn set_h2_evidence(&mut self, evidence: &[H2Evidence])
pub fn set_h1_evidence(&mut self, evidence: &[H1Evidence])
pub fn set_derived(&mut self, derived: &Derived) -> Result<(), CaptureH2Error>
pub fn stamp(&mut self, tool: &str, version: &BrowserVersion, now: DateTime<Utc>)
pub fn to_pretty_json(&self) -> Result<String, CaptureH2Error>
pub fn save(&self) -> Result<(), CaptureH2Error>
```


- `open`: Opens `&lt;dir&gt;/&lt;id file name&gt;`, which must already hold the TLS measurement of `version`.
- `path`: The file this document is saved to.
- `h2_evidence`: The HTTP/2 evidence a previous run stored, when there is any.
- `h1_evidence`: The HTTP/1 evidence a previous run stored, when there is any.
- `set_h2_evidence`: Stores the evidence of the HTTP/2 pass.
- `set_h1_evidence`: Stores the evidence of the HTTP/1 pass.
- `set_derived`: Stores the specs derived from the evidence of both passes, and where the passes disagree.
- `stamp`: Updates the provenance: the date, the browser, and the tool merged with the tools that already wrote the file. The known gaps are kept.
- `to_pretty_json`: The document as pretty-printed JSON with a final newline.
- `save`: Writes the file in place, through a temporary file and a rename.

## Functions

### `parse_args`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn parse_args<I>(args: I, root: &Path) -> Result<(ToolArgs, Passes), CaptureH2Error>
where
    I: IntoIterator<Item = String>,
```

Reads the command line of the tool: the options the capture tools share, and `--h1` or `--h2`.

### `measure_and_write`

<Badge color="green" shape="pill">function</Badge>

```rust
pub async fn measure_and_write(
    args: &ToolArgs,
    passes: Passes,
    tool: &str,
    now: DateTime<Utc>,
) -> Result<Report, CaptureH2Error>
```

Runs the requested passes against the local Chrome and writes the result into the measured file of that browser.

### `h2_evidence`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn h2_evidence(observed: &[Observed]) -> Result<Vec<H2Evidence>, CaptureH2Error>
```

The evidence of the first HTTP/2 connection to each probe, in probe order.

### `h1_evidence`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn h1_evidence(observed: &[Observed]) -> Result<Vec<H1Evidence>, CaptureH2Error>
```

The evidence of the first HTTP/1 request to each probe, in probe order.

### `derive`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn derive(h2: &[H2Evidence], h1: &[H1Evidence]) -> Result<Derived, CaptureH2Error>
```

Derives the HTTP/2 spec and the header spec from the evidence of both passes.

### `run_pass`

<Badge color="green" shape="pill">function</Badge>

```rust
pub async fn run_pass(
    options: &ChromeOptions,
    protocol: Protocol,
) -> Result<(BrowserVersion, Vec<Observed>), CaptureH2Error>
```

Starts a browser that trusts the site's CA and sends every capture host to loopback, makes it request every probe over `protocol`, and returns its version and what the site saw.

### `page_script`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn page_script(port: u16) -> String
```

The script, run in the document page, that makes the browser send the other probes.

### `peek_server_name`

<Badge color="green" shape="pill">function</Badge>

```rust
pub async fn peek_server_name(tcp: &TcpStream, within: Duration) -> Result<String, CaptureH2Error>
```

Reads the server name from the `ClientHello` waiting on `tcp`, leaving the bytes in place for the handshake.

### `h2_reply`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn h2_reply(stream_id: u32, body: Option<&[u8]>) -> Result<Vec<u8>, CaptureH2Error>
```

The bytes a server sends to let an HTTP/2 client finish its first request: an empty SETTINGS frame, the acknowledgement of the client's SETTINGS, and a `200` response.

### `write_h2_fixtures`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn write_h2_fixtures(
    dir: &Path,
    id: EntryId,
    evidence: &[H2Evidence],
) -> Result<Vec<PathBuf>, CaptureH2Error>
```

Writes the raw bytes of each HTTP/2 connection start as `&lt;stem&gt;.h2.&lt;probe&gt;.bin`.

## Related

The guide is [Runnable examples](/scraping/advanced-stealthy/examples). The overview is [mimic](/stealth/api/mimic).
