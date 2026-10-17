//! HAR recording against a scripted browser, with no Chrome: the test plays the
//! `Network` and `Page` events Chrome would send and checks the archive.
//! Run with `cargo test --features test-util --test sb_cdp_har`.

#![cfg(feature = "test-util")]

use seleniumbase_rs::sb_cdp::{
    Browser, Har, HarEntry, HarNameValue, HarOptions, MockCtrl, Page, MAX_CONTENT_BYTES,
};
use serde_json::{json, Value};

/// The wall-clock time of the first request: 2023-11-14T22:13:20.500Z.
const WALL: f64 = 1_700_000_000.5;

async fn page() -> (Page, MockCtrl) {
    let (browser, mock) = Browser::new_mocked();
    (browser.default_page().await.unwrap(), mock)
}

/// Sends an event from the default tab, as Chrome would.
fn send(mock: &MockCtrl, method: &str, params: Value) {
    mock.emit(method, params, Some("S1"));
}

/// A `requestWillBeSent` for a sub-resource of the page.
fn request(mock: &MockCtrl, id: &str, url: &str, kind: &str, at: f64) {
    request_in(mock, "S1", id, url, kind, at);
}

/// A `requestWillBeSent` for a sub-resource, from the protocol session
/// `session`.
fn request_in(mock: &MockCtrl, session: &str, id: &str, url: &str, kind: &str, at: f64) {
    mock.emit(
        "Network.requestWillBeSent",
        json!({
            "requestId": id, "loaderId": "L1", "frameId": "T1", "type": kind,
            "timestamp": at, "wallTime": WALL + at,
            "request": { "url": url, "method": "GET", "headers": { "Accept": "*/*" } },
        }),
        Some(session),
    );
}

/// A plain 200 response with no connection timing, then the end of loading.
fn ok(mock: &MockCtrl, id: &str, mime: &str, at: f64, length: u64) {
    ok_in(mock, "S1", id, mime, at, length);
}

/// [`ok`], from the protocol session `session`.
fn ok_in(mock: &MockCtrl, session: &str, id: &str, mime: &str, at: f64, length: u64) {
    let send = |method: &str, params: Value| mock.emit(method, params, Some(session));
    send(
        "Network.responseReceived",
        json!({
            "requestId": id, "timestamp": at,
            "response": {
                "url": "x", "status": 200, "statusText": "OK",
                "headers": { "Content-Type": mime }, "mimeType": mime, "protocol": "http/1.1",
            },
        }),
    );
    send(
        "Network.dataReceived",
        json!({ "requestId": id, "timestamp": at, "dataLength": length, "encodedDataLength": 0 }),
    );
    send(
        "Network.loadingFinished",
        json!({ "requestId": id, "timestamp": at + 0.01, "encodedDataLength": length + 100 }),
    );
}

fn value<'a>(pairs: &'a [HarNameValue], name: &str) -> Vec<&'a str> {
    pairs
        .iter()
        .filter(|pair| pair.name.eq_ignore_ascii_case(name))
        .map(|pair| pair.value.as_str())
        .collect()
}

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() < 1e-6
}

fn entry<'a>(har: &'a Har, url: &str) -> &'a HarEntry {
    har.log
        .entries
        .iter()
        .find(|entry| entry.request.url == url)
        .unwrap_or_else(|| panic!("no entry for {url}"))
}

#[tokio::test]
async fn starting_enables_the_network_domain() {
    let (page, mock) = page().await;

    let _recorder = page.start_har().await.unwrap();

    assert_eq!(mock.calls_to("Network.enable").len(), 1);
}

#[tokio::test]
async fn a_navigation_becomes_a_page_and_an_entry_with_timings() {
    let (page, mock) = page().await;
    let recorder = page.start_har().await.unwrap();
    let url = "https://shop.test/list?q=red+shoes&page=2";

    send(
        &mock,
        "Network.requestWillBeSent",
        json!({
            "requestId": "D1", "loaderId": "D1", "frameId": "T1", "type": "Document",
            "timestamp": 100.0, "wallTime": WALL, "documentURL": url,
            "request": { "url": url, "method": "GET", "headers": { "Accept": "text/html" } },
        }),
    );
    send(
        &mock,
        "Network.requestWillBeSentExtraInfo",
        json!({ "requestId": "D1", "headers": { "Accept": "text/html", "Cookie": "sid=abc; theme=dark" } }),
    );
    send(
        &mock,
        "Network.responseReceived",
        json!({
            "requestId": "D1", "loaderId": "D1", "timestamp": 100.25, "type": "Document",
            "response": {
                "url": url, "status": 200, "statusText": "OK",
                "headers": { "Content-Type": "text/html" }, "mimeType": "text/html",
                "protocol": "h2", "remoteIPAddress": "203.0.113.7", "connectionId": 42,
                "timing": {
                    "requestTime": 100.01, "proxyStart": -1, "proxyEnd": -1,
                    "dnsStart": 1.0, "dnsEnd": 5.0, "connectStart": 5.0, "connectEnd": 20.0,
                    "sslStart": 10.0, "sslEnd": 20.0, "sendStart": 21.0, "sendEnd": 22.0,
                    "receiveHeadersEnd": 200.0,
                },
            },
        }),
    );
    send(
        &mock,
        "Network.responseReceivedExtraInfo",
        json!({
            "requestId": "D1", "statusCode": 200,
            "headers": {
                "content-type": "text/html",
                "set-cookie": "sid=abc; Path=/; HttpOnly\ntheme=dark; Secure",
            },
        }),
    );
    for length in [1000, 234] {
        send(
            &mock,
            "Network.dataReceived",
            json!({ "requestId": "D1", "timestamp": 100.28, "dataLength": length, "encodedDataLength": 0 }),
        );
    }
    send(
        &mock,
        "Page.domContentEventFired",
        json!({ "timestamp": 100.4 }),
    );
    send(
        &mock,
        "Network.loadingFinished",
        json!({ "requestId": "D1", "timestamp": 100.3, "encodedDataLength": 812 }),
    );
    send(&mock, "Page.loadEventFired", json!({ "timestamp": 100.5 }));

    let har = recorder.finish().await.unwrap();

    assert_eq!(har.log.version, "1.2");
    assert_eq!(har.log.creator.name, "seleniumbase-rs");
    assert_eq!(har.log.pages.len(), 1);
    let first_page = &har.log.pages[0];
    assert_eq!(first_page.id, "page_1");
    assert_eq!(first_page.title, url);
    assert_eq!(first_page.started_date_time, "2023-11-14T22:13:20.500Z");
    assert!(close(first_page.page_timings.on_content_load, 400.0));
    assert!(close(first_page.page_timings.on_load, 500.0));

    assert_eq!(har.log.entries.len(), 1);
    let entry = &har.log.entries[0];
    assert_eq!(entry.pageref.as_deref(), Some("page_1"));
    assert_eq!(entry.started_date_time, "2023-11-14T22:13:20.500Z");
    assert_eq!(entry.resource_type.as_deref(), Some("document"));
    assert_eq!(entry.server_ip_address.as_deref(), Some("203.0.113.7"));
    assert_eq!(entry.connection.as_deref(), Some("42"));

    let request = &entry.request;
    assert_eq!(request.method, "GET");
    assert_eq!(request.url, url);
    assert_eq!(request.http_version, "HTTP/2");
    assert_eq!(
        value(&request.headers, "cookie"),
        ["sid=abc; theme=dark"],
        "the headers as sent, from the extra info"
    );
    let cookies: Vec<_> = request
        .cookies
        .iter()
        .map(|c| (c.name.as_str(), c.value.as_str()))
        .collect();
    assert_eq!(cookies, [("sid", "abc"), ("theme", "dark")]);
    let query: Vec<_> = request
        .query_string
        .iter()
        .map(|q| (q.name.as_str(), q.value.as_str()))
        .collect();
    assert_eq!(query, [("q", "red shoes"), ("page", "2")]);
    assert!(request.post_data.is_none());
    assert_eq!(request.body_size, 0);

    let response = &entry.response;
    assert_eq!(response.status, 200);
    assert_eq!(response.status_text, "OK");
    assert_eq!(response.http_version, "HTTP/2");
    assert_eq!(value(&response.headers, "set-cookie").len(), 2);
    assert_eq!(response.cookies.len(), 2);
    assert_eq!(response.cookies[0].name, "sid");
    assert_eq!(response.cookies[0].path.as_deref(), Some("/"));
    assert_eq!(response.cookies[0].http_only, Some(true));
    assert_eq!(response.cookies[1].secure, Some(true));
    assert_eq!(response.content.size, 1234, "the bytes of dataReceived");
    assert_eq!(response.content.mime_type, "text/html");
    assert!(response.content.text.is_none(), "no bodies by default");
    assert_eq!(response.redirect_url, "");
    assert_eq!(response.transfer_size, Some(812));
    assert!(response.error.is_none());

    // requestTime is 10 ms after the request was issued, and the phases are
    // offsets from it.
    let timings = &entry.timings;
    assert!(close(timings.blocked, 11.0), "{timings:?}");
    assert!(close(timings.dns, 4.0), "{timings:?}");
    assert!(close(timings.connect, 15.0), "{timings:?}");
    assert!(close(timings.ssl, 10.0), "{timings:?}");
    assert!(close(timings.send, 1.0), "{timings:?}");
    assert!(close(timings.wait, 178.0), "{timings:?}");
    assert!(close(timings.receive, 90.0), "{timings:?}");
    assert!(
        close(entry.time, 299.0),
        "ssl is not counted twice: {}",
        entry.time
    );
}

#[tokio::test]
async fn a_redirect_becomes_one_entry_per_hop_on_one_page() {
    let (page, mock) = page().await;
    let recorder = page.start_har().await.unwrap();
    let document = |url: &str, at: f64, redirect: Option<Value>| {
        let mut params = json!({
            "requestId": "R1", "loaderId": "R1", "frameId": "T1", "type": "Document",
            "timestamp": at, "wallTime": WALL + at,
            "request": { "url": url, "method": "GET", "headers": {} },
        });
        if let Some(redirect) = redirect {
            params["redirectResponse"] = redirect;
        }
        params
    };

    send(
        &mock,
        "Network.requestWillBeSent",
        document("http://a.test/old", 1.0, None),
    );
    send(
        &mock,
        "Network.requestWillBeSent",
        document(
            "https://a.test/new",
            1.1,
            Some(json!({
                "url": "http://a.test/old", "status": 301, "statusText": "Moved Permanently",
                "headers": { "Location": "https://a.test/new" }, "mimeType": "",
                "protocol": "http/1.1",
            })),
        ),
    );
    ok(&mock, "R1", "text/html", 1.2, 10);

    let har = recorder.finish().await.unwrap();

    assert_eq!(har.log.pages.len(), 1, "a redirect is not a new page");
    assert_eq!(har.log.entries.len(), 2);
    let old = entry(&har, "http://a.test/old");
    assert_eq!(old.response.status, 301);
    assert_eq!(old.response.redirect_url, "https://a.test/new");
    assert_eq!(old.response.http_version, "HTTP/1.1");
    assert!(close(old.timings.wait, 100.0), "{:?}", old.timings);
    let new = entry(&har, "https://a.test/new");
    assert_eq!(new.response.status, 200);
    assert_eq!(new.response.redirect_url, "");
    assert_eq!(new.pageref.as_deref(), Some("page_1"));
}

/// The 302's headers as they went over the wire.
fn redirect_extra() -> Value {
    json!({
        "requestId": "R2", "statusCode": 302,
        "headers": { "location": "/", "set-cookie": "a=1" },
    })
}

/// The 200's headers as they went over the wire.
fn document_extra() -> Value {
    json!({
        "requestId": "R2", "statusCode": 200,
        "headers": { "content-type": "text/html" },
    })
}

/// Plays `GET /old` answered by a 302 to `/`, then `/` answered by a 200, with
/// the two `responseReceivedExtraInfo` events at the points `extra_at` names:
/// 0 before the redirect's `requestWillBeSent`, 1 after it, 2 after the 200's
/// `responseReceived`.
async fn redirect_with_extra_info(extra_at: [usize; 2]) -> Har {
    let (page, mock) = page().await;
    let recorder = page.start_har().await.unwrap();
    let extras = [redirect_extra(), document_extra()];
    let play_extras_at = |point: usize| {
        for (extra, at) in extras.iter().zip(extra_at) {
            if at == point {
                send(&mock, "Network.responseReceivedExtraInfo", extra.clone());
            }
        }
    };
    let document = |url: &str, at: f64| {
        json!({
            "requestId": "R2", "loaderId": "R2", "frameId": "T1", "type": "Document",
            "timestamp": at, "wallTime": WALL + at,
            "request": { "url": url, "method": "GET", "headers": {} },
        })
    };

    send(
        &mock,
        "Network.requestWillBeSent",
        document("https://a.test/old", 1.0),
    );
    play_extras_at(0);
    let mut hop = document("https://a.test/", 1.1);
    hop["redirectHasExtraInfo"] = json!(true);
    hop["redirectResponse"] = json!({
        "url": "https://a.test/old", "status": 302, "statusText": "Found",
        "headers": { "Location": "/" }, "mimeType": "", "protocol": "http/1.1",
    });
    send(&mock, "Network.requestWillBeSent", hop);
    play_extras_at(1);
    send(
        &mock,
        "Network.responseReceived",
        json!({
            "requestId": "R2", "timestamp": 1.2, "type": "Document", "hasExtraInfo": true,
            "response": {
                "url": "https://a.test/", "status": 200, "statusText": "OK",
                "headers": { "Content-Type": "text/html" }, "mimeType": "text/html",
                "protocol": "http/1.1",
            },
        }),
    );
    play_extras_at(2);
    send(
        &mock,
        "Network.loadingFinished",
        json!({ "requestId": "R2", "timestamp": 1.3, "encodedDataLength": 100 }),
    );
    recorder.finish().await.unwrap()
}

#[tokio::test]
async fn each_hop_of_a_redirect_gets_its_own_wire_headers_whenever_they_arrive() {
    // Chrome does not order the ExtraInfo events against requestWillBeSent:
    // the 302's may come before or after the next hop starts.
    for extra_at in [[0, 1], [1, 1], [0, 2], [1, 2], [2, 2]] {
        let har = redirect_with_extra_info(extra_at).await;

        let old = entry(&har, "https://a.test/old");
        assert_eq!(old.response.status, 302, "{extra_at:?}");
        assert_eq!(
            value(&old.response.headers, "set-cookie"),
            ["a=1"],
            "{extra_at:?}"
        );
        assert_eq!(old.response.cookies.len(), 1, "{extra_at:?}");
        assert_eq!(old.response.redirect_url, "/", "{extra_at:?}");
        let new = entry(&har, "https://a.test/");
        assert_eq!(new.response.status, 200, "{extra_at:?}");
        assert_eq!(
            value(&new.response.headers, "content-type"),
            ["text/html"],
            "{extra_at:?}: the 200's own headers"
        );
        assert!(
            value(&new.response.headers, "location").is_empty(),
            "{extra_at:?}: not the 302's"
        );
        assert!(new.response.cookies.is_empty(), "{extra_at:?}");
    }
}

#[tokio::test]
async fn a_failed_request_has_status_zero_and_the_error() {
    let (page, mock) = page().await;
    let recorder = page.start_har().await.unwrap();

    request(&mock, "F1", "https://nowhere.invalid/app.js", "Script", 5.0);
    send(
        &mock,
        "Network.loadingFailed",
        json!({
            "requestId": "F1", "timestamp": 5.02, "type": "Script",
            "errorText": "net::ERR_NAME_NOT_RESOLVED", "canceled": false,
        }),
    );
    request(&mock, "F2", "https://a.test/blocked.png", "Image", 5.0);
    send(
        &mock,
        "Network.loadingFailed",
        json!({
            "requestId": "F2", "timestamp": 5.01, "errorText": "net::ERR_BLOCKED_BY_CLIENT",
            "canceled": false, "blockedReason": "inspector",
        }),
    );

    let har = recorder.finish().await.unwrap();

    let failed = entry(&har, "https://nowhere.invalid/app.js");
    assert_eq!(failed.response.status, 0);
    assert_eq!(
        failed.response.error.as_deref(),
        Some("net::ERR_NAME_NOT_RESOLVED")
    );
    assert_eq!(failed.response.content.mime_type, "x-unknown");
    assert_eq!(failed.resource_type.as_deref(), Some("script"));
    assert!(failed.pageref.is_none(), "no page was recorded before it");
    assert!(close(failed.timings.wait, 20.0), "{:?}", failed.timings);
    let blocked = entry(&har, "https://a.test/blocked.png");
    assert_eq!(
        blocked.response.error.as_deref(),
        Some("net::ERR_BLOCKED_BY_CLIENT (blocked: inspector)")
    );
}

#[tokio::test]
async fn a_request_without_an_answer_yet_is_kept_with_a_comment() {
    let (page, mock) = page().await;
    let recorder = page.start_har().await.unwrap();

    request(&mock, "P1", "https://a.test/slow", "Fetch", 1.0);

    let har = recorder.finish().await.unwrap();

    let pending = entry(&har, "https://a.test/slow");
    assert_eq!(pending.response.status, 0);
    assert!(pending.response.error.is_none());
    assert_eq!(
        pending.response.comment.as_deref(),
        Some("no response before the recording finished")
    );
    // No phase is known, so the total is 0, and not the -0.0 an empty float
    // sum gives, which the JSON would show as `-0.0`.
    assert!(
        pending.time == 0.0 && pending.time.is_sign_positive(),
        "{}",
        pending.time
    );
    assert!(har.to_json().unwrap().contains("\"time\": 0.0"));
}

#[tokio::test]
async fn headers_sent_before_their_request_are_still_used() {
    let (page, mock) = page().await;
    let recorder = page.start_har().await.unwrap();

    send(
        &mock,
        "Network.requestWillBeSentExtraInfo",
        json!({ "requestId": "E1", "headers": { "Accept": "*/*", "Cookie": "early=1" } }),
    );
    request(&mock, "E1", "https://a.test/early", "XHR", 1.0);
    send(
        &mock,
        "Network.responseReceivedExtraInfo",
        json!({ "requestId": "E1", "statusCode": 200, "headers": { "set-cookie": "late=2" } }),
    );
    ok(&mock, "E1", "application/json", 1.1, 2);

    let har = recorder.finish().await.unwrap();

    let early = entry(&har, "https://a.test/early");
    assert_eq!(value(&early.request.headers, "cookie"), ["early=1"]);
    assert_eq!(early.response.cookies[0].name, "late");
}

#[tokio::test]
async fn post_data_is_recorded_from_text_or_from_entries() {
    let (page, mock) = page().await;
    let recorder = page.start_har().await.unwrap();

    send(
        &mock,
        "Network.requestWillBeSent",
        json!({
            "requestId": "B1", "loaderId": "L1", "frameId": "T1", "type": "XHR",
            "timestamp": 1.0, "wallTime": WALL,
            "request": {
                "url": "https://a.test/form", "method": "POST", "postData": "a=1&b=2",
                "headers": { "Content-Type": "application/x-www-form-urlencoded" },
            },
        }),
    );
    send(
        &mock,
        "Network.requestWillBeSent",
        json!({
            "requestId": "B2", "loaderId": "L1", "frameId": "T1", "type": "Fetch",
            "timestamp": 1.0, "wallTime": WALL,
            "request": {
                "url": "https://a.test/api", "method": "PUT", "hasPostData": true,
                // `{"x":1}` in base64, split in two the way Chrome may.
                "postDataEntries": [{ "bytes": "eyJ4Ijo=" }, { "bytes": "MX0=" }],
                "headers": { "content-type": "application/json" },
            },
        }),
    );

    let har = recorder.finish().await.unwrap();

    let form = entry(&har, "https://a.test/form");
    let post = form.request.post_data.as_ref().unwrap();
    assert_eq!(post.mime_type, "application/x-www-form-urlencoded");
    assert_eq!(post.text, "a=1&b=2");
    assert_eq!(form.request.body_size, 7);
    let api = entry(&har, "https://a.test/api");
    let post = api.request.post_data.as_ref().unwrap();
    assert_eq!(post.text, r#"{"x":1}"#);
    assert_eq!(post.mime_type, "application/json");
}

#[tokio::test]
async fn a_body_the_browser_left_out_has_an_unknown_size() {
    let (page, mock) = page().await;
    let recorder = page.start_har().await.unwrap();
    let post = |id: &str, url: &str, body: Value| {
        let mut request =
            json!({ "url": url, "method": "POST", "hasPostData": true, "headers": {} });
        if let (Some(request), Some(body)) = (request.as_object_mut(), body.as_object()) {
            request.extend(body.clone());
        }
        send(
            &mock,
            "Network.requestWillBeSent",
            json!({
                "requestId": id, "loaderId": "L1", "frameId": "T1", "type": "Fetch",
                "timestamp": 1.0, "wallTime": WALL, "request": request,
            }),
        );
    };

    // Too long: Chrome says there is a body and leaves it out.
    post("B3", "https://a.test/upload", json!({}));
    // A file part comes without its bytes.
    post(
        "B4",
        "https://a.test/files",
        json!({ "postDataEntries": [{ "bytes": "YT0x" }, {}] }),
    );
    // Two bytes that are not UTF-8.
    post(
        "B5",
        "https://a.test/binary",
        json!({ "postDataEntries": [{ "bytes": "//4=" }] }),
    );
    let har = recorder.finish().await.unwrap();

    let upload = &entry(&har, "https://a.test/upload").request;
    assert_eq!(upload.body_size, -1, "unknown, not empty");
    assert!(upload.post_data.is_none());
    assert!(upload
        .comment
        .as_deref()
        .unwrap_or_default()
        .contains("not reported"));
    let files = &entry(&har, "https://a.test/files").request;
    assert_eq!(files.body_size, -1);
    let post_data = files.post_data.as_ref().unwrap();
    assert_eq!(post_data.text, "a=1");
    assert!(post_data
        .comment
        .as_deref()
        .unwrap_or_default()
        .contains("not reported"));
    let binary = &entry(&har, "https://a.test/binary").request;
    assert_eq!(binary.body_size, 2, "the bytes, not the text they make");
}

#[tokio::test]
async fn bodies_are_not_fetched_by_default() {
    let (page, mock) = page().await;
    let recorder = page.start_har().await.unwrap();

    request(&mock, "N1", "https://a.test/a.css", "Stylesheet", 1.0);
    ok(&mock, "N1", "text/css", 1.1, 20);
    let har = recorder.finish().await.unwrap();

    assert!(mock.calls_to("Network.getResponseBody").is_empty());
    assert!(har.log.entries[0].response.content.text.is_none());
}

#[tokio::test]
async fn bodies_are_kept_when_asked_up_to_the_size_limit() {
    let (page, mock) = page().await;
    mock.on("Network.getResponseBody", |params| {
        Ok(match params["requestId"].as_str() {
            Some("small") => json!({ "body": "hello", "base64Encoded": false }),
            // The bytes 0, 1 and 2.
            Some("binary") => json!({ "body": "AAEC", "base64Encoded": true }),
            Some("big") => json!({ "body": "x".repeat(100), "base64Encoded": false }),
            _ => return Err("No resource with given identifier found".to_owned()),
        })
    });
    let recorder = page
        .start_har_with(HarOptions::new().content(true).max_content_bytes(16))
        .await
        .unwrap();

    for (id, kind) in [
        ("small", "Fetch"),
        ("binary", "Image"),
        ("big", "Script"),
        ("huge", "Media"),
        ("gone", "Fetch"),
    ] {
        request(&mock, id, &format!("https://a.test/{id}"), kind, 1.0);
    }
    ok(&mock, "small", "text/plain", 1.1, 5);
    ok(&mock, "binary", "image/png", 1.1, 3);
    // The body's size is only known once it is fetched.
    ok(&mock, "big", "text/javascript", 1.1, 0);
    // Known to be too large from the bytes received: never fetched.
    ok(&mock, "huge", "video/mp4", 1.1, 1000);
    ok(&mock, "gone", "text/plain", 1.1, 4);
    let har = recorder.finish().await.unwrap();

    let small = &entry(&har, "https://a.test/small").response.content;
    assert_eq!(small.text.as_deref(), Some("hello"));
    assert_eq!(small.size, 5);
    assert!(small.encoding.is_none());
    let binary = &entry(&har, "https://a.test/binary").response.content;
    assert_eq!(binary.text.as_deref(), Some("AAEC"));
    assert_eq!(binary.encoding.as_deref(), Some("base64"));
    assert_eq!(binary.size, 3);
    let big = &entry(&har, "https://a.test/big").response.content;
    assert!(big.text.is_none());
    assert_eq!(
        big.comment.as_deref(),
        Some("content not recorded: 100 bytes is over the 16-byte limit")
    );
    let huge = &entry(&har, "https://a.test/huge").response.content;
    assert!(huge.text.is_none());
    assert!(huge.comment.as_deref().unwrap().contains("1000 bytes"));
    let gone = &entry(&har, "https://a.test/gone").response.content;
    assert!(gone.text.is_none());
    assert!(gone
        .comment
        .as_deref()
        .unwrap()
        .contains("No resource with given identifier found"));

    let fetched: Vec<_> = mock
        .calls_to("Network.getResponseBody")
        .into_iter()
        .map(|call| call.params["requestId"].as_str().unwrap().to_owned())
        .collect();
    assert!(!fetched.contains(&"huge".to_owned()), "{fetched:?}");
    assert_eq!(fetched.len(), 4, "{fetched:?}");
}

#[tokio::test]
async fn the_body_limit_stops_where_one_devtools_message_would_overflow() {
    let (page, mock) = page().await;
    mock.on("Network.getResponseBody", |_| {
        Ok(json!({ "body": "", "base64Encoded": false }))
    });
    let options = HarOptions::new()
        .content(true)
        .max_content_bytes(usize::MAX);
    assert_eq!(
        options,
        HarOptions::new()
            .content(true)
            .max_content_bytes(MAX_CONTENT_BYTES)
    );
    let recorder = page.start_har_with(options).await.unwrap();

    request(&mock, "V1", "https://a.test/film.mp4", "Media", 1.0);
    let over = u64::try_from(MAX_CONTENT_BYTES).unwrap() + 1;
    ok(&mock, "V1", "video/mp4", 1.1, over);
    let har = recorder.finish().await.unwrap();

    assert!(
        mock.calls_to("Network.getResponseBody").is_empty(),
        "a body that large is not fetched"
    );
    let content = &entry(&har, "https://a.test/film.mp4").response.content;
    let comment = content.comment.as_deref().unwrap_or_default();
    assert!(
        comment.contains(&format!("over the {MAX_CONTENT_BYTES}-byte limit")),
        "{comment}"
    );
}

#[tokio::test]
async fn events_of_other_tabs_are_not_recorded() {
    let (page, mock) = page().await;
    let recorder = page.start_har().await.unwrap();

    mock.emit(
        "Network.requestWillBeSent",
        json!({
            "requestId": "O1", "loaderId": "O1", "frameId": "T9", "type": "Document",
            "timestamp": 1.0, "wallTime": WALL,
            "request": { "url": "https://other.test/", "method": "GET", "headers": {} },
        }),
        Some("S9"),
    );
    let har = recorder.finish().await.unwrap();

    assert!(har.log.entries.is_empty());
    assert!(har.log.pages.is_empty());
}

/// A `Target.attachedToTarget` for a new target of type `kind`, paused until
/// told to run, from the session it was attached through.
fn attached(mock: &MockCtrl, parent: &str, session: &str, kind: &str) {
    mock.emit(
        "Target.attachedToTarget",
        json!({
            "sessionId": session, "waitingForDebugger": true,
            "targetInfo": {
                "targetId": format!("target-{session}"), "type": kind, "title": "",
                "url": "about:blank", "attached": true, "canAccessOpener": false,
            },
        }),
        Some(parent),
    );
}

/// The sessions `method` was sent to with parameters that are `wanted`,
/// sorted, since each iframe is set up apart.
fn sessions_of(mock: &MockCtrl, method: &str, wanted: fn(&Value) -> bool) -> Vec<String> {
    let mut sessions: Vec<_> = mock
        .calls_to(method)
        .into_iter()
        .filter(|call| wanted(&call.params))
        .map(|call| call.session_id.unwrap_or_default())
        .collect();
    sessions.sort();
    sessions
}

#[tokio::test]
async fn requests_of_cross_site_iframes_are_recorded_too() {
    let (page, mock) = page().await;
    mock.on("Network.getResponseBody", |_| {
        Ok(json!({ "body": "ad()", "base64Encoded": false }))
    });
    let recorder = page
        .start_har_with(HarOptions::new().content(true))
        .await
        .unwrap();

    // Chrome runs a cross-site iframe in a target of its own and reports its
    // requests on that target's session, which the tab's session attaches to;
    // an iframe inside it is attached through its session in turn.
    attached(&mock, "S1", "S2", "iframe");
    request_in(&mock, "S2", "A1", "https://ads.test/ad.js", "Script", 1.0);
    ok_in(&mock, "S2", "A1", "text/javascript", 1.1, 4);
    attached(&mock, "S2", "S3", "iframe");
    request_in(&mock, "S3", "P1", "https://pixel.test/p.gif", "Image", 1.2);
    ok_in(&mock, "S3", "P1", "image/gif", 1.3, 0);
    // An iframe's load is not the page's.
    mock.emit(
        "Page.loadEventFired",
        json!({ "timestamp": 1.5 }),
        Some("S2"),
    );
    let har = recorder.finish().await.unwrap();

    let ad = entry(&har, "https://ads.test/ad.js");
    assert_eq!(ad.response.status, 200);
    assert_eq!(ad.response.content.text.as_deref(), Some("ad()"));
    assert_eq!(entry(&har, "https://pixel.test/p.gif").response.status, 200);
    assert!(har.log.pages.is_empty(), "{:?}", har.log.pages);

    assert_eq!(
        sessions_of(&mock, "Network.enable", |_| true),
        ["S1", "S2", "S3"]
    );
    // Each iframe waits until its requests are reported, then runs.
    let attaching = |params: &Value| {
        params["autoAttach"] == true
            && params["flatten"] == true
            && params["waitForDebuggerOnStart"] == true
    };
    assert_eq!(
        sessions_of(&mock, "Target.setAutoAttach", attaching),
        ["S1", "S2", "S3"]
    );
    assert_eq!(
        sessions_of(&mock, "Runtime.runIfWaitingForDebugger", |_| true),
        ["S2", "S3"]
    );
    let body = mock.calls_to("Network.getResponseBody");
    assert_eq!(body.len(), 2, "{body:?}");
    let fetched_from = |id: &str| {
        body.iter()
            .find(|call| call.params["requestId"] == id)
            .and_then(|call| call.session_id.clone())
    };
    assert_eq!(fetched_from("A1").as_deref(), Some("S2"));
    assert_eq!(fetched_from("P1").as_deref(), Some("S3"));
    // Finishing stops attaching to the tab's iframes, which detaches them.
    let last = mock.calls_to("Target.setAutoAttach").pop().unwrap();
    assert_eq!(last.session_id.as_deref(), Some("S1"));
    assert_eq!(last.params["autoAttach"], false);
}

#[tokio::test]
async fn dropping_a_recorder_stops_attaching_to_iframes() {
    let (page, mock) = page().await;
    let recorder = page.start_har().await.unwrap();

    drop(recorder);
    // The clean-up runs on the runtime, after the drop.
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }

    let attach = mock.calls_to("Target.setAutoAttach");
    let last = attach.last().expect("attaching was turned on");
    assert_eq!(attach.len(), 2, "{attach:?}");
    assert_eq!(last.session_id.as_deref(), Some("S1"));
    assert_eq!(last.params["autoAttach"], false);
}

#[tokio::test]
async fn a_paused_target_that_is_not_an_iframe_is_let_go_unrecorded() {
    let (page, mock) = page().await;
    let recorder = page.start_har().await.unwrap();

    // A browser that does not know the filter attaches a worker too.
    attached(&mock, "S1", "S4", "worker");
    request_in(
        &mock,
        "S4",
        "W1",
        "https://a.test/worker-data",
        "Fetch",
        1.0,
    );
    ok_in(&mock, "S4", "W1", "application/json", 1.1, 2);
    let har = recorder.finish().await.unwrap();

    assert!(har.log.entries.is_empty(), "{:?}", har.log.entries);
    assert_eq!(
        sessions_of(&mock, "Runtime.runIfWaitingForDebugger", |_| true),
        ["S4"]
    );
    assert_eq!(sessions_of(&mock, "Network.enable", |_| true), ["S1"]);
}

#[tokio::test]
async fn a_recording_that_falls_behind_says_so_and_attaches_its_iframes_again() {
    let (page, mock) = page().await;
    let recorder = page.start_har().await.unwrap();

    // More events than the channel holds arrive before the recorder reads
    // any; the oldest are lost, and an iframe's attachment could be among
    // them, leaving that iframe paused.
    for _ in 0..300 {
        send(
            &mock,
            "Network.dataReceived",
            json!({ "requestId": "X", "timestamp": 1.0, "dataLength": 1, "encodedDataLength": 0 }),
        );
    }
    let har = recorder.finish().await.unwrap();

    let comment = har.log.comment.unwrap_or_default();
    assert!(comment.contains("missed 44 "), "{comment}");
    // Attaching again lets every paused iframe go and reports each anew.
    let tab_attach: Vec<_> = mock
        .calls_to("Target.setAutoAttach")
        .into_iter()
        .filter(|call| call.session_id.as_deref() == Some("S1"))
        .map(|call| call.params["autoAttach"] == true)
        .collect();
    assert_eq!(tab_attach, [true, false, true, false]);
}

#[tokio::test]
async fn a_response_from_the_cache_has_only_wait_and_receive() {
    let (page, mock) = page().await;
    let recorder = page.start_har().await.unwrap();

    request(&mock, "C1", "https://a.test/logo.png", "Image", 2.0);
    ok(&mock, "C1", "image/png", 2.05, 10);
    let har = recorder.finish().await.unwrap();

    let timings = &har.log.entries[0].timings;
    assert!(close(timings.blocked, -1.0));
    assert!(close(timings.dns, -1.0));
    assert!(close(timings.connect, -1.0));
    assert!(close(timings.ssl, -1.0));
    assert!(close(timings.send, 0.0));
    assert!(close(timings.wait, 50.0), "{timings:?}");
    assert!(close(timings.receive, 10.0), "{timings:?}");
    assert!(close(har.log.entries[0].time, 60.0));
}

#[tokio::test]
async fn the_browser_version_names_the_browser() {
    let (page, mock) = page().await;
    mock.reply(
        "Browser.getVersion",
        json!({ "product": "HeadlessChrome/141.0.7390.54", "protocolVersion": "1.3" }),
    );

    let har = page.start_har().await.unwrap().finish().await.unwrap();

    let browser = har.log.browser.unwrap();
    assert_eq!(browser.name, "HeadlessChrome");
    assert_eq!(browser.version, "141.0.7390.54");
}

#[tokio::test]
async fn the_har_is_saved_as_json_that_reads_back() {
    let (page, mock) = page().await;
    let recorder = page.start_har().await.unwrap();
    request(&mock, "S1", "https://a.test/data.json?id=7", "Fetch", 1.0);
    ok(&mock, "S1", "application/json", 1.1, 12);
    let har = recorder.finish().await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("run.har");

    har.save(&path).unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    let json: Value = serde_json::from_str(&text).expect("valid JSON");
    assert_eq!(json["log"]["version"], "1.2");
    assert_eq!(json["log"]["creator"]["name"], "seleniumbase-rs");
    let first = &json["log"]["entries"][0];
    assert_eq!(first["request"]["url"], "https://a.test/data.json?id=7");
    assert_eq!(first["request"]["queryString"][0]["name"], "id");
    assert_eq!(first["response"]["redirectURL"], "");
    assert_eq!(first["response"]["_transferSize"], 112);
    assert_eq!(first["cache"], json!({}));
    for field in [
        "blocked", "dns", "connect", "send", "wait", "receive", "ssl",
    ] {
        assert!(first["timings"][field].is_number(), "{field}");
    }
    let read_back: Har = serde_json::from_str(&text).unwrap();
    assert_eq!(read_back, har);
}
