//! The Pure CDP API against a real Chrome.
//!
//! These tests need a Chrome or Chromium on the machine, so they are ignored
//! by default. Run them with:
//!
//! ```text
//! cargo test --test sb_cdp_chrome -- --ignored
//! ```

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

use seleniumbase_rs::sb_cdp::{
    Browser, BrowserPool, Cookie, LaunchOptions, Outcome, Page, PoolOptions, Response, Rule,
    SelectBy, State,
};
use seleniumbase_rs::stealth::behavior::Behavior;
use seleniumbase_rs::SeleniumBaseError;
use serde_json::json;

const FIXTURE: &str = r##"<!doctype html>
<html><head><title>Fixture</title>
<style>.hidden{display:none} .zero{width:0;height:0}</style></head>
<body>
<h1 id="title">Welcome</h1>
<form id="login" onsubmit="document.title='submitted:'+document.getElementById('user').value;return false;">
  <input id="user" name="user"> <input id="pass" type="password">
  <button id="submit" type="submit">Go</button>
</form>
<a href="#one">Sign in</a> <a href="#two">Sign in help</a>
<select id="country">
  <option value="us">United States</option><option value="de">Germany</option><option value="fr">France</option>
</select>
<input type="checkbox" id="agree">
<ul id="items"><li>one</li><li class="hidden">two</li><li>three</li><li>four</li></ul>
<div id="hidden" class="hidden">secret</div><div id="zero" class="zero"></div>
<button id="counter" onclick="this.dataset.trusted=event.isTrusted;this.textContent=String(Number(this.textContent)+1)">0</button>
<input id="keys">
<div id="card"><span class="label">inside</span></div><span class="label">outside</span>
<script>
  window.keylog = [];
  document.getElementById('keys').addEventListener('keydown', e => window.keylog.push([e.key, e.isTrusted]));
  window.keytimes = []; window.moves = 0;
  document.getElementById('keys').addEventListener('keydown', () => window.keytimes.push(performance.now()));
  document.addEventListener('mousemove', e => { if (e.isTrusted) window.moves++; });
  setTimeout(() => {
    const late = document.createElement('div');
    late.id = 'late'; late.textContent = 'arrived';
    document.body.append(late);
  }, 600);
</script></body></html>"##;

/// A page of frames. `srcdoc` frames share the page's origin, so they can be
/// entered; the third frame is served from another port, so it cannot.
const FRAMES: &str = r##"<!doctype html>
<html><head><title>Frames</title></head>
<body>
<h1 id="top">Top</h1>
<iframe id="inner" style="margin:60px 0 0 40px;width:320px;height:120px"
  srcdoc="<button id='inside' onclick='parent.document.title=&quot;clicked-inside:&quot;+event.isTrusted'>In</button> <input id='name'> <iframe id='deep' style='width:200px;height:60px'></iframe>"></iframe>
<iframe id="foreign" src="OTHER_ORIGIN/" style="width:200px;height:60px"></iframe>
<script>
  // The innermost frame is filled in from script: three levels of attribute
  // quoting in one string would not survive.
  window.addEventListener('load', () => {
    const deep = document.getElementById('inner').contentDocument.getElementById('deep');
    deep.srcdoc = '<button id="bottom">Deep</button><script>' +
      'document.getElementById("bottom").onclick = () => { top.document.title = "clicked-deep"; };' +
      '<\/script>';
  });
</script>
</body></html>"##;

/// Serves the fixture, and a page that sets a cookie, on a loopback port.
fn serve() -> String {
    let other = serve_other_origin();
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let address = listener.local_addr().expect("bound address");
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buffer = [0_u8; 2048];
            let read = stream.read(&mut buffer).unwrap_or(0);
            let request = String::from_utf8_lossy(&buffer[..read]);
            let cookie = if request.starts_with("GET /cookie") {
                "Set-Cookie: sid=abc123; Path=/\r\n"
            } else {
                ""
            };
            let body = if request.starts_with("GET /frames") {
                FRAMES.replace("OTHER_ORIGIN", &other)
            } else {
                FIXTURE.to_owned()
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n{cookie}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    format!("http://{address}")
}

/// A second origin (another port) whose page has a button the first cannot reach.
fn serve_other_origin() -> String {
    const BODY: &str = "<!doctype html><button id='secret'>Foreign</button>";
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let address = listener.local_addr().expect("bound address");
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buffer = [0_u8; 1024];
            let _ = stream.read(&mut buffer);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{BODY}",
                BODY.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    format!("http://{address}")
}

async fn open() -> (Browser, Page, String) {
    let base = serve();
    let options = LaunchOptions::builder()
        .headless(true)
        .no_sandbox(std::env::var_os("CI").is_some())
        .window_size(1000, 800)
        .build()
        .unwrap();
    let browser = Browser::launch(options).await.expect("Chrome starts");
    let page = browser.default_page().await.unwrap();
    page.goto(format!("{base}/")).await.unwrap();
    (browser, page.with_timeout(Duration::from_secs(3)), base)
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn finds_elements_by_css_xpath_and_link_text() {
    let (browser, page, _) = open().await;

    assert_eq!(page.locator("#title").count().await.unwrap(), 1);
    assert_eq!(page.locator("//h1").count().await.unwrap(), 1);
    assert_eq!(
        page.locator("link=Sign in").count().await.unwrap(),
        1,
        "exact link text"
    );
    assert_eq!(
        page.locator("partial_link=Sign in").count().await.unwrap(),
        2,
        "partial link text"
    );
    assert_eq!(page.locator("#items li").count().await.unwrap(), 4);
    assert_eq!(
        page.locator("#items li").visible().count().await.unwrap(),
        3
    );
    assert_eq!(
        page.locator("#items li")
            .visible()
            .nth(1)
            .text()
            .await
            .unwrap(),
        "three"
    );
    assert_eq!(
        page.locator("#items li").last().text().await.unwrap(),
        "four"
    );

    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn an_xpath_inside_a_parent_only_matches_within_it() {
    let (browser, page, _) = open().await;

    // "//span" on its own matches both spans; scoped to #card it must not
    // reach the one outside.
    assert_eq!(page.locator("//span").count().await.unwrap(), 2);
    assert_eq!(
        page.locator("#card")
            .locator("//span")
            .count()
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        page.locator("#card")
            .locator(".label")
            .text()
            .await
            .unwrap(),
        "inside"
    );
    // Relative paths may lead out of the scope they start from.
    let card = page.locator("#card");
    assert_eq!(
        card.parent().count().await.unwrap(),
        1,
        "the parent of #card is <body>"
    );
    assert_eq!(card.locator("./..").count().await.unwrap(), 1);
    assert_eq!(card.locator("parent::body").count().await.unwrap(), 1);
    assert_eq!(
        page.locator(".label")
            .first()
            .parent()
            .attribute("id")
            .await
            .unwrap()
            .as_deref(),
        Some("card")
    );

    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn fills_a_form_and_submits_it() {
    let (browser, page, _) = open().await;

    page.locator("#user").fill("demo_user").await.unwrap();
    page.locator("#submit").click().await.unwrap();

    page.expect()
        .to_have_title("submitted:demo_user")
        .await
        .unwrap();
    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn typing_produces_trusted_key_events_one_per_character() {
    let (browser, page, _) = open().await;

    page.locator("#keys").type_text("ab").await.unwrap();

    let log = page.evaluate("window.keylog").await.unwrap();
    assert_eq!(log, json!([["a", true], ["b", true]]));
    assert_eq!(
        page.locator("#keys").attribute("value").await.unwrap(),
        None,
        "value is a property, not an attribute"
    );
    assert_eq!(
        page.evaluate("document.getElementById('keys').value")
            .await
            .unwrap(),
        "ab"
    );
    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn clicks_are_trusted_events() {
    let (browser, page, _) = open().await;
    let counter = page.locator("#counter");

    counter.click().await.unwrap();
    counter.click().await.unwrap();

    assert_eq!(counter.text().await.unwrap(), "2");
    assert_eq!(
        counter.attribute("data-trusted").await.unwrap().as_deref(),
        Some("true")
    );
    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn waits_and_expectations_see_elements_that_arrive_later() {
    let (browser, page, _) = open().await;
    let late = page.locator("#late");

    assert!(
        !late.exists().await.unwrap(),
        "the element is not there yet"
    );
    late.wait_for(State::Visible).await.unwrap();
    late.expect().to_have_text("arrived").await.unwrap();

    page.locator("#nothing")
        .expect()
        .not()
        .to_exist()
        .await
        .unwrap();
    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn hidden_elements_exist_but_refuse_to_be_clicked() {
    let (browser, page, _) = open().await;
    let page = page.with_timeout(Duration::from_millis(400));

    for selector in ["#hidden", "#zero"] {
        let locator = page.locator(selector);
        assert!(locator.exists().await.unwrap(), "{selector} is in the DOM");
        assert!(
            !locator.is_visible().await.unwrap(),
            "{selector} is not visible"
        );
        let error = locator.click().await.unwrap_err();
        assert!(
            matches!(error, SeleniumBaseError::ElementNotInteractable { .. }),
            "{selector}: {error}"
        );
    }
    let error = page.locator("#absent").click().await.unwrap_err();
    assert!(
        matches!(error, SeleniumBaseError::ElementNotFound { .. }),
        "{error}"
    );
    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn chooses_dropdown_options_and_toggles_checkboxes() {
    let (browser, page, _) = open().await;
    let country = page.locator("#country");
    let value = || page.evaluate("document.getElementById('country').value");

    country
        .select_option(SelectBy::Text("Germany"))
        .await
        .unwrap();
    assert_eq!(value().await.unwrap(), "de");
    country.select_option(SelectBy::Value("fr")).await.unwrap();
    assert_eq!(value().await.unwrap(), "fr");
    country.select_option(SelectBy::Index(0)).await.unwrap();
    assert_eq!(value().await.unwrap(), "us");
    let error = country
        .select_option(SelectBy::Text("Mars"))
        .await
        .unwrap_err();
    assert!(
        matches!(error, SeleniumBaseError::InvalidSelector(_)),
        "{error}"
    );

    let agree = page.locator("#agree");
    agree.check().await.unwrap();
    assert!(agree.is_checked().await.unwrap());
    agree.check().await.unwrap();
    assert!(
        agree.is_checked().await.unwrap(),
        "checking twice leaves it checked"
    );
    agree.uncheck().await.unwrap();
    assert!(!agree.is_checked().await.unwrap());
    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn cookies_and_storage_round_trip() {
    let (browser, page, base) = open().await;
    page.goto(format!("{base}/cookie")).await.unwrap();

    let cookies = page.cookies().all().await.unwrap();
    assert!(
        cookies.iter().any(|c| c.name == "sid"),
        "the server's cookie arrived: {cookies:?}"
    );
    assert!(page
        .cookies()
        .header()
        .await
        .unwrap()
        .contains("sid=abc123"));

    let file = tempfile::NamedTempFile::new().unwrap();
    assert_eq!(
        page.cookies().save(file.path(), Some("sid")).await.unwrap(),
        1
    );
    page.cookies().clear().await.unwrap();
    assert!(page.cookies().all().await.unwrap().is_empty());
    assert_eq!(page.cookies().load(file.path(), None).await.unwrap(), 1);
    assert!(page
        .cookies()
        .header()
        .await
        .unwrap()
        .contains("sid=abc123"));

    let local = page.local_storage();
    local.set("theme", "dark").await.unwrap();
    assert_eq!(local.get("theme").await.unwrap().as_deref(), Some("dark"));
    local.remove("theme").await.unwrap();
    assert_eq!(local.get("theme").await.unwrap(), None);
    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn captures_png_and_pdf_and_an_element() {
    let (browser, page, _) = open().await;

    let png = page.screenshot().await.unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    let pdf = page.pdf().await.unwrap();
    assert_eq!(&pdf[..5], b"%PDF-");
    let element = page.locator("#title").screenshot().await.unwrap();
    assert_eq!(&element[..4], b"\x89PNG");
    assert!(
        element.len() < png.len(),
        "an element is smaller than the page"
    );
    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn manages_tabs_independently() {
    let (browser, first, base) = open().await;

    let second = browser.new_page(Some(format!("{base}/"))).await.unwrap();
    assert_eq!(browser.list_pages().await.unwrap().len(), 2);

    // Each handle talks to its own tab, concurrently.
    first.evaluate("document.title = 'first'").await.unwrap();
    second.evaluate("document.title = 'second'").await.unwrap();
    let (a, b) = tokio::join!(first.title(), second.title());
    assert_eq!(
        (a.unwrap(), b.unwrap()),
        ("first".to_owned(), "second".to_owned())
    );

    second.close().await.unwrap();
    assert_eq!(browser.list_pages().await.unwrap().len(), 1);
    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn reports_and_changes_the_window() {
    let (browser, page, _) = open().await;

    let before = page.window().bounds().await.unwrap();
    assert!(before.width > 0 && before.height > 0);
    page.window().set_bounds(10, 10, 700, 500).await.unwrap();
    let after = page.window().bounds().await.unwrap();
    assert_eq!((after.width, after.height), (700, 500));
    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn navigation_history_and_reload_work() {
    let (browser, page, base) = open().await;

    page.goto(format!("{base}/cookie")).await.unwrap();
    assert!(page.url().await.unwrap().ends_with("/cookie"));
    page.back().await.unwrap();
    assert!(page.url().await.unwrap().ends_with('/'));
    page.forward().await.unwrap();
    assert!(page.url().await.unwrap().ends_with("/cookie"));
    page.reload().await.unwrap();
    assert!(page.content().await.unwrap().contains("Welcome"));
    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn locators_reach_into_same_origin_frames_and_clicks_land_through_the_frame_offset() {
    let (browser, page, base) = open().await;
    page.goto(format!("{base}/frames")).await.unwrap();

    let inner = page.locator("#inner");
    let button = inner.locator("#inside");
    assert_eq!(button.text().await.unwrap(), "In");
    assert!(button.is_visible().await.unwrap());

    // A trusted click, delivered at the frame's offset, reaches the right button.
    button.click().await.unwrap();
    assert_eq!(page.title().await.unwrap(), "clicked-inside:true");

    // Typing goes to the field inside the frame.
    inner.locator("#name").fill("framed").await.unwrap();
    let typed: String = page
        .evaluate_as(
            "document.getElementById('inner').contentDocument.getElementById('name').value",
        )
        .await
        .unwrap();
    assert_eq!(typed, "framed");

    // XPath works inside a frame too.
    assert_eq!(inner.locator("//button").count().await.unwrap(), 1);

    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn frames_nest_and_a_cross_origin_frame_is_empty_rather_than_an_error() {
    let (browser, page, base) = open().await;
    page.goto(format!("{base}/frames")).await.unwrap();

    // A frame inside a frame: both offsets are added.
    page.locator("#inner")
        .locator("#deep")
        .locator("#bottom")
        .click()
        .await
        .unwrap();
    assert_eq!(page.title().await.unwrap(), "clicked-deep");

    // The foreign frame cannot be entered, so nothing matches inside it.
    let foreign = page.locator("#foreign").locator("#secret");
    assert_eq!(foreign.count().await.unwrap(), 0);
    assert!(!foreign.exists().await.unwrap());

    // The frame element itself is still an ordinary element.
    assert!(page.locator("#foreign").is_visible().await.unwrap());

    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn human_input_is_trusted_moves_in_many_steps_and_types_with_varied_timing() {
    let (browser, page, _) = open().await;
    let person = page.human(
        Behavior::builder()
            .typing_wpm(120.0)
            .seed(11)
            .build()
            .unwrap(),
    );

    // A click: many trusted mousemove events, then one trusted click.
    person.click(&page.locator("#counter")).await.unwrap();
    let moves: u32 = page.evaluate_as("window.moves").await.unwrap();
    assert!(
        moves > 10,
        "the page should see a path of moves, saw {moves}"
    );
    let trusted: String = page
        .evaluate_as("document.getElementById('counter').dataset.trusted")
        .await
        .unwrap();
    assert_eq!(trusted, "true");
    assert_eq!(page.locator("#counter").text().await.unwrap(), "1");

    // Typing: the right text, every key trusted, and an uneven rhythm.
    let text = "hello human typing";
    person
        .type_text(&page.locator("#keys"), text)
        .await
        .unwrap();
    let typed: String = page
        .evaluate_as("document.getElementById('keys').value")
        .await
        .unwrap();
    assert_eq!(typed, text);
    let untrusted: u32 = page
        .evaluate_as("window.keylog.filter(k => !k[1]).length")
        .await
        .unwrap();
    assert_eq!(untrusted, 0, "every key must be a trusted event");

    let times: Vec<f64> = page.evaluate_as("window.keytimes").await.unwrap();
    let gaps: Vec<f64> = times.windows(2).map(|pair| pair[1] - pair[0]).collect();
    let mean = gaps.iter().sum::<f64>() / gaps.len() as f64;
    let spread = (gaps.iter().map(|g| (g - mean).powi(2)).sum::<f64>() / gaps.len() as f64).sqrt();
    assert!((50.0..300.0).contains(&mean), "mean gap {mean:.0} ms");
    assert!(
        spread > 10.0,
        "the rhythm should vary, spread {spread:.1} ms"
    );

    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn pooled_workers_are_isolated_and_can_share_a_session_in_memory() {
    let base = serve();
    let pool = BrowserPool::new(
        LaunchOptions::builder()
            .headless(true)
            .no_sandbox(std::env::var_os("CI").is_some())
            .build()
            .unwrap(),
        PoolOptions::builder()
            .max_browsers(2)
            .contexts_per_browser(2)
            .build()
            .unwrap(),
    );

    // Four workers at once, each setting a cookie that only it may see.
    let mut workers = tokio::task::JoinSet::new();
    for n in 0..4 {
        let (pool, base) = (pool.clone(), base.clone());
        workers.spawn(async move {
            let lease = pool.acquire().await.unwrap();
            lease.page().goto(format!("{base}/")).await.unwrap();
            let mine = Cookie::new("who", format!("worker-{n}")).domain("127.0.0.1");
            lease.page().cookies().set(&[mine]).await.unwrap();
            // Stay long enough for the others to be running too.
            tokio::time::sleep(Duration::from_millis(400)).await;
            let seen: Vec<String> = lease
                .page()
                .cookies()
                .all()
                .await
                .unwrap()
                .into_iter()
                .filter(|cookie| cookie.name == "who")
                .map(|cookie| cookie.value)
                .collect();
            lease.release().await;
            (n, seen)
        });
    }
    while let Some(done) = workers.join_next().await {
        let (n, seen) = done.unwrap();
        assert_eq!(
            seen,
            [format!("worker-{n}")],
            "worker {n} must see only its own cookie"
        );
    }
    assert_eq!(
        pool.stats().await.launched,
        2,
        "four leases fit in two browsers"
    );

    // One worker signs in; another, in a fresh context, starts signed in.
    let alice = pool.acquire().await.unwrap();
    alice.page().goto(format!("{base}/cookie")).await.unwrap();
    alice.save_session("alice").await.unwrap();
    alice.release().await;
    assert_eq!(pool.sessions().names(), ["alice"]);

    let bob = pool.acquire().await.unwrap();
    bob.page().goto(format!("{base}/")).await.unwrap();
    let before = bob.page().cookies().all().await.unwrap();
    assert!(
        before.iter().all(|cookie| cookie.name != "sid"),
        "a new context starts empty"
    );
    assert!(bob.load_session("alice").await.unwrap());
    let after = bob.page().cookies().all().await.unwrap();
    assert!(
        after
            .iter()
            .any(|cookie| cookie.name == "sid" && cookie.value == "abc123"),
        "the saved login arrives in the new context"
    );
    bob.release().await;

    pool.close().await;
}

/// A server with a page, an image, a JSON API and an endpoint that echoes the
/// request it receives.
fn serve_routes() -> String {
    // A 1x1 PNG.
    const PIXEL: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";
    const PAGE: &str = "<!doctype html><title>start</title><img id=pic src=/logo.png>\
        <script>fetch('/api/data').then(r=>r.json()).then(j=>{document.title=JSON.stringify(j)});\
        fetch('/echo').then(r=>r.text()).then(t=>{window.echoed=t})</script>";
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let address = listener.local_addr().expect("bound address");
    thread::spawn(move || {
        use base64::Engine as _;
        let pixel = base64::engine::general_purpose::STANDARD
            .decode(PIXEL)
            .unwrap();
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buffer = [0_u8; 4096];
            let read = stream.read(&mut buffer).unwrap_or(0);
            let request = String::from_utf8_lossy(&buffer[..read]).to_lowercase();
            let path = request.split_whitespace().nth(1).unwrap_or("/").to_owned();
            let (kind, body): (&str, Vec<u8>) = match path.as_str() {
                "/logo.png" => ("image/png", pixel.clone()),
                "/api/data" => ("application/json", br#"{"real":true}"#.to_vec()),
                "/echo" => ("text/plain", request.clone().into_bytes()),
                _ => ("text/html", PAGE.as_bytes().to_vec()),
            };
            let head = format!("HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body);
        }
    });
    format!("http://{address}")
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn interception_blocks_mocks_and_rewrites_real_requests_and_stops_cleanly() {
    let base = serve_routes();
    let browser = Browser::launch(
        LaunchOptions::builder()
            .headless(true)
            .no_sandbox(std::env::var_os("CI").is_some())
            .build()
            .unwrap(),
    )
    .await
    .unwrap();
    let page = browser
        .default_page()
        .await
        .unwrap()
        .with_timeout(Duration::from_secs(5));

    let interception = page
        .intercept(vec![
            Rule::block().url("*/logo.png"),
            Rule::fulfill(Response::json(200, &json!({ "mocked": true }))).url("*/api/data"),
            Rule::modify().set_header("X-Probe", "yes").url("*/echo"),
        ])
        .await
        .unwrap();
    page.goto(format!("{base}/")).await.unwrap();
    page.wait_for_function("document.title.startsWith('{')")
        .await
        .unwrap();
    page.wait_for_function("window.echoed !== undefined")
        .await
        .unwrap();

    // The API answer is the made-up one, the image never loaded, and the
    // server saw the header we added.
    assert_eq!(page.title().await.unwrap(), r#"{"mocked":true}"#);
    let blocked: bool = page
        .evaluate_as("document.getElementById('pic').complete && document.getElementById('pic').naturalWidth === 0")
        .await
        .unwrap();
    assert!(blocked, "the blocked image must not have loaded");
    let echoed: String = page.evaluate_as("window.echoed").await.unwrap();
    assert!(echoed.contains("x-probe: yes"), "{echoed}");

    let outcomes: Vec<(String, Outcome)> = interception
        .log()
        .into_iter()
        .map(|seen| {
            (
                seen.request
                    .url
                    .rsplit('/')
                    .next()
                    .unwrap_or_default()
                    .to_owned(),
                seen.outcome,
            )
        })
        .collect();
    assert!(
        outcomes.contains(&("logo.png".to_owned(), Outcome::Blocked)),
        "{outcomes:?}"
    );
    assert!(
        outcomes.contains(&("data".to_owned(), Outcome::Fulfilled)),
        "{outcomes:?}"
    );
    assert!(
        outcomes.contains(&("echo".to_owned(), Outcome::Modified)),
        "{outcomes:?}"
    );

    // Stopped: the real network answers again.
    interception.stop().await.unwrap();
    page.goto(format!("{base}/")).await.unwrap();
    page.wait_for_function("document.title.startsWith('{')")
        .await
        .unwrap();
    assert_eq!(page.title().await.unwrap(), r#"{"real":true}"#);
    let loaded: bool = page
        .evaluate_as("document.getElementById('pic').naturalWidth === 1")
        .await
        .unwrap();
    assert!(loaded, "without interception the image loads");

    browser.close().await.unwrap();
}
