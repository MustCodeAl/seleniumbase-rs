---
title: "HTTP/3 through a Helper Program"
sidebarTitle: "HTTP/3"
description: "Send a request over HTTP/3 with Chrome's TLS, QUIC transport parameters, SETTINGS and headers, through the sbrs-h3d helper of the stealth repository. Experimental and off by default."
---

<Badge color="orange">Experimental</Badge>

<Visibility for="agents">

Summary for AI exports: HTTP/3 lives in the `seleniumbase-rs-wire` crate behind the facade's `http3` feature (off by default; it turns `wire` on). The QUIC stack is phantom-http `=0.6.0`, whose own BoringSSL cannot be linked next to the one wreq uses, so it runs in a separate program, `sbrs-h3d` (`cargo install --path crates/h3d` in the stealth repository), which `Http3Client` drives over stdin and stdout. Requests go over HTTP/3 only, with no fallback, no proxy and no `Alt-Svc` learning. The TLS ClientHello, QUIC transport parameters, HTTP/3 SETTINGS and header order were measured equal to Chrome 156 on macOS; the Initial packets, control and QPACK streams and Huffman coding of short values were measured different; everything after the first flight was not measured.

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

`sbrs-h3d` is the package in `crates/h3d` of the
[stealth repository](https://github.com/MustCodeAl/seleniumbase-rs-stealth).
From a clone of it:

```bash
cargo install --path crates/h3d            # builds sbrs-h3d (phantom and its BoringSSL)
```

</Step>

<Step title="Turn the feature on" icon="toggle-on">

The facade's `http3` feature turns `wire` on too, so it builds BoringSSL and
needs cmake and a C/C++ toolchain.

```toml Cargo.toml
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth", features = ["http3"] }
```

</Step>

<Step title="Send a request" icon="paper-plane">

```rust
use seleniumbase_rs_stealth::wire::http3::{Http3Client, Http3Options, Http3Request};

async fn demo() -> Result<(), Box<dyn std::error::Error>> {
    // sbrs-h3d on the PATH, or the program named in SBRS_H3D.
    let mut client = Http3Client::start(&Http3Options::new())?;
    let response = client.send(Http3Request::get("https://example.com/")).await?;
    assert_eq!(response.protocol(), "h3");
    println!("{} {} bytes", response.status(), response.body().len());
    Ok(())
}
```

</Step>

</Steps>

What to know:

- **HTTP/3 only.** Every request goes over HTTP/3; nothing falls back to
  HTTP/2. To fall back, send the request again through
  [`HttpSession`](/scraping/stealthy/http-session).
- **Errors, not panics.** A helper that cannot be started is
  `Http3Error::HelperMissing`, whose message says how to install it; a helper
  that ended is `Http3Error::HelperEnded`. A request the helper could not send,
  or that got no response, is `Http3Error::Failed` with the helper's kind
  (`request`, `send`, `timeout` or `body`).
- **The profile.** The helper uses phantom's `chrome-156-macos` profile unless
  `Http3Options::profile` names another. Its headers come from that profile's
  navigation template; a header you add with `Http3Request::header` keeps the
  template's place when the template has it.
- **No `Alt-Svc`.** HTTP/3 is used only when you ask for it. With UDP blocked, a
  request fails at its time limit: 30 seconds unless `Http3Request::timeout`
  says otherwise.
- **No proxy.** An HTTP proxy cannot carry HTTP/3 (a CONNECT tunnel carries
  TCP), and the helper takes no proxy. Requests go straight from this machine,
  so a [proxy](/stealth/proxies) set elsewhere does not apply to them.
- **Local test servers.** `Http3Options::root_cert_der` trusts one more
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

`crates/h3d` pins `phantom-http = "=0.6.0"` and commits its own `Cargo.lock`.
phantom is pre-1.0 and changes fast. An upgrade is a pull request that runs the
spike's capture again and updates the lists above.

## Tests

`crates/h3d/tests/local_h3.rs` starts a local HTTP/3 server (quinn and h3 on
`127.0.0.1`, a fresh self-signed certificate) and drives `sbrs-h3d` against it:

- a GET over `h3`, with the profile's Chrome `User-Agent`;
- a POST body and a caller header;
- a server that never answers, which gives an error within the time limit;
- a line that is not a request, which gets an answer while the next request
  still works.

The wire crate's tests check the line format against a stand-in script. Run
them from the stealth repository's root:

```bash
cd crates/h3d && cargo test
cargo test -p seleniumbase-rs-wire --features http3 --lib http3
```

The stealth workspace never builds `crates/h3d`, so its usual gate does not
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
