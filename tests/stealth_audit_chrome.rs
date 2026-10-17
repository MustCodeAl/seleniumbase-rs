//! The stealth layer audited against a real Chrome.
//!
//! Each test asks the page what a detection script would ask, once with a
//! fingerprint applied through the Pure CDP engine and, where the browser itself
//! is the reference, once without. The questions are about what the layer must
//! leave alone as much as about what it changes:
//!
//! - automation is not announced: `navigator.webdriver` reads `false`, no
//!   `cdc_` marker is left, and the Runtime domain is off, which a page can
//!   test by logging an object whose getter fires only when a client listens;
//! - what is replaced still looks native: names, lengths and `[native code]`;
//! - WebGL, canvas, the screen and Client Hints agree with the profile;
//! - `userAgentData` exists only where Chrome has it, in a secure context;
//! - `navigator.plugins`, `mimeTypes`, `pdfViewerEnabled` and
//!   `navigator.connection` are the browser's own.
//!
//! They need a Chrome or Chromium, so they are ignored by default. Run them one
//! at a time, since each launches a browser:
//!
//! ```text
//! cargo test --features "mcp-server test-util" --test stealth_audit_chrome \
//!     -- --ignored --test-threads=1
//! ```

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::{Duration, Instant};

use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, Page};
use seleniumbase_rs::stealth::fingerprint::{ConnectionProfile, Fingerprint};
use seleniumbase_rs::stealth::providers::{
    builtin, session_registry, EvasionConfig, EvasionContext, EvasionRegistry,
};
use seleniumbase_rs::OsType;
use serde_json::Value;

/// Seed of the persona every test applies.
const SEED: u64 = 20_261_011;

/// A name that resolves to the loopback server but is not a secure context.
const INSECURE_HOST: &str = "insecure.test";

const FIXTURE: &str = "<!doctype html><html><head><title>Audit</title></head>\
                       <body><h1 id=\"title\">fixture</h1></body></html>";

/// Serves the fixture for any request on a loopback port.
fn serve() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let port = listener.local_addr().expect("bound address").port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buffer = [0_u8; 2048];
            let _ = stream.read(&mut buffer);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n{FIXTURE}",
                FIXTURE.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    port
}

/// A macOS persona: the only OS whose values this project measures.
fn persona() -> Fingerprint {
    Fingerprint::randomized(OsType::Macos, SEED)
}

struct Lab {
    browser: Browser,
    page: Page,
    port: u16,
}

/// A headless Chrome, with `fingerprint` applied to every tab if there is one,
/// on the fixture page.
async fn lab(fingerprint: Option<&Fingerprint>) -> Lab {
    let port = serve();
    let mut builder = LaunchOptions::builder()
        .headless(true)
        .no_sandbox(std::env::var_os("CI").is_some())
        .arg(format!(
            "--host-resolver-rules=MAP {INSECURE_HOST} 127.0.0.1"
        ));
    if let Some(fingerprint) = fingerprint {
        builder = builder.fingerprint(fingerprint);
    }
    let browser = Browser::launch(builder.build().expect("valid launch options"))
        .await
        .expect("Chrome starts");
    let page = browser
        .default_page()
        .await
        .expect("a page")
        .with_timeout(Duration::from_secs(10));
    page.goto(format!("http://127.0.0.1:{port}/"))
        .await
        .expect("the fixture loads");
    Lab {
        browser,
        page,
        port,
    }
}

impl Lab {
    async fn eval(&self, js: &str) -> Value {
        self.page
            .evaluate(js)
            .await
            .unwrap_or_else(|error| panic!("script failed: {error}\n{js}"))
    }

    async fn close(self) {
        self.browser.close().await.expect("Chrome closes");
    }
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn automation_is_not_announced() {
    let fingerprint = persona();
    let lab = lab(Some(&fingerprint)).await;

    let seen = lab
        .eval(
            r##"(() => ({
                webdriver: navigator.webdriver,
                onPrototype: 'webdriver' in navigator,
                own: Object.getOwnPropertyNames(navigator),
                markers: Object.getOwnPropertyNames(window)
                    .concat(Object.getOwnPropertyNames(document))
                    .filter((k) => /^\$?cdc_|__webdriver|__selenium|__driver|domAutomation|callPhantom|_phantom/.test(k)),
            }))()"##,
        )
        .await;

    assert_eq!(seen["webdriver"], false, "navigator.webdriver");
    assert_eq!(
        seen["onPrototype"], true,
        "the property is still there, as in Chrome"
    );
    let own: Vec<_> = seen["own"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(
        !own.contains(&"webdriver"),
        "an own property would be a hook: {own:?}"
    );
    assert_eq!(seen["markers"], serde_json::json!([]), "automation markers");
    lab.close().await;
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn the_runtime_domain_is_off_so_a_logged_error_is_never_read() {
    let fingerprint = persona();
    let lab = lab(Some(&fingerprint)).await;

    // With `Runtime.enable` the browser serialises what is logged for the
    // client and reads the error's `stack`; without it, nothing reads it.
    let read = lab
        .eval(
            r##"(() => {
                let read = false;
                const error = new Error('probe');
                Object.defineProperty(error, 'stack', { get() { read = true; return ''; } });
                console.debug(error);
                return read;
            })()"##,
        )
        .await;

    assert_eq!(
        read, false,
        "the page can see that a client listens to the console"
    );
    lab.close().await;
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn patched_getters_and_methods_print_native_code() {
    let fingerprint = persona();
    let lab = lab(Some(&fingerprint)).await;

    let seen = lab
        .eval(
            r##"(() => {
                const print = (f) => Function.prototype.toString.call(f);
                const getters = {};
                for (const name of ['userAgent', 'platform', 'hardwareConcurrency', 'deviceMemory',
                                    'languages', 'webdriver', 'vendor', 'maxTouchPoints']) {
                    const d = Object.getOwnPropertyDescriptor(Navigator.prototype, name);
                    getters[name] = d && d.get ? [d.get.name, print(d.get), d.get.length, 'prototype' in d.get] : null;
                }
                return {
                    getters,
                    methods: {
                        toString: print(Function.prototype.toString),
                        query: print(navigator.permissions.query),
                        getParameter: print(WebGLRenderingContext.prototype.getParameter),
                        toDataURL: print(HTMLCanvasElement.prototype.toDataURL),
                        getImageData: print(CanvasRenderingContext2D.prototype.getImageData),
                    },
                };
            })()"##,
        )
        .await;

    let mut audited = 0;
    for (name, getter) in seen["getters"].as_object().unwrap() {
        if getter.is_null() {
            continue;
        }
        audited += 1;
        assert_eq!(
            getter[0],
            format!("get {name}"),
            "{name}: the getter's name"
        );
        assert_eq!(
            getter[1],
            format!("function get {name}() {{ [native code] }}"),
            "{name}: the getter's source"
        );
        assert_eq!(getter[2], 0, "{name}: the getter's length");
        assert_eq!(getter[3], false, "{name}: a getter has no prototype");
    }
    assert!(audited >= 5, "only {audited} getters were found");
    for (name, source) in seen["methods"].as_object().unwrap() {
        assert_eq!(
            source.as_str().unwrap(),
            format!("function {name}() {{ [native code] }}"),
            "{name}"
        );
    }
    lab.close().await;
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn webgl_and_canvas_agree_with_the_profile() {
    let fingerprint = persona();
    let lab = lab(Some(&fingerprint)).await;

    let seen = lab
        .eval(
            r##"(() => {
                const info = (context) => {
                    const ext = context && context.getExtension('WEBGL_debug_renderer_info');
                    return ext ? [context.getParameter(ext.UNMASKED_VENDOR_WEBGL),
                                  context.getParameter(ext.UNMASKED_RENDERER_WEBGL)] : null;
                };
                const art = document.createElement('canvas');
                art.width = 120; art.height = 40;
                const ctx = art.getContext('2d');
                ctx.fillStyle = '#f60'; ctx.fillRect(10, 10, 80, 20);
                ctx.fillStyle = '#069'; ctx.font = '14px Arial'; ctx.fillText('audit', 12, 28);
                const full = ctx.getImageData(0, 0, 120, 40).data;
                const part = ctx.getImageData(10, 10, 20, 20).data;
                let sameBytes = true;
                for (let row = 0; row < 20; row++) {
                    for (let col = 0; col < 20; col++) {
                        for (let k = 0; k < 4; k++) {
                            if (full[((10 + row) * 120 + 10 + col) * 4 + k] !== part[(row * 20 + col) * 4 + k]) { sameBytes = false; }
                        }
                    }
                }
                return {
                    webgl: info(document.createElement('canvas').getContext('webgl')),
                    webgl2: info(document.createElement('canvas').getContext('webgl2')),
                    twoReadsAgree: art.toDataURL() === art.toDataURL(),
                    aPartIsTheWhole: sameBytes,
                };
            })()"##,
        )
        .await;

    for key in ["webgl", "webgl2"] {
        let pair = seen[key]
            .as_array()
            .unwrap_or_else(|| panic!("{key} gave no renderer info"));
        if let Some(vendor) = &fingerprint.webgl_vendor {
            assert_eq!(pair[0].as_str(), Some(vendor.as_str()), "{key} vendor");
        }
        if let Some(renderer) = &fingerprint.webgl_renderer {
            assert_eq!(pair[1].as_str(), Some(renderer.as_str()), "{key} renderer");
        }
    }
    assert_eq!(
        seen["twoReadsAgree"], true,
        "canvas noise is stable between reads"
    );
    assert_eq!(
        seen["aPartIsTheWhole"], true,
        "a sub-rectangle carries the same noise"
    );
    lab.close().await;
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn user_agent_data_exists_only_in_a_secure_context() {
    let fingerprint = persona();
    let hints = fingerprint
        .client_hints
        .clone()
        .expect("a randomized persona carries Client Hints");
    let lab = lab(Some(&fingerprint)).await;
    let probe = r##"(() => ({
        secure: window.isSecureContext,
        present: 'userAgentData' in navigator,
        platform: navigator.userAgentData ? navigator.userAgentData.platform : null,
        mobile: navigator.userAgentData ? navigator.userAgentData.mobile : null,
        brands: navigator.userAgentData ? navigator.userAgentData.brands.length : 0,
    }))()"##;

    let secure = lab.eval(probe).await;
    lab.page
        .goto(format!("http://{INSECURE_HOST}:{}/", lab.port))
        .await
        .expect("the insecure origin loads");
    let insecure = lab.eval(probe).await;

    assert_eq!(secure["secure"], true, "127.0.0.1 is a secure context");
    assert_eq!(secure["present"], true);
    assert_eq!(secure["platform"].as_str(), Some(hints.platform.as_str()));
    assert_eq!(secure["mobile"], hints.mobile);
    assert!(secure["brands"].as_u64().unwrap() > 0);
    assert_eq!(
        insecure["secure"], false,
        "{INSECURE_HOST} over http is not secure"
    );
    assert_eq!(
        insecure["present"], false,
        "Chrome has no userAgentData on a plain http page, so neither may the persona"
    );
    lab.close().await;
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn the_screen_is_the_profiles_and_the_viewport_is_the_windows() {
    let fingerprint = persona();
    let (width, height) = (
        i64::from(fingerprint.screen_width.expect("a screen width")),
        i64::from(fingerprint.screen_height.expect("a screen height")),
    );
    let lab = lab(Some(&fingerprint)).await;
    let probe = r##"(() => ({
        width: screen.width, height: screen.height,
        availWidth: screen.availWidth, availHeight: screen.availHeight,
        ratio: window.devicePixelRatio,
        innerWidth: window.innerWidth, innerHeight: window.innerHeight,
        outerWidth: window.outerWidth, outerHeight: window.outerHeight,
    }))()"##;

    let before = lab.eval(probe).await;
    lab.page
        .window()
        .set_bounds(0, 0, 700, 500)
        .await
        .expect("the window is resized");
    let started = Instant::now();
    let after = loop {
        let seen = lab.eval(probe).await;
        if seen["innerWidth"].as_i64().is_some_and(|w| w <= 700)
            || started.elapsed() > Duration::from_secs(5)
        {
            break seen;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    };

    for seen in [&before, &after] {
        assert_eq!(seen["width"], width, "screen.width: {seen}");
        assert_eq!(seen["height"], height, "screen.height: {seen}");
        assert!(
            seen["availWidth"].as_i64().unwrap() <= width,
            "availWidth fits the screen: {seen}"
        );
        assert!(
            seen["availHeight"].as_i64().unwrap() <= height,
            "availHeight fits the screen: {seen}"
        );
        assert_eq!(
            seen["ratio"].as_f64(),
            Some(fingerprint.pixel_ratio.unwrap_or(1.0)),
            "devicePixelRatio: {seen}"
        );
        assert!(
            seen["outerWidth"].as_i64() >= seen["innerWidth"].as_i64(),
            "a window is never narrower than its page: {seen}"
        );
        assert!(
            seen["outerHeight"].as_i64() >= seen["innerHeight"].as_i64(),
            "a window is never shorter than its page: {seen}"
        );
    }
    assert!(
        after["innerWidth"].as_i64().unwrap() <= 700,
        "the viewport follows the window, it is not pinned to the screen: {after}"
    );
    lab.close().await;
}

/// What a page can see of the lists, flags and reports that Chrome answers for
/// itself, in a form that two browsers can be compared on.
const BROWSER_OWNED_PROBE: &str = r##"(() => {
    const own = (o) => Object.getOwnPropertyNames(o).sort();
    const getter = (name) => {
        const d = Object.getOwnPropertyDescriptor(Navigator.prototype, name);
        return d && d.get ? [d.get.name, Function.prototype.toString.call(d.get)] : null;
    };
    return {
        plugins: {
            getter: getter('plugins'),
            names: Array.from(navigator.plugins, (p) => p.name),
            arrayOwn: own(navigator.plugins),
            firstOwn: navigator.plugins.length ? own(navigator.plugins[0]) : null,
            firstIsPlugin: navigator.plugins.length ? navigator.plugins[0] instanceof Plugin : null,
            sameObjectTwice: navigator.plugins === navigator.plugins,
        },
        mimeTypes: {
            getter: getter('mimeTypes'),
            types: Array.from(navigator.mimeTypes, (m) => m.type),
            arrayOwn: own(navigator.mimeTypes),
            firstOwn: navigator.mimeTypes.length ? own(navigator.mimeTypes[0]) : null,
        },
        pdfViewerEnabled: [navigator.pdfViewerEnabled, getter('pdfViewerEnabled')],
        connection: {
            getter: getter('connection'),
            kind: Object.prototype.toString.call(navigator.connection),
            sameObjectTwice: navigator.connection === navigator.connection,
            own: navigator.connection ? own(navigator.connection) : null,
            protoOwn: navigator.connection ? own(Object.getPrototypeOf(navigator.connection)) : null,
        },
    };
})()"##;

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn plugins_mime_types_the_pdf_flag_and_connection_are_the_browsers_own() {
    let baseline = {
        let lab = lab(None).await;
        let seen = lab.eval(BROWSER_OWNED_PROBE).await;
        lab.close().await;
        seen
    };

    // A persona whose network report is far from anything a loopback Chrome says.
    let mut fingerprint = persona();
    fingerprint.connection = Some(ConnectionProfile {
        effective_type: "slow-2g".to_owned(),
        rtt: 2_777,
        downlink: 0.05,
        save_data: true,
    });
    let lab = lab(Some(&fingerprint)).await;
    let with_persona = lab.eval(BROWSER_OWNED_PROBE).await;
    let report = lab
        .eval(
            r##"(() => navigator.connection
                ? { type: navigator.connection.effectiveType, rtt: navigator.connection.rtt,
                    saveData: navigator.connection.saveData }
                : null)()"##,
        )
        .await;
    lab.close().await;

    for key in ["plugins", "mimeTypes", "pdfViewerEnabled", "connection"] {
        assert_eq!(
            baseline[key], with_persona[key],
            "{key} differs from a Chrome without a fingerprint"
        );
    }
    if !report.is_null() {
        assert_ne!(
            report["rtt"], 2_777,
            "the persona's round-trip time reached the page"
        );
        assert_ne!(
            report["type"], "slow-2g",
            "the persona's connection type reached the page"
        );
        assert_ne!(
            report["saveData"], true,
            "the persona's data saver flag reached the page"
        );
    }
}

/// A registry of the one provider named, with the `toString` masking it relies
/// on.
fn only(name: &str) -> EvasionRegistry {
    let mut registry = EvasionRegistry::empty();
    for provider in builtin::all() {
        if provider.name() == "native_tostring" || provider.name() == name {
            registry.register_provider(provider);
        }
    }
    registry
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn every_session_provider_runs_in_chrome_without_throwing() {
    let mut fingerprint = persona();
    // A generated identity has an audio seed; the media devices and the fonts
    // are values that turn their providers on.
    fingerprint.audio_inputs = Some(1);
    fingerprint.audio_outputs = Some(1);
    fingerprint.video_inputs = Some(1);
    fingerprint.fonts = vec!["Arial".to_owned()];
    let lab = lab(None).await;
    // A script error must reach the test, not be swallowed by the snippet.
    let config = EvasionConfig {
        isolate_failures: false,
        annotate: false,
    };
    let mut ran = Vec::new();

    for provider in session_registry(&fingerprint).ordered() {
        let name = provider.name();
        if name == "native_tostring" || !provider.applies(&fingerprint) {
            continue;
        }
        // A fresh document for each, so the providers cannot lean on each other.
        let page = lab
            .browser
            .new_page(Some(format!("http://127.0.0.1:{}/", lab.port)))
            .await
            .expect("a fresh tab")
            .with_timeout(Duration::from_secs(10));
        let script =
            only(name).bootstrap(&EvasionContext::with_config(&fingerprint, config.clone()));
        let outcome = page.evaluate(&script).await;
        page.close().await.expect("the tab closes");
        outcome.unwrap_or_else(|error| panic!("provider {name} threw: {error}\n{script}"));
        ran.push(name.to_owned());
    }

    assert!(ran.len() >= 15, "only {} providers ran: {ran:?}", ran.len());
    for omitted in ["plugins", "connection"] {
        assert!(
            !ran.contains(&omitted.to_owned()),
            "{omitted} is the browser's to answer"
        );
    }
    lab.close().await;
}
