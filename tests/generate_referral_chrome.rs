//! A real-browser check of `BaseCase::generate_referral`, `generate_traffic` and
//! their chains.
//!
//! Python SeleniumBase opens a start page, builds a link to the destination in
//! it and clicks the link, so the destination receives a real `Referer`
//! header. A `?ref=` query parameter, which this crate used to add instead,
//! sends none. The site below runs on the loopback interface and records the
//! `Referer` of every request, so the test can say what the browser sent.
//!
//! This test is ignored by default. Run it locally with Chrome and a matching
//! chromedriver installed:
//!
//! ```bash
//! cargo test --test generate_referral_chrome -- --ignored --test-threads=1
//! ```
//!
//! Set `SB_WEBDRIVER_URL` to use a remote grid instead of the chromedriver that
//! `BaseCase` starts on port 9515.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use seleniumbase_rs::{BaseCase, BrowserConfig};

/// Serializes browser sessions so parallel tests do not race to bind
/// chromedriver's default port when auto-starting the driver.
static BROWSER_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// The path and `Referer` header of every request the site received.
type Requests = Arc<Mutex<Vec<(String, Option<String>)>>>;

fn test_config() -> BrowserConfig {
    let mut config = BrowserConfig {
        headless: true,
        auto_start_driver: true,
        ..BrowserConfig::default()
    };
    if let Ok(url) = std::env::var("SB_WEBDRIVER_URL") {
        config.webdriver_url = url;
        config.auto_start_driver = false;
    }
    config
}

/// Answers one connection: records the path and the `Referer`, and serves a
/// page with a link `#more` to `/more`.
fn answer(mut stream: TcpStream, requests: &Mutex<Vec<(String, Option<String>)>>) {
    let mut buffer = [0_u8; 8192];
    let read = stream.read(&mut buffer).unwrap_or(0);
    let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .unwrap_or("/")
        .to_owned();
    let referer = request.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("referer")
            .then(|| value.trim().to_owned())
    });
    requests
        .lock()
        .expect("request log")
        .push((path.clone(), referer));
    let body = format!(
        "<!doctype html><html><head><title>{path}</title></head>\
         <body><p>{path}</p><a id=\"more\" href=\"/more\">more</a></body></html>"
    );
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

/// Starts the site and returns its `http://127.0.0.1:port` base and its log.
fn serve() -> (String, Requests) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let base = format!("http://{}", listener.local_addr().expect("bound address"));
    let requests = Requests::default();
    let log = Arc::clone(&requests);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            // A thread each: Chrome opens idle connections ahead of time, and
            // reading from one of those would stall every later request.
            let log = Arc::clone(&log);
            std::thread::spawn(move || answer(stream, &log));
        }
    });
    (base, requests)
}

/// The `Referer` of each request for `path`, oldest first.
fn referers(requests: &Requests, path: &str) -> Vec<Option<String>> {
    requests
        .lock()
        .expect("request log")
        .iter()
        .filter(|(requested, _)| requested == path)
        .map(|(_, referer)| referer.clone())
        .collect()
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn the_destination_receives_the_page_the_link_was_clicked_on_as_its_referer() {
    let _guard = BROWSER_LOCK.lock().await;
    let (base, requests) = serve();
    let mut sb = BaseCase::new(test_config())
        .await
        .expect("connect to browser");
    let start = format!("{base}/start");
    let landing = format!("{base}/landing");

    // One referral: start page, then the destination, which saw the start page.
    sb.generate_referral(&start, &landing, None)
        .await
        .expect("the referral is generated");
    assert_eq!(sb.get_current_url().await.unwrap(), landing);
    assert_eq!(referers(&requests, "/landing"), [Some(start.clone())]);
    assert_eq!(
        referers(&requests, "/start"),
        [None],
        "the start page was opened directly, not by a link"
    );

    // Traffic: the same again, twice more.
    sb.generate_traffic(&start, &landing, 2, None)
        .await
        .expect("the traffic is generated");
    assert_eq!(
        referers(&requests, "/landing"),
        [
            Some(start.clone()),
            Some(start.clone()),
            Some(start.clone())
        ]
    );

    // A selector is clicked on the destination page, so it sees a second hop.
    sb.generate_referral(&start, &landing, Some("#more"))
        .await
        .expect("the referral with a selector is generated");
    assert_eq!(referers(&requests, "/more"), [Some(landing.clone())]);
    assert_eq!(sb.get_current_url().await.unwrap(), format!("{base}/more"));

    // A chain starts from the page the browser is on, then each page links on
    // to the next.
    let (one, two) = (format!("{base}/one"), format!("{base}/two"));
    sb.open(&start).await.unwrap();
    sb.generate_referral_chain(&[&one, &two])
        .await
        .expect("the chain is generated");
    assert_eq!(referers(&requests, "/one"), [Some(start.clone())]);
    assert_eq!(referers(&requests, "/two"), [Some(one.clone())]);
    assert_eq!(sb.get_current_url().await.unwrap(), two);

    // A traffic chain does it again, from the last page of the one before.
    sb.generate_traffic_chain(&[&one, &two], 2)
        .await
        .expect("the traffic chain is generated");
    assert_eq!(
        referers(&requests, "/one"),
        [Some(start), Some(two.clone()), Some(two)],
        "each loop starts from the page the last one ended on"
    );

    sb.quit().await.expect("the browser closes");
}
