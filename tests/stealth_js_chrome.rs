//! The page scripts in `stealth::js`, and the proxy-auth extension's
//! background script, against a real Chrome.
//!
//! Until these tests existed, none of this JavaScript had ever been parsed or
//! run. Each test loads a small page through the Pure CDP engine and runs the
//! script the way WebDriver's Execute Script does, as a function body, then
//! asks the page what happened.
//!
//! They need a Chrome or Chromium, so they are ignored by default. Run them one
//! at a time, since each starts its own browser:
//!
//! ```text
//! cargo test --test stealth_js_chrome -- --ignored --test-threads=1
//! ```

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use base64::Engine as _;
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, Page};
use seleniumbase_rs::stealth::js::active_css_js::get_active_css_js;
use seleniumbase_rs::utils::extensions::{
    extract_extension, get_ad_block_bytes, get_disable_csp_bytes, get_recorder_bytes,
    get_sbase_ext_bytes, proxy_auth_background_js, write_proxy_auth_extension,
};
use serde_json::{json, Value};

/// A form with the kinds of control the recorder meets.
const FORM_PAGE: &str = r##"<!doctype html><html><head><title>Form</title></head><body>
<form id="login" onsubmit="return false">
  <input id="email" type="text">
  <input name="q" type="search">
  <input id="pw" type="password">
  <textarea></textarea>
  <select id="color"><option value="r">Red</option><option value="g">Green</option></select>
  <input id="agree" type="checkbox">
  <button type="button" id="go"><span id="label">Go</span></button>
  <button type="submit" id="send">Send</button>
</form>
</body></html>"##;

/// Elements whose selectors are awkward: no ids, repeated ids, ids and
/// attribute values that need escaping, repeated text.
const MESSY_PAGE: &str = r##"<!doctype html><html><head><title>Messy</title></head><body>
<div class="wrap"><div><div>
  <p tabindex="0" data-probe="deep-1"></p>
  <p tabindex="0" data-probe="deep-2"></p>
</div></div></div>
<input data-probe="named" name="q">
<input data-probe="dup-1" id="dup"><input data-probe="dup-2" id="dup">
<input data-probe="dotted" id="a.b">
<input data-probe="digit" id="1st">
<input data-probe="spaced" id="has space">
<input data-probe="quoted" name="say &quot;hi&quot; it's">
<input data-probe="backslash" name="c:\temp">
<a data-probe="link-text" tabindex="0">Pricing</a>
<a data-probe="twice-1" tabindex="0">Docs</a><a data-probe="twice-2" tabindex="0">Docs</a>
<ul><li data-probe="item-1" tabindex="0">One</li><li data-probe="item-2" tabindex="0">Two</li></ul>
<button data-probe="icon-button"><span>Save</span></button>
<a data-probe="href-link" href="/one">Sign in</a>
<div id="outer.x"><span data-probe="inside-odd-id" tabindex="0">x</span></div>
</body></html>"##;

/// Serves `FORM_PAGE` for `/`, `MESSY_PAGE` for `/messy` and a copy of the form
/// for `/next`, on a loopback port.
fn serve() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let port = listener.local_addr().expect("bound address").port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buffer = [0_u8; 2048];
            let read = stream.read(&mut buffer).unwrap_or(0);
            let request = String::from_utf8_lossy(&buffer[..read]);
            let body = if request.starts_with("GET /messy") {
                MESSY_PAGE
            } else {
                FORM_PAGE
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    port
}

struct Lab {
    browser: Browser,
    page: Page,
}

async fn lab(path: &str) -> Lab {
    let port = serve();
    let options = LaunchOptions::builder()
        .headless(true)
        .no_sandbox(std::env::var_os("CI").is_some())
        .window_size(1000, 800)
        .build()
        .expect("valid launch options");
    let browser = Browser::launch(options).await.expect("Chrome starts");
    let page = browser.default_page().await.expect("a page");
    page.goto(format!("http://127.0.0.1:{port}{path}"))
        .await
        .expect("the fixture loads");
    let page = page.with_timeout(Duration::from_secs(10));
    // The engine installs its own helpers on first use.
    page.evaluate("1").await.expect("the page runs script");
    Lab { browser, page }
}

impl Lab {
    /// Runs a script written as a function body, as Execute Script does.
    async fn run(&self, script: &str) -> Value {
        self.page
            .evaluate(format!("(function () {{\n{script}\n}})()"))
            .await
            .unwrap_or_else(|error| panic!("script failed: {error}"))
    }

    async fn close(self) {
        self.browser.close().await.expect("Chrome closes");
    }
}

/// Focuses the element matching `selector`, then runs the active-element script.
async fn active_selector(lab: &Lab, selector: &str) -> String {
    let focus = format!("document.querySelector({}).focus(); 1", json!(selector));
    lab.page.evaluate(focus).await.expect("focus the element");
    lab.run(get_active_css_js())
        .await
        .as_str()
        .expect("the script returns a string")
        .to_owned()
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn active_element_selector_names_the_focused_element() {
    let lab = lab("/").await;

    assert_eq!(active_selector(&lab, "#email").await, "input#email");
    assert_eq!(active_selector(&lab, "[name=q]").await, "input[name=\"q\"]");
    // The only text area on the page needs no more than its tag.
    assert_eq!(active_selector(&lab, "textarea").await, "textarea");
    assert_eq!(active_selector(&lab, "#go").await, "button#go");

    // Nothing focused is the page itself.
    lab.page
        .evaluate("document.activeElement.blur(); 1")
        .await
        .expect("blur");
    assert_eq!(lab.run(get_active_css_js()).await, json!("body"));

    lab.close().await;
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn active_element_selector_always_finds_its_element() {
    let lab = lab("/messy").await;
    let script = serde_json::to_string(get_active_css_js()).expect("a string");
    let probe = format!(
        r##"(() => {{
            const script = {script};
            const found = [];
            for (const el of document.querySelectorAll('[data-probe]')) {{
                el.focus();
                const selector = new Function(script)();
                found.push({{
                    probe: el.dataset.probe,
                    focused: document.activeElement === el,
                    selector,
                    // `:contains()` is SeleniumBase's own pseudo-class.
                    finds: selector.includes(':contains(')
                        || document.querySelector(selector) === el,
                }});
            }}
            return found;
        }})()"##
    );
    let results = lab.page.evaluate(probe).await.expect("the probe runs");
    let results = results.as_array().expect("a list");
    assert_eq!(results.len(), 18, "every probe element is checked");
    for result in results {
        assert_eq!(result["focused"], json!(true), "{result}");
        assert_eq!(result["finds"], json!(true), "{result}");
    }

    let selector_of = |probe: &str| {
        results
            .iter()
            .find(|result| result["probe"] == probe)
            .and_then(|result| result["selector"].as_str())
            .unwrap_or_default()
            .to_owned()
    };
    // A name that holds quotes and a backslash is escaped, not spliced in.
    assert_eq!(selector_of("quoted"), "input[name=\"say \\\"hi\\\" it's\"]");
    assert_eq!(selector_of("backslash"), "input[name=\"c:\\\\temp\"]");
    // An id that is not a plain identifier is matched as an attribute, and
    // it is the element's own id that counts, not an ancestor's.
    assert_eq!(selector_of("dotted"), "input[id=\"a.b\"]");
    assert_eq!(selector_of("spaced"), "input[id=\"has space\"]");
    assert_eq!(selector_of("inside-odd-id"), "div[id=\"outer.x\"] span");
    // The second of two elements with one id cannot be named by that id.
    assert_eq!(selector_of("dup-1"), "input#dup");
    assert_ne!(selector_of("dup-2"), "input#dup");
    assert_eq!(selector_of("href-link"), "a[href=\"/one\"]");
    // Text that appears once names a link; text that appears twice cannot.
    assert_eq!(selector_of("link-text"), "a:contains(\"Pricing\")");
    assert!(!selector_of("twice-1").contains(":contains("));
    assert_eq!(selector_of("icon-button"), "button:contains(\"Save\")");

    lab.close().await;
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn proxy_auth_script_answers_only_the_proxy_with_the_exact_credentials() {
    let lab = lab("/").await;
    let password = "p\"w\\d\n</script>';alert(1);//\u{2028}";
    let script =
        serde_json::to_string(&proxy_auth_background_js("us\"er", password)).expect("a string");
    // Run the extension's script against a stand-in for Chrome's `chrome` API
    // and call the listener it registers the way Chrome would.
    let probe = format!(
        r##"(() => {{
            const listeners = {{}};
            const registrar = (name) => ({{
                addListener(callback, filter, extra) {{
                    listeners[name] = {{ callback, filter, extra }};
                }},
            }});
            const chrome = {{
                webRequest: {{
                    onAuthRequired: registrar('auth'),
                    onCompleted: registrar('completed'),
                    onErrorOccurred: registrar('failed'),
                }},
            }};
            new Function('chrome', {script})(chrome);
            const ask = (details) => listeners.auth.callback(details);
            const proxy = ask({{ isProxy: true, requestId: '1' }});
            const site = ask({{ isProxy: false, requestId: '2' }});
            const again = ask({{ isProxy: true, requestId: '1' }});
            listeners.completed.callback({{ requestId: '1' }});
            const afterwards = ask({{ isProxy: true, requestId: '1' }});
            listeners.failed.callback({{ requestId: '3' }});
            return {{
                extra: listeners.auth.extra,
                filter: listeners.auth.filter,
                proxy, site, again, afterwards,
            }};
        }})()"##
    );
    let seen = lab.page.evaluate(probe).await.expect("the probe runs");

    assert_eq!(seen["extra"], json!(["blocking"]));
    assert_eq!(seen["filter"], json!({"urls": ["<all_urls>"]}));
    let credentials = json!({"authCredentials": {"username": "us\"er", "password": password}});
    assert_eq!(seen["proxy"], credentials);
    // A web site's own login prompt is not for the proxy's password.
    assert_eq!(seen["site"], json!({}));
    // The same request asking a second time means the proxy said no.
    assert_eq!(seen["again"], json!({"cancel": true}));
    // Once the request is over, a new request with the same id starts afresh.
    assert_eq!(seen["afterwards"], credentials);

    lab.close().await;
}

/// A proxy that wants `Proxy-Authorization`, and a site behind it that wants
/// its own login. It records the credentials it was sent.
struct FakeProxy {
    port: u16,
    proxy_auth: Arc<Mutex<Vec<String>>>,
    site_auth: Arc<Mutex<Vec<String>>>,
}

fn fake_proxy() -> FakeProxy {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let port = listener.local_addr().expect("bound address").port();
    let proxy_auth = Arc::new(Mutex::new(Vec::new()));
    let site_auth = Arc::new(Mutex::new(Vec::new()));
    let (proxy_seen, site_seen) = (Arc::clone(&proxy_auth), Arc::clone(&site_auth));
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buffer = [0_u8; 4096];
            let read = stream.read(&mut buffer).unwrap_or(0);
            let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
            let header = |name: &str| {
                request.lines().find_map(|line| {
                    let (key, value) = line.split_once(':')?;
                    key.eq_ignore_ascii_case(name)
                        .then(|| value.trim().to_owned())
                })
            };
            let response = match header("proxy-authorization") {
                None => "HTTP/1.1 407 Proxy Authentication Required\r\n\
                         Proxy-Authenticate: Basic realm=\"sb-proxy\"\r\n\
                         Content-Length: 0\r\nConnection: close\r\n\r\n"
                    .to_owned(),
                Some(credentials) => {
                    proxy_seen.lock().expect("lock").push(credentials);
                    if let Some(login) = header("authorization") {
                        site_seen.lock().expect("lock").push(login);
                    }
                    if request.starts_with("GET http://sb-site.example/private") {
                        // The site asks for a login of its own.
                        "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"site\"\r\n\
                         Content-Type: text/plain\r\nContent-Length: 12\r\n\
                         Connection: close\r\n\r\nsite says no"
                            .to_owned()
                    } else {
                        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 17\r\n\
                         Connection: close\r\n\r\nthrough the proxy"
                            .to_owned()
                    }
                }
            };
            let _ = stream.write_all(response.as_bytes());
        }
    });
    FakeProxy {
        port,
        proxy_auth,
        site_auth,
    }
}

#[tokio::test]
#[ignore = "launches a real Chrome and loads an extension into it"]
async fn proxy_auth_extension_logs_in_to_the_proxy_in_a_real_chrome() {
    // Chrome 137 and later ignore `--load-extension` in branded builds, so the
    // extension is loaded over the protocol, which `--enable-unsafe-extension-debugging`
    // allows.
    let proxy = fake_proxy();
    let extension = tempfile::tempdir().expect("a temporary folder");
    write_proxy_auth_extension(extension.path(), "alice", "s3cret").expect("writes");

    let options = LaunchOptions::builder()
        .headless(true)
        .no_sandbox(std::env::var_os("CI").is_some())
        .arg(format!("--proxy-server=127.0.0.1:{}", proxy.port))
        .arg("--enable-unsafe-extension-debugging")
        .build()
        .expect("valid launch options");
    let browser = Browser::launch(options).await.expect("Chrome starts");
    browser
        .execute(
            "Extensions.loadUnpacked",
            json!({ "path": extension.path().to_string_lossy() }),
        )
        .await
        .expect("Chrome loads the extension");
    let page = browser
        .default_page()
        .await
        .expect("a page")
        .with_timeout(Duration::from_secs(15));

    // The extension's worker starts a moment after it is loaded, and a
    // challenge that arrives first is refused, so try again.
    let mut text = Value::Null;
    for _ in 0..10 {
        let _ = page.goto("http://sb-proxy-test.example/").await;
        text = page
            .evaluate("document.body ? document.body.innerText : ''")
            .await
            .unwrap_or(Value::Null);
        if text == json!("through the proxy") {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    assert_eq!(
        text,
        json!("through the proxy"),
        "the proxy never let us through"
    );
    let expected = format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode("alice:s3cret")
    );
    assert!(
        proxy.proxy_auth.lock().expect("lock").contains(&expected),
        "the proxy was sent {:?}",
        proxy.proxy_auth.lock().expect("lock")
    );

    // A site that asks for its own login does not get the proxy's.
    let _ = page.goto("http://sb-site.example/private").await;
    assert!(
        proxy.site_auth.lock().expect("lock").is_empty(),
        "the site was sent {:?}",
        proxy.site_auth.lock().expect("lock")
    );

    browser.close().await.expect("Chrome closes");
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn every_script_in_the_bundled_extensions_parses() {
    let lab = lab("/").await;
    let mut checked = 0;
    for (name, archive) in [
        ("ad_block", get_ad_block_bytes()),
        ("disable_csp", get_disable_csp_bytes()),
        ("recorder", get_recorder_bytes()),
        ("sbase_ext", get_sbase_ext_bytes()),
    ] {
        let dir = tempfile::tempdir().expect("a temporary folder");
        extract_extension(archive, dir.path()).expect("unpacks");
        for entry in std::fs::read_dir(dir.path()).expect("read the folder") {
            let path = entry.expect("an entry").path();
            if path.extension().is_none_or(|extension| extension != "js") {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("the script is UTF-8");
            let source = serde_json::to_string(&source).expect("a string");
            let outcome = lab
                .page
                .evaluate(format!(
                    "(() => {{ try {{ new Function({source}); return null; }} \
                     catch (error) {{ return String(error); }} }})()"
                ))
                .await
                .expect("the probe runs");
            assert_eq!(outcome, Value::Null, "{name}: {}", path.display());
            checked += 1;
        }
    }
    // Seven today: three in disable_csp, two each in recorder and sbase_ext.
    assert!(checked >= 7, "only {checked} scripts were found");

    lab.close().await;
}
