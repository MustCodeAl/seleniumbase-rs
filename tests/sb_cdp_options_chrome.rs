//! The launch options of the pure CDP engine against a real Chrome: mobile
//! emulation in every tab, the colour scheme, the switches that become Chrome
//! arguments or profile preferences, the content security policy bypass, and
//! the proxy bypass list with the host resolver rules.
//!
//! They start Chrome, so they are ignored by default. Only one Chrome-driving
//! test binary may run on the machine at a time:
//!
//! ```text
//! cargo test --test sb_cdp_options_chrome -- --ignored --test-threads=1
//! ```

use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use seleniumbase_rs::browser::options::{DeviceMetrics, LaunchSwitch};
use seleniumbase_rs::sb_cdp::{Browser, ColorScheme, LaunchOptions, LaunchOptionsBuilder, Page};

/// A page that fits a phone: without the viewport tag a mobile browser lays
/// the page out 980 pixels wide.
const PHONE_PAGE: &str = r#"<!doctype html><html><head>
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Phone</title></head><body>phone</body></html>"#;

/// Inline script that a policy without `script-src` refuses.
const POLICY_PAGE: &str = "<!doctype html><title>blocked</title>\
<script>document.title = 'ran';</script>";

const IMAGE_PAGE: &str = r#"<!doctype html><title>Image</title><img id="pic" src="/pic.svg">"#;

const IMAGE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"/>"#;

/// An HTTP response with a body and extra header lines.
fn response(content_type: &str, extra: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\n{extra}Content-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    )
}

/// What the server saw: the first line and the headers of every request.
type Heads = Arc<Mutex<Vec<String>>>;

/// Serves `answer(request head)` on a loopback port, and remembers every
/// request head. Returns the port.
fn serve(answer: fn(&str) -> String) -> (u16, Heads) {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("binds a loopback port");
    let port = listener.local_addr().expect("an address").port();
    let heads = Heads::default();
    let record = Arc::clone(&heads);
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let record = Arc::clone(&record);
            thread::spawn(move || {
                let mut buffer = [0_u8; 8192];
                let read = stream.read(&mut buffer).unwrap_or(0);
                if read == 0 {
                    return;
                }
                let head = String::from_utf8_lossy(&buffer[..read]).into_owned();
                let reply = answer(&head);
                record
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push(head);
                let _ = stream.write_all(reply.as_bytes());
            });
        }
    });
    (port, heads)
}

/// The site: the phone page, a page a policy guards, and a page with a picture.
fn site(head: &str) -> String {
    let path = head.split_whitespace().nth(1).unwrap_or("/");
    match path {
        "/policy" => response(
            "text/html",
            "Content-Security-Policy: script-src 'none'\r\n",
            POLICY_PAGE,
        ),
        "/image" => response("text/html", "", IMAGE_PAGE),
        "/pic.svg" => response("image/svg+xml", "", IMAGE),
        _ => response("text/html", "", PHONE_PAGE),
    }
}

/// The builder every test starts from: headless, with the sandbox kept unless
/// the continuous integration machine cannot give one.
fn builder() -> LaunchOptionsBuilder {
    LaunchOptions::builder()
        .headless(true)
        .no_sandbox(std::env::var_os("CI").is_some())
}

async fn launch(builder: LaunchOptionsBuilder) -> Browser {
    let options = builder.build().expect("launch options");
    Browser::launch(options).await.expect("Chrome starts")
}

async fn number(page: &Page, expression: &str) -> f64 {
    let value = page.evaluate(expression).await.expect("the script runs");
    value
        .as_f64()
        .unwrap_or_else(|| panic!("{expression} is not a number: {value}"))
}

/// The command line of the running browser, as `chrome://version` shows it.
async fn command_line(browser: &Browser) -> String {
    let page = browser.default_page().await.expect("a tab");
    page.goto("chrome://version")
        .await
        .expect("the version page");
    let text = page
        .evaluate("document.getElementById('command_line').textContent")
        .await
        .expect("the command line is on the page");
    text.as_str().expect("text").to_owned()
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn a_mobile_launch_makes_every_tab_a_phone() {
    let (port, _heads) = serve(site);
    let browser = launch(builder().mobile(true)).await;
    let page = browser.default_page().await.expect("a tab");
    page.goto(format!("http://127.0.0.1:{port}/"))
        .await
        .expect("the page loads");

    assert!((number(&page, "window.innerWidth").await - 390.0).abs() < f64::EPSILON);
    assert!((number(&page, "window.devicePixelRatio").await - 3.0).abs() < f64::EPSILON);
    assert!(number(&page, "navigator.maxTouchPoints").await > 0.0);
    let agent = page
        .evaluate("navigator.userAgent")
        .await
        .expect("an agent");
    assert!(
        agent
            .as_str()
            .is_some_and(|agent| agent.contains("Android")),
        "{agent}"
    );
    assert_eq!(
        page.evaluate("navigator.userAgentData.mobile")
            .await
            .expect("client hints"),
        true
    );

    // A tab opened later is a phone too.
    let second = browser
        .new_page_at(format!("http://127.0.0.1:{port}/"))
        .await
        .expect("a second tab");
    assert!((number(&second, "window.innerWidth").await - 390.0).abs() < f64::EPSILON);
    assert!(number(&second, "navigator.maxTouchPoints").await > 0.0);
    browser.close().await.expect("Chrome closes");
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn chosen_metrics_are_the_screen_of_the_phone() {
    let (port, _heads) = serve(site);
    let browser = launch(builder().device_metrics(DeviceMetrics::new(360, 640, 2.0))).await;
    let page = browser.default_page().await.expect("a tab");
    page.goto(format!("http://127.0.0.1:{port}/"))
        .await
        .expect("the page loads");

    assert!((number(&page, "window.innerWidth").await - 360.0).abs() < f64::EPSILON);
    assert!((number(&page, "window.devicePixelRatio").await - 2.0).abs() < f64::EPSILON);

    // The screen of a running tab can be taken away again.
    page.emulation()
        .clear_device_metrics()
        .await
        .expect("the override goes");
    assert!(number(&page, "window.innerWidth").await > 390.0);
    browser.close().await.expect("Chrome closes");
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn the_colour_scheme_of_a_tab_can_be_chosen() {
    let browser = launch(builder()).await;
    let page = browser.default_page().await.expect("a tab");
    let dark = "matchMedia('(prefers-color-scheme: dark)').matches";

    page.emulation()
        .color_scheme(ColorScheme::Dark)
        .await
        .expect("dark is accepted");
    assert_eq!(page.evaluate(dark).await.expect("a media query"), true);
    page.emulation()
        .color_scheme(ColorScheme::Light)
        .await
        .expect("light is accepted");
    assert_eq!(page.evaluate(dark).await.expect("a media query"), false);
    browser.close().await.expect("Chrome closes");
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn the_switches_reach_the_command_line_of_the_browser() {
    let browser = launch(
        builder()
            .dark_mode(true)
            .disable_web_security(true)
            .disable_features(["TranslateUI"])
            .host_resolver_rules("MAP never.test 127.0.0.1"),
    )
    .await;
    let arguments = command_line(&browser).await;
    browser.close().await.expect("Chrome closes");

    for want in [
        "--enable-features=WebContentsForceDark",
        "--disable-web-security",
        "--host-resolver-rules=MAP never.test 127.0.0.1",
        "TranslateUI",
    ] {
        assert!(
            arguments.contains(want),
            "{want} is missing from {arguments}"
        );
    }
    assert!(arguments.contains("--disable-features="), "{arguments}");
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn the_content_security_policy_is_bypassed_only_when_asked() {
    let (port, _heads) = serve(site);
    let url = format!("http://127.0.0.1:{port}/policy");

    let guarded = launch(builder()).await;
    let page = guarded.default_page().await.expect("a tab");
    page.goto(&url).await.expect("the page loads");
    assert_eq!(
        page.evaluate("document.title").await.expect("a title"),
        "blocked"
    );
    guarded.close().await.expect("Chrome closes");

    let bypassed = launch(builder().switch(LaunchSwitch::DisableCsp, true)).await;
    let page = bypassed.default_page().await.expect("a tab");
    page.goto(&url).await.expect("the page loads");
    assert_eq!(
        page.evaluate("document.title").await.expect("a title"),
        "ran"
    );
    bypassed.close().await.expect("Chrome closes");
}

/// Whether the picture of the image page loaded, after it had its time to.
async fn picture_width(page: &Page) -> f64 {
    let mut width = 0.0;
    for _ in 0..30 {
        width = number(page, "document.getElementById('pic').naturalWidth").await;
        if width > 0.0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    width
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn block_images_is_a_profile_preference_the_browser_follows() {
    let (port, _heads) = serve(site);
    let url = format!("http://127.0.0.1:{port}/image");

    let plain = launch(builder()).await;
    let page = plain.default_page().await.expect("a tab");
    page.goto(&url).await.expect("the page loads");
    assert!(
        picture_width(&page).await > 0.0,
        "the picture does not load even when images are allowed"
    );
    plain.close().await.expect("Chrome closes");

    let blocked = launch(builder().switch(LaunchSwitch::BlockImages, true)).await;
    let page = blocked.default_page().await.expect("a tab");
    page.goto(&url).await.expect("the page loads");
    assert!(
        picture_width(&page).await.abs() < f64::EPSILON,
        "the picture loaded although images are blocked"
    );
    blocked.close().await.expect("Chrome closes");
}

/// A page the proxy answers, titled `proxied`.
fn proxy_answer(_head: &str) -> String {
    response("text/html", "", "<!doctype html><title>proxied</title>")
}

/// A page the site answers without a proxy, titled `direct`.
fn direct_answer(_head: &str) -> String {
    response("text/html", "", "<!doctype html><title>direct</title>")
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn a_host_on_the_bypass_list_skips_the_proxy() {
    let (proxy_port, proxy_heads) = serve(proxy_answer);
    let (direct_port, direct_heads) = serve(direct_answer);
    let browser = launch(
        builder()
            .proxy(format!("http://127.0.0.1:{proxy_port}"))
            .proxy_bypass_list("bypassed.test".parse().expect("a bypass list"))
            // The name a bypassed request is made to has to lead somewhere.
            .host_resolver_rules(format!("MAP bypassed.test 127.0.0.1:{direct_port}")),
    )
    .await;
    let page = browser.default_page().await.expect("a tab");

    page.goto("http://proxied.test/")
        .await
        .expect("through the proxy");
    assert_eq!(
        page.evaluate("document.title").await.expect("a title"),
        "proxied"
    );
    page.goto("http://bypassed.test/").await.expect("direct");
    assert_eq!(
        page.evaluate("document.title").await.expect("a title"),
        "direct"
    );
    browser.close().await.expect("Chrome closes");

    let proxied = proxy_heads.lock().unwrap_or_else(PoisonError::into_inner);
    assert!(
        proxied.iter().any(|head| head.contains("proxied.test")),
        "the proxy never saw the first page: {proxied:?}"
    );
    assert!(
        !proxied.iter().any(|head| head.contains("bypassed.test")),
        "a bypassed host went to the proxy: {proxied:?}"
    );
    let direct = direct_heads.lock().unwrap_or_else(PoisonError::into_inner);
    assert!(
        direct.iter().any(|head| head.contains("bypassed.test")),
        "the bypassed host was never asked for directly: {direct:?}"
    );
}
