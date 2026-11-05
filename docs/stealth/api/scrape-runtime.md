---
title: "scrape::runtime"
sidebarTitle: "Runtime"
description: "CDP helpers, the egress switch, the identity file, state stores, the tracing sink and the capture tools."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: scrape</Badge>

These modules are the I/O around the pure scrape types, other than `BrowserScraper`. `egress_switch` opens a browser context behind a new proxy. `json_store` and `identity_file` persist state and identities. `tracing_sink` writes events to `tracing`. `capture` is the `capture_tls` and `capture_hints` measurement tools.

The programs are on [Runnable examples](/scraping/advanced-stealthy/examples). `BrowserScraper` has its own page.

<RequestExample>

```rust Open a state file
use seleniumbase_rs_stealth::scrape::ledger::state::StateError;
use seleniumbase_rs_stealth::scrape::JsonStateStore;

fn open(path: &str) -> Result<JsonStateStore, StateError> {
    JsonStateStore::open(path)
}
```

</RequestExample>

## Constants and type aliases

```rust
// runtime/capture/args.rs
pub const DEFAULT_OUT_DIR: &str = "../codec/src/measured";
pub const DEFAULT_FIXTURES_DIR: &str = "../codec/tests/fixtures/measured";
pub const MAX_PROBES: u64 = 64;
pub const MAX_TIMEOUT_SECS: u64 = 600;
pub const USAGE: &str = "\
// runtime/capture/chrome.rs
pub const CAPTURE_DOMAIN: &str = "capture.test";
pub const WEBSOCKET_HOST: &str = "ws.capture.test";
pub const DEFAULT_PAGE_PROBES: usize = 4;
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
pub const DEFAULT_LANG: &str = "en-US";
// runtime/capture/hints.rs
pub const ACCEPT_CH: &str = "Sec-CH-UA-Arch, Sec-CH-UA-Bitness, Sec-CH-UA-Form-Factors, \
pub const HIGH_ENTROPY_KEYS: [&str; 8] = [
// runtime/capture/stream.rs
pub const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(5);
pub const MAX_HTTP_HEAD: usize = 16 * 1024;
pub const MAX_HTTP_BODY: usize = 64 * 1024;
pub const MAX_HTTP_HEADERS: usize = 100;
// runtime/cdp.rs
pub const CLEARANCE_POLL_INTERVAL: Duration = Duration::from_millis(250);
// runtime/identity_file.rs
pub const MAX_IDENTITY_FILE_BYTES: u64 = 16 * 1024 * 1024;
// runtime/json_store.rs
pub const MAX_STATE_FILE_BYTES: u64 = 16 * 1024 * 1024;
// runtime/tracing_sink.rs
pub const TARGET: &str = "seleniumbase_rs_scrape";
```


- `DEFAULT_OUT_DIR`: Where the measured files go, relative to the folder of the crate that runs the tool. The tools run from the scrape and mimic crates, which sit beside the codec crate that embeds the files.
- `DEFAULT_FIXTURES_DIR`: Where the raw byte fixtures go, relative to the folder of the crate that runs the tool: the tests of the codec crate, next to the measured files.
- `MAX_PROBES`: The most TLS probe connections `--probes` accepts.
- `MAX_TIMEOUT_SECS`: The longest `--timeout` accepted, in seconds.
- `USAGE`: What every capture tool prints for `--help`.
- `CAPTURE_DOMAIN`: The domain every probe host lives under: `*.capture.test` is mapped to the loopback listener.
- `WEBSOCKET_HOST`: The host name the WebSocket probe connects to, to tell its hello from the page hellos.
- `DEFAULT_PAGE_PROBES`: How many ordinary HTTPS connections the TLS probe makes by default.
- `DEFAULT_TIMEOUT`: How long a measurement waits for the browser by default.
- `DEFAULT_LANG`: The `Accept-Language` locale the browser is started with by default.
- `ACCEPT_CH`: The hints the page asks for beyond the low-entropy ones the browser always sends.
- `HIGH_ENTROPY_KEYS`: The high-entropy values `navigator.userAgentData.getHighEntropyValues` is asked for.
- `DEFAULT_READ_TIMEOUT`: How long a reader waits for the first message of a connection.
- `MAX_HTTP_HEAD`: The most bytes an HTTP/1 request line and headers may take.
- `MAX_HTTP_BODY`: The most bytes of an HTTP/1 request body that are read.
- `MAX_HTTP_HEADERS`: The most header lines an HTTP/1 request may have.
- `CLEARANCE_POLL_INTERVAL`: How often a wait for a clearance looks at the browser again.
- `MAX_IDENTITY_FILE_BYTES`: The largest identity file that is read. A jar at its bounds fits well inside it.
- `MAX_STATE_FILE_BYTES`: The largest state file that is read. A state file this big holds far more sites than [`MAX_STATE_SITES`] allows, so something else wrote it.
- `TARGET`: The `tracing` target of every event this sink writes.

## `runtime::capture`

### `CaptureError`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum CaptureError {
    Io(#[from] io::Error),
    Timeout {
            /// What was awaited.
            what: &'static str,
        },
    Closed {
            /// The bytes that had arrived.
            have: usize,
        },
    TooLarge {
            /// The limit that was exceeded.
            limit: usize,
        },
    Hello(#[from] ClientHelloError),
    H2(#[from] H2CaptureError),
    Http {
            /// What was malformed.
            reason: &'static str,
        },
    Spec(#[from] SpecError),
    Unstable {
            /// The JA4 of the first hello.
            first: String,
            /// The JA4 of the hello that differs.
            other: String,
        },
    NoHello {
            /// The probe that sent nothing.
            what: &'static str,
        },
    Hex,
    Json(#[from] serde_json::Error),
    NotAnObject {
            /// The file.
            path: String,
        },
    Product(String),
    UnknownOs,
    Headless,
    HintsDisagree {
            /// The request header that disagrees.
            header: String,
        },
    HintsReport {
            /// What was wrong.
            reason: &'static str,
        },
    NoUserAgentData,
    Browser(#[from] SeleniumBaseError),
    Usage(String),
}
```

Why a capture failed.

## `runtime::capture::args`

### `ToolArgs`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct ToolArgs {
    pub out_dir: PathBuf,
    pub fixtures_dir: PathBuf,
    pub chrome: ChromeOptions,
    pub snippet: bool,
    pub help: bool,
}
```

The parsed command line of a capture tool.

### `ToolArgs` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn parse<I>(args: I, root: &Path) -> Result<Self, CaptureError>
    where
        I: IntoIterator<Item = String>,
```


- `parse`: Parses `args` (without the program name). Relative defaults are resolved against `root`.

## `runtime::capture::chrome`

### `ChromeOptions`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct ChromeOptions {
    pub executable: Option<PathBuf>,
    pub headless: bool,
    pub lang: String,
    pub timeout: Duration,
    pub no_sandbox: bool,
    pub page_probes: usize,
}
```

How to start the browser that is measured.

## `runtime::capture::hints`

### `Brand`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct Brand {
    pub brand: String,
    pub version: String,
}
```

A brand and version as `navigator.userAgentData` lists them.

### `HighEntropy`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct HighEntropy {
    pub architecture: String,
    pub bitness: String,
    pub form_factors: Vec<String>,
    pub full_version_list: Vec<Brand>,
    pub model: String,
    pub platform_version: String,
    pub ua_full_version: Option<String>,
    pub wow64: bool,
}
```

The high-entropy values of `navigator.userAgentData`.

### `UaData`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct UaData {
    pub brands: Vec<Brand>,
    pub mobile: bool,
    pub platform: String,
    pub high: HighEntropy,
}
```

`navigator.userAgentData`, low- and high-entropy values together.

### `HintsReport`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct HintsReport {
    pub user_agent: String,
    pub languages: Vec<String>,
    pub language: String,
    pub platform: String,
    pub secure_context: bool,
    pub ua_data: Option<UaData>,
}
```

What the hints page read from the browser.

### `HintsMeasurement`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct HintsMeasurement {
    pub user_agent: String,
    pub accept_language: String,
    pub languages: Vec<String>,
    pub client_hints: ClientHints,
    pub first_visit_headers: Vec<(String, String)>,
    pub request_headers: Vec<(String, String)>,
}
```

What the hints capture learned about a browser.

### `HintsServer`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct HintsServer
```

A web site on a loopback port that makes a browser report its client hints.

### `HintsServer` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub async fn start() -> Result<Self, CaptureError>
pub fn addr(&self) -> SocketAddr
pub fn url(&self) -> String
pub fn requests(&self) -> Vec<Http1Request>
pub async fn next_report(&self, within: Duration) -> Result<HintsReport, CaptureError>
```


- `start`: Starts serving on `127.0.0.1` and a port the system picks.
- `addr`: The address the server listens on.
- `url`: The URL of the hints page. It is on `127.0.0.1`, which browsers treat as a secure context.
- `requests`: Every request the server has answered, in arrival order.
- `next_report`: Waits for the next report the page posts.

## `runtime::capture::record`

### `BrowserVersion`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct BrowserVersion {
    pub product: String,
    pub full_version: String,
    pub major: u32,
    pub user_agent: String,
}
```

The browser a capture ran against, as `Browser.getVersion` describes it.

### `BrowserVersion` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn from_get_version(result: &Value) -> Result<Self, CaptureError>
pub fn os(&self) -> Result<OsType, CaptureError>
pub fn is_headless(&self) -> bool
pub fn label(&self) -> String
```


- `from_get_version`: Reads the `product` and `userAgent` fields of a `Browser.getVersion` result.
- `os`: The operating system the user agent names.
- `is_headless`: Whether the user agent is a headless browser's.
- `label`: The browser as the provenance names it, such as `Chrome 155.0.8059.40`.

### `EntryId`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct EntryId {
    pub kind: BrowserKind,
    pub major: u32,
    pub os: OsType,
}
```

Which measured file a capture belongs to.

### `EntryId` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn file_name(&self) -> String
pub fn stem(&self) -> String
```


- `file_name`: The file name, such as `chrome-155-macos.json`.
- `stem`: The stem shared by the file and its raw fixtures, such as `chrome-155-macos`. A browser or a system added to the core after this was written is spelled `unknown`.

### `HelloRecord`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct HelloRecord {
    pub sni: Option<String>,
    pub raw_hex: String,
    pub ja3: String,
    pub ja3_hash: String,
    pub ja4: String,
    pub ja4_r: String,
    pub ja4_ro: String,
    pub peetprint: String,
    pub peetprint_hash: String,
}
```

One captured `ClientHello` and the fingerprints computed from its bytes.

### `HelloRecord` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn of(raw: &[u8]) -> Result<Self, CaptureError>
pub fn raw(&self) -> Result<Vec<u8>, CaptureError>
pub fn hello(&self) -> Result<ClientHello, CaptureError>
```


- `of`: Parses `raw` as a hello and computes its fingerprints.
- `raw`: The raw bytes.
- `hello`: The parsed hello.

### `TlsMeasurement`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct TlsMeasurement {
    pub spec: TlsSpec,
    pub websocket_spec: Option<TlsSpec>,
    pub samples: Vec<HelloRecord>,
    pub websocket_sample: Option<HelloRecord>,
    pub known_gaps: Vec<KnownGap>,
}
```

What the TLS capture learned about a browser.

### `TlsMeasurement` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn from_hellos(page: &[Vec<u8>], websocket: Option<&[u8]>) -> Result<Self, CaptureError>
```


- `from_hellos`: Builds a measurement from the raw bytes of several hellos of one browser and, optionally, a WebSocket hello.

### `MeasuredFile`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct MeasuredFile
```

A measured JSON file being filled in by the tools.

### `MeasuredFile` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn open(dir: &Path, id: EntryId) -> Result<Self, CaptureError>
pub fn path(&self) -> &Path
pub fn get(&self, key: &str) -> Option<&Value>
pub fn set_identity(&mut self, id: EntryId, version: &BrowserVersion)
pub fn set_tls(&mut self, tls: &TlsMeasurement) -> Result<(), CaptureError>
pub fn set_hints(&mut self, hints: &HintsMeasurement) -> Result<(), CaptureError>
pub fn stamp(&mut self, tool: &str, version: &BrowserVersion, now: DateTime<Utc>)
pub fn to_pretty_json(&self) -> Result<String, CaptureError>
pub fn save(&self) -> Result<(), CaptureError>
```


- `open`: Opens `&lt;dir&gt;/&lt;id file name&gt;`, or starts an empty document when it does not exist.
- `path`: The file this document is saved to.
- `get`: The value stored under a top-level key.
- `set_identity`: Sets `kind`, `major`, `full_version` and `os`.
- `set_tls`: Sets `tls`, `tls_websocket` and the evidence under `capture.tls`, and records the known gaps.
- `set_hints`: Sets `user_agent`, `client_hints` and the evidence under `capture.hints`.
- `stamp`: Updates `provenance`: the date, the browser and the tool, which is merged with the tools that already wrote the file.
- `to_pretty_json`: The document as pretty-printed JSON with a final newline.
- `save`: Writes the file, creating its directory if needed.

## `runtime::capture::stream`

### `CapturedHello`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct CapturedHello {
    pub peer: SocketAddr,
    pub raw: Vec<u8>,
    pub hello: ClientHello,
}
```

A `ClientHello` read from a connection.

### `CapturedH2c`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct CapturedH2c {
    pub peer: SocketAddr,
    pub raw: Vec<u8>,
    pub preface: H2Preface,
}
```

The start of an HTTP/2 connection read from a plain TCP connection.

### `CapturedHttp1`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct CapturedHttp1 {
    pub peer: SocketAddr,
    pub request: Http1Request,
}
```

An HTTP/1 request read from a connection.

### `Http1Request`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct Http1Request {
    pub method: String,
    pub target: String,
    pub version: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}
```

An HTTP/1 request with its header names, values and order exactly as sent.

### `Http1Request` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn header(&self, name: &str) -> Option<&str>
pub fn path(&self) -> &str
```


- `header`: The value of the first header named `name`, compared without regard to ASCII case.
- `path`: The path of the target, without a query string.

### `Capture`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct Capture
```

A listener on a loopback port that reads the first message of each connection it accepts.

### `Capture` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub async fn bind() -> Result<Self, CaptureError>
pub fn with_timeout(mut self, timeout: Duration) -> Self
pub fn addr(&self) -> SocketAddr
pub fn port(&self) -> u16
pub async fn accept_client_hello(&self) -> Result<CapturedHello, CaptureError>
pub async fn collect_client_hellos(
        &self,
        count: usize,
        within: Duration,
    ) -> Vec<CapturedHello>
pub async fn collect_client_hellos_until(
        &self,
        within: Duration,
        mut done: impl FnMut(&[CapturedHello]) -> bool,
    ) -> Vec<CapturedHello>
pub async fn accept_h2c(&self) -> Result<CapturedH2c, CaptureError>
pub async fn accept_http1(&self, response: &[u8]) -> Result<CapturedHttp1, CaptureError>
```


- `bind`: Binds `127.0.0.1` on a port the system picks.
- `with_timeout`: Replaces how long a connection may take to send its first message.
- `addr`: The address the listener is bound to.
- `port`: The port the listener is bound to.
- `accept_client_hello`: Accepts one connection, reads its `ClientHello` and drops the connection.
- `collect_client_hellos`: Accepts connections until `count` hellos have arrived or `within` has passed, and returns the hellos that arrived.
- `collect_client_hellos_until`: Accepts connections until `done` accepts the hellos collected so far or `within` has passed, and returns them in arrival order.
- `accept_h2c`: Accepts one connection, reads an HTTP/2 connection start, answers it with [`h2c_reply`] and closes.
- `accept_http1`: Accepts one connection, reads an HTTP/1 request, sends `response` and closes.

## `runtime::cdp`

### `NavResponse`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct NavResponse {
    pub url: String,
    pub status: u16,
    pub headers: Vec<(String, String)>,
}
```

What the server answered the main frame of a navigation.

### `PageSnapshot`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct PageSnapshot {
    pub detection: Detection,
    pub body: String,
}
```

The page as the browser rendered it, with the verdict on it.

### `EgressLocale`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct EgressLocale {
    pub locale: &'static Locale,
    pub timezone: String,
}
```

The locale and time zone an egress puts a tab in.

### `EgressLocale` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub async fn resolve(
        resolver: &dyn GeoResolver,
        egress: &EgressRef,
    ) -> Result<Self, ScrapeError>
pub fn localize(&self, fingerprint: &Fingerprint) -> Result<Fingerprint, ScrapeError>
```


- `resolve`: Asks `resolver` where `egress` is and finds the locale of that country.
- `localize`: The fingerprint `fingerprint` becomes in this locale: its language, `Accept-Language`, time zone and position are this locale's, with the time zone this egress reported, and every other setting is kept. `fingerprint` is left as it was. See [`LocaleExt::localize`].

## `runtime::egress_switch`

### `EgressSwitch`

<Badge color="blue" shape="pill">trait</Badge>

```rust
pub trait EgressSwitch
```

Changes where a browser's traffic leaves from.

### `EgressSwitch` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
async fn switch(
        &self,
        browser: &Browser,
        egress: Option<&EgressRef>,
    ) -> Result<Page, ScrapeError>
```


- `switch`: Opens a tab in `browser` that leaves through `egress`, or directly when `egress` is `None`.

### `ContextProxySwitch`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct ContextProxySwitch
```

An [`EgressSwitch`] that opens a new browser context with its own proxy and drops the one it opened before.

### `ContextProxySwitch` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn new() -> Self
```


- `new`: A switch that has opened no context yet.

## `runtime::identity_file`

### `IdentityFileError`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum IdentityFileError {
    Io {
            /// The identity file.
            path: PathBuf,
            /// What the operating system said.
            #[source]
            source: io::Error,
        },
    Corrupt {
            /// The identity file.
            path: PathBuf,
            /// In which way it is not valid. Never the value that was met.
            reason: &'static str,
            /// The line where the parse failed, from one.
            line: usize,
            /// The column where the parse failed, from one.
            column: usize,
        },
    TooLarge {
            /// The identity file.
            path: PathBuf,
            /// The most it may hold.
            limit: u64,
        },
    Owner {
            /// The identity file.
            path: PathBuf,
        },
    Permissions {
            /// The identity file.
            path: PathBuf,
            /// Its permission bits.
            mode: u32,
        },
    Identity {
            /// The identity file.
            path: PathBuf,
            /// Why. Boxed, so that this error stays small to return.
            #[source]
            source: Box<IdentityError>,
        },
}
```

Why an identity file could not be read or written.

### `IdentityFileError` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn is_not_found(&self) -> bool
```


- `is_not_found`: Whether the file does not exist.

## `runtime::json_store`

### `JsonStateStore`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct JsonStateStore
```

A state store backed by one JSON file.

### `JsonStateStore` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn open(path: impl Into<PathBuf>) -> Result<Self, StateError>
pub fn path(&self) -> &Path
```


- `open`: Opens the store at `path`, reading the file if there is one. This reads the file at once on the calling thread; do it at start-up.
- `path`: The file this store writes.

## `runtime::tracing_sink`

### `TracingEventSink`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct TracingEventSink;
```

An event sink that writes to `tracing`.

## Functions

### `host_resolver_rule`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn host_resolver_rule() -> String
```

The Chrome argument that sends every `*.capture.test` host to the loopback interface.

### `probe_script`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn probe_script(port: u16, probes: usize) -> String
```

The script that makes the browser open `probes` HTTPS connections and one WebSocket to `port` on the probe hosts. It does not wait for them, since none can complete.

### `measure_tls`

<Badge color="green" shape="pill">function</Badge>

```rust
pub async fn measure_tls(
    options: &ChromeOptions,
) -> Result<(BrowserVersion, TlsMeasurement), CaptureError>
```

Measures the TLS `ClientHello` of the browser.

### `measure_hints`

<Badge color="green" shape="pill">function</Badge>

```rust
pub async fn measure_hints(
    options: &ChromeOptions,
) -> Result<(BrowserVersion, HintsMeasurement), CaptureError>
```

Measures the client hints, `User-Agent` and `Accept-Language` of the browser.

### `collect_js`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn collect_js() -> String
```

The JavaScript that reads the browser's identity: defines `collectHints()`.

### `page_html`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn page_html() -> String
```

The page the hints server serves at `/`.

### `console_snippet`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn console_snippet() -> String
```

A snippet to paste into the developer console of a browser this tool cannot drive: it prints the same JSON the page posts.

### `parse_brand_list`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn parse_brand_list(value: &str) -> Result<Vec<BrandVersion>, CaptureError>
```

Parses a structured-field list of `"brand";v="version"` items, as `Sec-CH-UA` is written.

### `parse_string_list`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn parse_string_list(value: &str) -> Result<Vec<String>, CaptureError>
```

Parses a structured-field list of strings, such as `"Desktop", "EmbeddedDevice"`.

### `client_hints_from`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn client_hints_from(report: &HintsReport) -> Result<ClientHints, CaptureError>
```

Turns the page's report into the profile's [`ClientHints`].

### `cross_check`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn cross_check(hints: &ClientHints, headers: &[(String, String)]) -> Result<(), CaptureError>
```

Checks the hints against the `Sec-CH-UA` request headers the browser sent. A header the browser did not send is skipped; one it sent must say the same.

### `measurement_from`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn measurement_from(
    report: &HintsReport,
    requests: &[Http1Request],
) -> Result<HintsMeasurement, CaptureError>
```

Builds the measurement from the page's report and the requests the server recorded.

### `merge_tool`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn merge_tool(existing: Option<&str>, tool: &str) -> String
```

Joins `tool` into the `; `-separated list in `existing`, replacing a tool of the same name.

### `write_fixtures`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn write_fixtures(
    dir: &Path,
    id: EntryId,
    tls: &TlsMeasurement,
) -> Result<Vec<PathBuf>, CaptureError>
```

Writes the raw hello bytes as binary fixtures next to the measured file's tests.

### `to_hex`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn to_hex(bytes: &[u8]) -> String
```

Lowercase hexadecimal of `bytes`.

### `from_hex`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn from_hex(text: &str) -> Result<Vec<u8>, CaptureError>
```

Decodes hexadecimal (either case).

### `read_client_hello`

<Badge color="green" shape="pill">function</Badge>

```rust
pub async fn read_client_hello<R>(
    reader: &mut R,
    within: Duration,
) -> Result<(Vec<u8>, ClientHello), CaptureError>
where
    R: AsyncRead + Unpin,
```

Reads the first `ClientHello` from `reader`.

### `read_h2c_preface`

<Badge color="green" shape="pill">function</Badge>

```rust
pub async fn read_h2c_preface<R>(
    reader: &mut R,
    within: Duration,
) -> Result<(Vec<u8>, H2Preface), CaptureError>
where
    R: AsyncRead + Unpin,
```

Reads an HTTP/2 connection start (prior knowledge, no TLS) from `reader`.

### `read_http1_request`

<Badge color="green" shape="pill">function</Badge>

```rust
pub async fn read_http1_request<R>(
    reader: &mut R,
    within: Duration,
) -> Result<Http1Request, CaptureError>
where
    R: AsyncRead + Unpin,
```

Reads one HTTP/1 request, with the body `Content-Length` announces.

### `h2c_reply`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn h2c_reply(request_stream_id: u32) -> Result<Vec<u8>, CaptureError>
```

The bytes a server sends to let an HTTP/2 client finish its first request: an empty SETTINGS frame, the acknowledgement of the client's SETTINGS, and a `200` response without a body.

### `http1_response`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn http1_response(status: u16, reason: &str, headers: &[(&str, &str)], body: &[u8]) -> Vec<u8>
```

An HTTP/1 response with a `Content-Length` and `Connection: close`.

### `goto_with_response`

<Badge color="green" shape="pill">function</Badge>

```rust
pub async fn goto_with_response(page: &Page, url: &str) -> Result<NavResponse, ScrapeError>
```

Navigates `page` to `url` and returns what the server answered the main frame.

### `detect_page`

<Badge color="green" shape="pill">function</Badge>

```rust
pub async fn detect_page(page: &Page, response: &NavResponse) -> Result<PageSnapshot, ScrapeError>
```

Reads the rendered page and decides whether it is a challenge, using the status and headers of `response`.

### `apply_locale`

<Badge color="green" shape="pill">function</Badge>

```rust
pub async fn apply_locale(page: &Page, placed: &EgressLocale) -> Result<(), ScrapeError>
```

Makes `page` report the locale and time zone of `placed` over the protocol, for the documents it loads from now on.

### `export_cookies`

<Badge color="green" shape="pill">function</Badge>

```rust
pub async fn export_cookies(page: &Page, now: u64) -> Result<CookieJar, ScrapeError>
```

The cookies of the tab's browser context, as a jar. Cookies the jar would refuse are left out.

### `import_cookies`

<Badge color="green" shape="pill">function</Badge>

```rust
pub async fn import_cookies(page: &Page, jar: &CookieJar, now: u64) -> Result<usize, ScrapeError>
```

Sets the unexpired cookies of `jar` in the tab's browser context and returns how many.

### `clearance_value`

<Badge color="green" shape="pill">function</Badge>

```rust
pub async fn clearance_value(
    page: &Page,
    vendor: Option<Vendor>,
) -> Result<Option<String>, ScrapeError>
```

The value of the clearance cookie of `vendor` in the tab's browser context, when there is one. A vendor without such a cookie gives `None`.

### `wait_for_clearance`

<Badge color="green" shape="pill">function</Badge>

```rust
pub async fn wait_for_clearance(
    page: &Page,
    vendor: Option<Vendor>,
    stale: Option<&str>,
    timeout: Duration,
) -> Result<bool, ScrapeError>
```

Waits until a solved challenge shows in the browser, for at most `timeout`.

### `capture_session`

<Badge color="green" shape="pill">function</Badge>

```rust
pub async fn capture_session(page: &Page) -> Result<Session, ScrapeError>
```

Captures the cookies of the tab's browser context and the local storage of the tab's origin as a [`Session`], the way a pool lease saves its login.

### `restore_session`

<Badge color="green" shape="pill">function</Badge>

```rust
pub async fn restore_session(page: &Page, session: Session, now: f64) -> Result<(), ScrapeError>
```

Puts a [`Session`] into the tab: its unexpired cookies, and its local storage when the tab is on the origin it was captured from.

### `human_warm_up`

<Badge color="green" shape="pill">function</Badge>

```rust
pub async fn human_warm_up(page: &Page, seed: u64)
```

Lets the pointer arrive on the page the way a person's would.

### `solve_widget`

<Badge color="green" shape="pill">function</Badge>

```rust
pub async fn solve_widget(page: &Page) -> Result<(), ScrapeError>
```

Attempts the CAPTCHA widget the page shows, if it shows one, with the solver of the core ([`Page::solve_captcha_with`]).

### `save_blocking`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn save_blocking(path: &Path, identity: &StealthIdentity) -> Result<(), IdentityFileError>
```

Writes `identity` to `path`, creating the directories above it. This blocks on the file system; use [`save`] from async code.

### `load_blocking`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn load_blocking(path: &Path) -> Result<StealthIdentity, IdentityFileError>
```

Reads the identity at `path`. This blocks on the file system; use [`load`] from async code.

### `save`

<Badge color="green" shape="pill">function</Badge>

```rust
pub async fn save(
    path: impl AsRef<Path>,
    identity: &StealthIdentity,
) -> Result<(), IdentityFileError>
```

Writes `identity` to `path` without blocking the async threads. See [`save_blocking`].

### `load`

<Badge color="green" shape="pill">function</Badge>

```rust
pub async fn load(path: impl AsRef<Path>) -> Result<StealthIdentity, IdentityFileError>
```

Reads the identity at `path` without blocking the async threads. See [`load_blocking`].

### `atomic_write`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()>
```

Replaces the file at `path` with `bytes`, so that a reader or a crash never sees a mixture.

## Related

The guide is [Runnable examples](/scraping/advanced-stealthy/examples). The overview is [scrape](/stealth/api/scrape).
