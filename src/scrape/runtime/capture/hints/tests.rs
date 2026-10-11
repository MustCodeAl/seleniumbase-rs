//! Tests of the hints parsers, the cross-check and the hints server.

use tokio::io::AsyncReadExt;

use super::*;
use crate::scrape::runtime::capture::stream::MAX_HTTP_BODY;

/// A user agent in the format Chrome uses on macOS, as parser input.
const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
                  (KHTML, like Gecko) Chrome/100.0.0.0 Safari/537.36";

fn brand(brand: &str, version: &str) -> Brand {
    Brand {
        brand: brand.to_owned(),
        version: version.to_owned(),
    }
}

fn report() -> HintsReport {
    HintsReport {
        user_agent: UA.to_owned(),
        languages: vec!["en-US".to_owned(), "en".to_owned()],
        language: "en-US".to_owned(),
        platform: "MacIntel".to_owned(),
        secure_context: true,
        ua_data: Some(UaData {
            brands: vec![
                brand("Chromium", "100"),
                brand("Not;A=Brand", "99"),
                brand("Google Chrome", "100"),
            ],
            mobile: false,
            platform: "macOS".to_owned(),
            high: HighEntropy {
                architecture: "arm".to_owned(),
                bitness: "64".to_owned(),
                form_factors: vec!["Desktop".to_owned()],
                full_version_list: vec![
                    brand("Chromium", "100.0.1.2"),
                    brand("Not;A=Brand", "99.0.0.0"),
                    brand("Google Chrome", "100.0.1.2"),
                ],
                model: String::new(),
                platform_version: "15.0.0".to_owned(),
                ua_full_version: Some("100.0.1.2".to_owned()),
                wow64: false,
            },
        }),
    }
}

fn header_list(brands: &[BrandVersion]) -> String {
    brands
        .iter()
        .map(|item| format!("\"{}\";v=\"{}\"", item.brand, item.version))
        .collect::<Vec<_>>()
        .join(", ")
}

fn pair(name: &str, value: &str) -> (String, String) {
    (name.to_owned(), value.to_owned())
}

/// The low-entropy headers a browser sends on a first visit.
fn first_visit(hints: &ClientHints) -> Vec<(String, String)> {
    vec![
        pair("host", "127.0.0.1:1"),
        pair("user-agent", UA),
        pair("accept-language", "en-US,en;q=0.9"),
        pair("sec-ch-ua", &header_list(&hints.brands)),
        pair("sec-ch-ua-mobile", "?0"),
        pair("sec-ch-ua-platform", &format!("\"{}\"", hints.platform)),
    ]
}

/// The headers once the browser has seen `Accept-CH`.
fn full_visit(hints: &ClientHints) -> Vec<(String, String)> {
    let mut headers = first_visit(hints);
    headers.extend([
        pair(
            "sec-ch-ua-full-version-list",
            &header_list(&hints.full_version_list),
        ),
        pair(
            "sec-ch-ua-full-version",
            &format!("\"{}\"", hints.ua_full_version),
        ),
        pair(
            "sec-ch-ua-platform-version",
            &format!("\"{}\"", hints.platform_version),
        ),
        pair("sec-ch-ua-arch", &format!("\"{}\"", hints.architecture)),
        pair("sec-ch-ua-bitness", &format!("\"{}\"", hints.bitness)),
        pair("sec-ch-ua-model", &format!("\"{}\"", hints.model)),
        pair("sec-ch-ua-wow64", "?0"),
        pair("sec-ch-ua-form-factors", "\"Desktop\""),
    ]);
    headers
}

fn request(method: &str, target: &str, headers: Vec<(String, String)>) -> Http1Request {
    Http1Request {
        method: method.to_owned(),
        target: target.to_owned(),
        version: "HTTP/1.1".to_owned(),
        headers,
        body: Vec::new(),
    }
}

fn hints() -> ClientHints {
    client_hints_from(&report()).expect("a report with user agent data")
}

fn session(hints: &ClientHints) -> Vec<Http1Request> {
    vec![
        request("GET", "/", first_visit(hints)),
        request("GET", "/favicon.ico", first_visit(hints)),
        request("GET", "/", full_visit(hints)),
        request("GET", "/echo", full_visit(hints)),
        request("POST", "/report", full_visit(hints)),
    ]
}

#[test]
fn brand_lists_are_read_with_quoted_separators() {
    let parsed = parse_brand_list(
        "\"Chromium\";v=\"100\", \"Not;A=Brand\";v=\"99\", \"Google Chrome\";v=\"100\"",
    )
    .expect("a brand list");

    assert_eq!(
        parsed,
        [
            BrandVersion::new("Chromium", "100"),
            BrandVersion::new("Not;A=Brand", "99"),
            BrandVersion::new("Google Chrome", "100"),
        ]
    );
}

#[test]
fn brand_lists_tolerate_spacing_and_escapes() {
    let parsed =
        parse_brand_list("\"A\";v=\"1\",\"B\"; v=\"2\" , \"Q\\\"x\";v=\"3\"").expect("list");

    assert_eq!(
        parsed,
        [
            BrandVersion::new("A", "1"),
            BrandVersion::new("B", "2"),
            BrandVersion::new("Q\"x", "3"),
        ]
    );
    assert_eq!(parse_brand_list("").expect("empty"), []);
}

#[test]
fn malformed_brand_lists_are_refused() {
    for bad in [
        "Chromium;v=100",
        "\"Chromium\"",
        "\"Chromium\";v=100",
        "\"Chromium\";v=\"1\" \"x\"",
        "\"Chromium;v=\"1\"",
        "\"Chromium\";v=\"1",
        "\"A\";v=\"1\",",
    ] {
        assert!(
            matches!(parse_brand_list(bad), Err(CaptureError::HintsReport { .. })),
            "{bad}"
        );
    }
}

#[test]
fn string_lists_are_read() {
    assert_eq!(parse_string_list("\"Desktop\"").expect("one"), ["Desktop"]);
    assert_eq!(
        parse_string_list("\"Desktop\", \"EmbeddedDevice\"").expect("two"),
        ["Desktop", "EmbeddedDevice"]
    );
    assert_eq!(parse_string_list("").expect("empty"), Vec::<String>::new());
    assert!(parse_string_list("Desktop").is_err());
    assert!(parse_string_list("\"a\" \"b\"").is_err());
}

#[test]
fn a_report_becomes_client_hints() {
    let hints = hints();

    assert_eq!(hints.brands.len(), 3);
    assert_eq!(
        hints.brands.first().expect("a brand"),
        &BrandVersion::new("Chromium", "100")
    );
    assert_eq!(
        hints.full_version_list.get(2).expect("a brand").version,
        "100.0.1.2"
    );
    assert_eq!(hints.platform, "macOS");
    assert_eq!(hints.platform_version, "15.0.0");
    assert_eq!(hints.architecture, "arm");
    assert_eq!(hints.bitness, "64");
    assert_eq!(hints.model, "");
    assert_eq!(hints.ua_full_version, "100.0.1.2");
    assert!(!hints.mobile);
    assert!(!hints.wow64);
    assert_eq!(hints.form_factors, ["Desktop"]);
}

#[test]
fn the_full_version_falls_back_to_the_browser_brand() {
    let mut report = report();
    report.ua_data.as_mut().expect("data").high.ua_full_version = None;

    let hints = client_hints_from(&report).expect("derived");
    assert_eq!(hints.ua_full_version, "100.0.1.2");

    report
        .ua_data
        .as_mut()
        .expect("data")
        .high
        .full_version_list = vec![brand("Not;A=Brand", "99.0.0.0")];
    assert!(matches!(
        client_hints_from(&report),
        Err(CaptureError::HintsReport { .. })
    ));
}

#[test]
fn a_browser_without_user_agent_data_has_no_hints() {
    let mut report = report();
    report.ua_data = None;

    assert!(matches!(
        client_hints_from(&report),
        Err(CaptureError::NoUserAgentData)
    ));
}

#[test]
fn matching_headers_pass_the_cross_check() {
    let hints = hints();

    cross_check(&hints, &full_visit(&hints)).expect("full headers agree");
    cross_check(&hints, &first_visit(&hints)).expect("missing headers are skipped");
    cross_check(&hints, &[]).expect("no headers at all");
}

#[test]
fn a_differing_header_fails_the_cross_check_by_name() {
    let hints = hints();
    let changes = [
        ("sec-ch-ua", "\"Other\";v=\"1\""),
        ("sec-ch-ua-mobile", "?1"),
        ("sec-ch-ua-platform", "\"Windows\""),
        ("sec-ch-ua-full-version-list", "\"Chromium\";v=\"1.0.0.0\""),
        ("sec-ch-ua-full-version", "\"1.0.0.0\""),
        ("sec-ch-ua-platform-version", "\"1.0.0\""),
        ("sec-ch-ua-arch", "\"x86\""),
        ("sec-ch-ua-bitness", "\"32\""),
        ("sec-ch-ua-model", "\"Pixel\""),
        ("sec-ch-ua-wow64", "?1"),
        ("sec-ch-ua-form-factors", "\"Tablet\""),
    ];
    for (name, value) in changes {
        let mut headers = full_visit(&hints);
        for entry in &mut headers {
            if entry.0 == name {
                entry.1 = value.to_owned();
            }
        }

        let error = cross_check(&hints, &headers).expect_err("differs");

        assert!(
            matches!(&error, CaptureError::HintsDisagree { header } if header == name),
            "{name}: {error}"
        );
    }
}

#[test]
fn an_unreadable_header_is_a_report_error() {
    let hints = hints();
    let headers = vec![pair("sec-ch-ua", "junk")];

    assert!(matches!(
        cross_check(&hints, &headers),
        Err(CaptureError::HintsReport { .. })
    ));
}

#[test]
fn the_measurement_takes_the_first_and_last_document_requests() {
    let hints = hints();
    let requests = session(&hints);

    let measurement = measurement_from(&report(), &requests).expect("a measurement");

    assert_eq!(measurement.user_agent, UA);
    assert_eq!(measurement.accept_language, "en-US,en;q=0.9");
    assert_eq!(measurement.languages, ["en-US", "en"]);
    assert_eq!(measurement.client_hints, hints);
    assert_eq!(measurement.first_visit_headers, first_visit(&hints));
    assert_eq!(measurement.request_headers, full_visit(&hints));
}

#[test]
fn a_headless_browser_is_refused() {
    let mut report = report();
    report.user_agent = UA.replace("Chrome/", "HeadlessChrome/");
    let hints = hints();

    let error = measurement_from(&report, &session(&hints)).expect_err("headless");

    assert!(matches!(error, CaptureError::Headless), "{error}");
}

#[test]
fn a_session_without_a_document_request_is_refused() {
    let hints = hints();
    let requests = vec![request("GET", "/echo", full_visit(&hints))];

    assert!(matches!(
        measurement_from(&report(), &requests),
        Err(CaptureError::HintsReport { .. })
    ));
}

#[test]
fn a_browser_that_ignored_accept_ch_is_refused() {
    let hints = hints();
    let requests = vec![
        request("GET", "/", first_visit(&hints)),
        request("GET", "/", first_visit(&hints)),
    ];

    let error = measurement_from(&report(), &requests).expect_err("no high-entropy hints");

    assert!(
        matches!(
            error,
            CaptureError::HintsReport {
                reason: "no high-entropy hints in the last document request"
            }
        ),
        "{error}"
    );
}

#[test]
fn a_user_agent_that_differs_between_the_channels_is_refused() {
    let hints = hints();
    let mut requests = session(&hints);
    for entry in &mut requests.get_mut(2).expect("a request").headers {
        if entry.0 == "user-agent" {
            entry.1 = "Other/1".to_owned();
        }
    }

    let error = measurement_from(&report(), &requests).expect_err("differs");

    assert!(
        matches!(&error, CaptureError::HintsDisagree { header } if header == "user-agent"),
        "{error}"
    );
}

#[test]
fn a_document_request_without_accept_language_is_refused() {
    let hints = hints();
    let mut requests = session(&hints);
    requests
        .get_mut(2)
        .expect("a request")
        .headers
        .retain(|(name, _)| name != "accept-language");

    assert!(matches!(
        measurement_from(&report(), &requests),
        Err(CaptureError::HintsReport { .. })
    ));
}

#[test]
fn the_page_and_the_snippet_read_every_value_the_tables_need() {
    let page = page_html();
    let snippet = console_snippet();

    assert!(page.contains("collectHints"));
    assert!(page.contains("/echo") && page.contains("/report"));
    assert!(snippet.contains("console.log"));
    for key in HIGH_ENTROPY_KEYS {
        assert!(collect_js().contains(key), "{key}");
        assert!(snippet.contains(key), "{key}");
    }
    for header in [
        "Sec-CH-UA-Arch",
        "Sec-CH-UA-Bitness",
        "Sec-CH-UA-Form-Factors",
        "Sec-CH-UA-Full-Version-List",
        "Sec-CH-UA-Model",
        "Sec-CH-UA-Platform-Version",
        "Sec-CH-UA-WoW64",
    ] {
        assert!(ACCEPT_CH.contains(header), "{header}");
    }
}

/// Sends `request` to the server and reads the whole response.
///
/// A server that refuses a request may reset the connection while the client is still writing, so
/// a write or read error ends the exchange with whatever arrived.
async fn exchange(server: &HintsServer, request: &[u8]) -> String {
    let mut stream = TcpStream::connect(server.addr()).await.expect("connects");
    let _sent = stream.write_all(request).await;
    let mut response = Vec::new();
    let _read = tokio::time::timeout(Duration::from_secs(5), stream.read_to_end(&mut response))
        .await
        .expect("the server closes the connection");
    String::from_utf8_lossy(&response).into_owned()
}

fn post_report(body: &str) -> Vec<u8> {
    format!(
        "POST /report HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

#[tokio::test]
async fn the_hints_page_asks_for_the_hints_and_is_recorded() {
    let server = HintsServer::start().await.expect("starts");

    let response = exchange(
        &server,
        b"GET / HTTP/1.1\r\nHost: x\r\nUser-Agent: Probe/1\r\n\r\n",
    )
    .await;

    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");
    let lower = response.to_ascii_lowercase();
    assert!(lower.contains(&format!("accept-ch: {}", ACCEPT_CH.to_ascii_lowercase())));
    assert!(lower.contains("critical-ch:"));
    assert!(lower.contains("cache-control: no-store"));
    assert!(lower.contains("content-type: text/html"));
    assert!(response.contains("collectHints"));
    let recorded = server.requests();
    assert_eq!(recorded.len(), 1);
    assert_eq!(
        recorded.first().expect("one").header("user-agent"),
        Some("Probe/1")
    );
    assert_eq!(server.url(), format!("http://{}/", server.addr()));
}

#[tokio::test]
async fn a_posted_report_is_handed_to_the_caller() {
    let server = HintsServer::start().await.expect("starts");
    let body = serde_json::to_string(&report()).expect("serializes");

    let response = exchange(&server, &post_report(&body)).await;
    let received = server
        .next_report(Duration::from_secs(5))
        .await
        .expect("a report");

    assert!(response.starts_with("HTTP/1.1 204 "), "{response}");
    assert_eq!(received, report());
}

#[tokio::test]
async fn a_report_that_is_not_the_expected_json_is_an_error_for_the_caller() {
    let server = HintsServer::start().await.expect("starts");

    exchange(&server, &post_report("{\"userAgent\": 7}")).await;
    let error = server
        .next_report(Duration::from_secs(5))
        .await
        .expect_err("wrong shape");

    assert!(matches!(error, CaptureError::HintsReport { .. }), "{error}");
}

#[tokio::test]
async fn waiting_for_a_report_times_out() {
    let server = HintsServer::start().await.expect("starts");

    let error = server
        .next_report(Duration::from_millis(30))
        .await
        .expect_err("nothing posted");

    assert!(matches!(error, CaptureError::Timeout { .. }), "{error}");
}

#[tokio::test]
async fn the_other_routes_answer_without_a_body_or_not_at_all() {
    let server = HintsServer::start().await.expect("starts");

    let echo = exchange(&server, b"GET /echo HTTP/1.1\r\nHost: x\r\n\r\n").await;
    let missing = exchange(&server, b"GET /nope HTTP/1.1\r\nHost: x\r\n\r\n").await;
    let wrong_method = exchange(&server, b"DELETE / HTTP/1.1\r\nHost: x\r\n\r\n").await;
    let junk = exchange(&server, b"not http at all\r\n\r\n").await;

    assert!(echo.starts_with("HTTP/1.1 204 "), "{echo}");
    assert!(missing.starts_with("HTTP/1.1 404 "), "{missing}");
    assert!(wrong_method.starts_with("HTTP/1.1 404 "), "{wrong_method}");
    assert!(
        junk.is_empty(),
        "a malformed request gets no answer: {junk}"
    );
    let paths: Vec<String> = server
        .requests()
        .iter()
        .map(|r| r.path().to_owned())
        .collect();
    assert_eq!(paths, ["/echo", "/nope", "/"]);
}

#[tokio::test]
async fn an_oversized_report_is_not_read() {
    let server = HintsServer::start().await.expect("starts");
    let body = "x".repeat(MAX_HTTP_BODY + 1);

    let response = exchange(&server, &post_report(&body)).await;

    assert!(response.is_empty(), "{response}");
    let error = server
        .next_report(Duration::from_millis(30))
        .await
        .expect_err("nothing was accepted");
    assert!(matches!(error, CaptureError::Timeout { .. }), "{error}");
}

#[tokio::test]
async fn dropping_the_server_closes_the_port() {
    let server = HintsServer::start().await.expect("starts");
    let addr = server.addr();
    drop(server);

    let closed = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if TcpStream::connect(addr).await.is_err() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;

    assert!(closed.is_ok(), "the listener should be gone");
}
