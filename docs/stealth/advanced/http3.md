---
title: "HTTP/3 through a Helper Program"
sidebarTitle: "HTTP/3"
description: "Send a request over HTTP/3 with Chrome's TLS, QUIC transport parameters, SETTINGS and headers, through the sbrs-h3d helper of the stealth repository. Experimental and off by default."
---

<Badge color="orange">Experimental</Badge>

<Visibility for="agents">

Summary for AI exports: HTTP/3 lives in `seleniumbase_rs_stealth::network::http3` behind the `http3` feature (off by default; it turns `network` on) alongside pure specs in `seleniumbase_rs_stealth::network::spec::h3_spec`. The QUIC stack is phantom-http `=0.6.0` in a standalone helper binary `sbrs-h3d` (`cargo install --path h3d`) driven over stdin/stdout by `Http3Client` or pooled via `Http3Pool`. `Http3Options` supports `for_browser(browser, major, os)`, `for_version(major, os)`, `for_fingerprint(&fp)`, `request_kind(Http3RequestKind::Navigation | Subresource)`, `proxy`, `socks5`, `connect_udp`, `default_header`, `default_headers`, `user_agent`, `default_timeout`, `root_cert_der`, `resolve`, and `pool(max_idle)`. `Http3Client` and `Http3Pool` provide `send`, `get`, `post_json`, and `is_alive`; `Http3Request` adds `headers`, `bearer_auth`, `basic_auth`, `referer`, `origin`, `accept_language`, `cookie`, `query`, `form`, `text_body`, `json`; `Http3Response` adds `headers_all`, `cookies`, `content_type`, status helpers, `text`, `text_lossy`, `json`. `session::HttpSession` supports `HttpSessionOptions::http3_alt_svc(true)` for Alt-Svc discovery with transparent H3-to-H2 fallback, `HttpSessionOptions::root_cert_der(path)`, and explicit `WireProtocol::Http3`. `sbrs-h3d` routes proxies through `socks5://` / `socks5h://` UDP relay and `connect-udp://` / MASQUE `https://` and accepts `--request-kind navigation|subresource`.

</Visibility>

The [wire formats](/stealth/advanced/wire-formats) client sends HTTP/1.1 and
HTTP/2 with Chrome's TLS and HTTP/2 settings through wreq and BoringSSL.
HTTP/3 needs a QUIC stack. The one used here is
[phantom-http](https://crates.io/crates/phantom-http) `=0.6.0`, which brings
its own BoringSSL. The two BoringSSL builds cannot be linked into one binary:
on macOS arm64 the linker crashes, and they export 6,047 of the same symbol
names. So HTTP/3 runs in a separate program, `sbrs-h3d`, and the wire crate
talks to it over stdin and stdout, one JSON object per line.

<Note>

This page covers HTTP/3 sent by the stealth crates without a browser. Whether
a *browser* may use QUIC is a launch setting: see the `quic` setting of
[Proxies and network](/stealth/proxies), and
[HTTP/3 and QUIC](/concepts/modern-web#http3-and-quic) for the background.

</Note>

## Using it

<Steps>

<Step title="Install the helper" icon="download">

`sbrs-h3d` is the standalone package in `h3d/` of the
[stealth repository](https://github.com/MustCodeAl/seleniumbase-rs-stealth).
From a clone of it:

```bash
cargo install --path h3d                   # builds sbrs-h3d (phantom and its BoringSSL)
```

</Step>

<Step title="Turn the feature on" icon="toggle-on">

The `http3` feature turns `network` on too, so it builds BoringSSL and
needs cmake and a C/C++ toolchain.

```toml Cargo.toml
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth", features = ["http3"] }
```

</Step>

<Step title="Send a request" icon="paper-plane">

```rust
use seleniumbase_rs_stealth::identity::OsType;
use seleniumbase_rs_stealth::network::http3::{Http3Client, Http3Options, Http3Request};
use seleniumbase_rs_stealth::network::spec::H3Spec;

async fn demo() -> Result<(), Box<dyn std::error::Error>> {
    // Inspect the current stable Chromium HTTP/3 specification (or chrome_156_macos / chrome(major, os))
    let spec = H3Spec::current_stable();
    assert_eq!(spec.pseudo_headers(), &[":method", ":authority", ":scheme", ":path"]);

    // sbrs-h3d on the PATH, or the program named in SBRS_H3D.
    let options = Http3Options::new().for_version(157, OsType::Macos);
    let mut client = Http3Client::start(&options)?;
    let response = client.send(Http3Request::get("https://example.com/")).await?;
    assert_eq!(response.protocol(), "h3");
    println!("{} {} bytes", response.status(), response.body().len());
    Ok(())
}
```

## Connection Pooling, Proxies, and Request Helpers

Spawning `sbrs-h3d` per request incurs process creation overhead. `Http3Options::pool(max_idle)` (or `Http3Pool::new(options)`) manages a pool of warm helper processes, reusing live workers (`Http3Client::is_alive`) across requests:

```rust
use std::time::Duration;
use seleniumbase_rs_stealth::identity::{BrowserType, OsType};
use seleniumbase_rs_stealth::network::http3::{Http3Options, Http3Request, Http3RequestKind};

let pool = Http3Options::new()
    .for_browser(BrowserType::Chrome, 157, OsType::Macos)
    .request_kind(Http3RequestKind::Subresource)
    .socks5("socks5h://127.0.0.1:1080")
    .default_timeout(Duration::from_secs(15))
    .pool(4);

let res = pool.get("https://example.com/api").await?;
assert!(res.is_success());
let text = res.text()?;
```

### Proxy routing and request kinds

`Http3Options` and `Http3Request` support UDP-capable proxies and browser request shapes (`sbrs-h3d --request-kind navigation|subresource`):

- **SOCKS5 UDP relay**: `.socks5("socks5://127.0.0.1:1080")` or `socks5h://...` (also via `.proxy(...)`).
- **CONNECT-UDP / MASQUE**: `.connect_udp("connect-udp://proxy.example.com:443")` or MASQUE `https://` URI template.
- **TCP-only HTTP proxies rejected**: Classic `http://` CONNECT tunnels cannot carry QUIC UDP datagrams and return a typed error.
- **Request/Response ergonomics**:
  - `Http3Options`: `for_browser`, `for_version`, `for_fingerprint`, `request_kind`, `default_header`, `default_headers`, `user_agent`, `default_timeout`, `root_cert_der`, `resolve`, `pool(max_idle)`.
  - `Http3Client` and `Http3Pool`: `send`, `get`, `post_json`.
  - `Http3Request`: `headers`, `bearer_auth`, `basic_auth`, `referer`, `origin`, `accept_language`, `cookie`, `query`, `form`, `text_body`, `json`.
  - `Http3Response`: `headers_all`, `cookies`, `content_type`, `is_success`, `is_redirect`, `is_client_error`, `is_server_error`, `text`, `text_lossy`, `json`.

## Dynamic Profile Resolution and `Alt-Svc` Discovery

`Http3Options::for_browser`, `for_version`, and `for_fingerprint` dynamically resolve catalog profiles across Chrome, Edge, Firefox, Brave, Opera, and Safari (capping Chromium majors to the highest supported `phantom-http` profile).

In `HttpSession`, enable `HttpSessionOptions::new().http3_alt_svc(true)` (and optionally `.root_cert_der(path)` for local test servers) to parse `Alt-Svc` response headers (`h3`, `ma=`, `clear`), upgrade matching origins to pooled `Http3Pool` workers, and fall back transparently to HTTP/2 when opportunistic HTTP/3 fails or UDP is blocked.

</Step>

</Steps>

### Wire specs, `HttpSession` routing, and QUIC capture (`network::http3::capture`)

Beyond standalone `sbrs-h3d` calls, HTTP/3 integrates across `network` and `session`:

- **`seleniumbase_rs_stealth::network::spec::H3Spec`**: Pure data representation of QUIC transport parameters (`transport_param_id`), HTTP/3 `SETTINGS` (`setting_id`), and pseudo-header order (`:method`, `:authority`, `:scheme`, `:path`). Provides `H3Spec::current_stable()`, `chrome_157_macos()`, `chrome_156_macos()`, `chrome(major, os)`, `new(...)`, and `new_with_version(...)`.
- **`Http3Options` persona matching**: `Http3Options::for_version(major, os)` and `Http3Options::for_fingerprint(&fp)` pass `--profile chrome-<major>-<os>` to `sbrs-h3d`.
- **`session::HttpSession` routing (`WireProtocol::Http3`)**: Calling `session.request(Method::GET, url).protocol(WireProtocol::Http3).send().await?` sends the request over `sbrs-h3d` with `CookieJar` cookies attached, stores `Set-Cookie` responses back into the session jar, and records `WireProtocol::Http3` for that origin.
- **RFC 9001 / RFC 9369 capture decoding (`network::http3::capture`)**: `seleniumbase_rs_stealth::network::http3::capture` derives client Initial packet keys (`client_initial_keys`, `parse_long_header` for QUIC `VERSION_1` and `VERSION_2`), parses QUIC variable-length integers and HTTP/3 frames (`parse_frames`, `parse_settings`), and decodes static-table / Huffman QPACK field sections (`decode_field_section`) without needing keylog secrets for Initial flights.

What to know:

- **Standalone vs. `HttpSession` fallback.** Calling `Http3Client` or `Http3Pool` directly sends HTTP/3 only (with UDP blocked, a request fails at its timeout: 30 seconds by default). Through [`HttpSession`](/scraping/stealthy/http-session) with `HttpSessionOptions::http3_alt_svc(true)`, opportunistic HTTP/3 failures fall back to HTTP/2 automatically.
- **Errors, not panics.** A helper that cannot be started is
  `Http3Error::HelperMissing`, whose message says how to install it; a helper
  that ended is `Http3Error::HelperEnded`. A request the helper could not send,
  or that got no response, is `Http3Error::Failed` with the helper's kind
  (`request`, `send`, `timeout` or `body`).
- **The profile.** `Http3Options::new()` uses the active Chrome profile, and `Http3Options::for_browser`, `for_version`, or `for_fingerprint` resolves the closest matching catalog profile (capping above the highest `phantom-http` release). Headers follow the chosen `Http3RequestKind` (`Navigation` or `Subresource`) template.
- **UDP proxies only.** `sbrs-h3d` routes `socks5://`, `socks5h://`, `connect-udp://`, and MASQUE `https://` proxies (on `Http3Options` or per-request on `Http3Request`). Classic TCP-only `http://` CONNECT proxies are refused with a clear error.
- **Local test servers.** `Http3Options::root_cert_der` (and `HttpSessionOptions::root_cert_der`) trusts one more
  certificate and `Http3Options::resolve` pins a host to an address, for a
  server on `127.0.0.1`.

## What matches Chrome, and what does not

These are the results of the HTTP/3 spike, recorded in the stealth repository's
[design note](https://github.com/MustCodeAl/seleniumbase-rs-stealth/blob/main/docs/design/http3.md).
It compared phantom's `chrome-156-macos` profile with headless Chrome
156.0.8078.76 on macOS, both against a local server that logged what it
received. Nothing else was compared.

<Tabs>

<Tab title="Measured the same" icon="circle-check">

- the TLS ClientHello inside QUIC: cipher suites, the extension set, groups,
  key-share group ids and key lengths, signature algorithms, ALPN `h3`, ALPS,
  the ECH header fields, and the length and first 32 bytes of extension
  `0xca34`;
- the QUIC transport parameters: the 11 fixed values, and the shape of the
  other two;
- the HTTP/3 SETTINGS: ids, values and order;
- the request header names and the pseudo-header order;
- QUIC version 1.

</Tab>

<Tab title="Measured different" icon="circle-xmark">

A server can see each of these in the first flight:

- the Initial packets: 1200 bytes, not 1250; a 20-byte destination connection
  id, not 8 bytes; packet number 0, not 1; no "chaos protection" layout;
- the control stream: no GREASE frame and no `PRIORITY_UPDATE`;
- the QPACK streams: not seen from phantom when the server's table is 0, and
  different use of the dynamic table on the first request;
- Huffman coding of short header values.

</Tab>

<Tab title="Not measured" icon="circle-question">

Not measured, so not known to match: acknowledgements and their timing,
flow-control updates, loss recovery and probe timers, congestion control,
pacing, path MTU discovery, short-header packet sizes, key updates, connection
close, 0-RTT and session resumption, Retry and tokens, connection migration,
QUIC v2, the spin bit, later requests on a warm connection, long-lived
connections, any real network or site, and Chrome on Windows or Linux.

</Tab>

</Tabs>

<Warning>

In short, this is "Chrome 156 TLS, transport parameters, SETTINGS and headers
over QUIC", not "Chrome's QUIC". The wire crate's own HTTP/2 entry is Chrome
155, and HTTP/3 uses phantom's 156 profile, so a session that uses both
presents two majors until the two entries are measured at one version.

</Warning>

## Version pin

`h3d/` pins `phantom-http = "=0.6.0"` and commits its own `Cargo.lock`.
phantom is pre-1.0 and changes fast. An upgrade is a pull request that runs the
capture again and updates the lists above.

## Tests

`h3d/tests/local_h3.rs` starts a local HTTP/3 server (quinn and h3 on
`127.0.0.1`, a fresh self-signed certificate) and drives `sbrs-h3d` against it:

- a GET over `h3`, with the profile's Chrome `User-Agent`;
- a POST body and a caller header;
- a server that never answers, which gives an error within the time limit;
- a line that is not a request, which gets an answer while the next request
  still works.

The library's tests check the line format against a stand-in script. Run
them from the stealth repository's root:

```bash
cd h3d && cargo test
cargo test --features http3 --lib network::http3
```

The root crate never builds `h3d/`, so its usual gate does not
build phantom.

## Where to go next

<Columns cols={2}>

<Card title="Wire formats" icon="fingerprint" color="#d32a61" href="/stealth/advanced/wire-formats">
  The measured TLS and HTTP/2 formats the HTTP/1.1 and HTTP/2 client presents.
</Card>

<Card title="Limits" icon="triangle-exclamation" color="#d5a910" href="/scraping/advanced-stealthy/limits">
  What the stealth crates do not cover, and how to read their errors.
</Card>

</Columns>
