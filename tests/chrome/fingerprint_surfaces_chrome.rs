//! The surfaces a fingerprint reaches once the whole identity is applied the
//! way a user applies it, through [`LaunchOptions::builder`] and
//! `.fingerprint(..)`, read back from a real headless Chrome.
//!
//! [`fingerprint_consistency_chrome`](super::fingerprint_consistency_chrome)
//! asks a captured persona whether it agrees with itself; the tests here read
//! one surface each, in the window and where the window is not the only reader
//! (a dedicated worker). Pages are this module's own loopback server, never a
//! site on the internet.
//!
//! The tests need a Chrome, so they are ignored by default. Run them one at a
//! time with:
//!
//! ```text
//! SB_HEADLESS=true cargo test -p seleniumbase-rs --features "mcp-server test-util testing bdd" \
//!   --test chrome fingerprint_surfaces_chrome:: -- --ignored --test-threads=1 --nocapture
//! ```

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use seleniumbase_rs::evasion::fingerprint::{ColorGamut, ColorScheme, MediaFeatures};
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, Page};

use seleniumbase_rs::Fingerprint;
use serde_json::{json, Value};

const PAGE: &str = "<!doctype html><title>surfaces</title><p>surfaces</p>";

/// Serves `PAGE` on a loopback port: `127.0.0.1` is a secure context, the only
/// kind of page Chrome gives Client Hints, WebGPU and the storage quota.
fn serve() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let address = listener.local_addr().expect("bound address");
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buffer = [0_u8; 4096];
            let _ = stream.read(&mut buffer);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{PAGE}",
                PAGE.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    format!("http://{address}/")
}

/// Launches a headless Chrome as `persona` and loads the loopback page.
async fn open(persona: &Fingerprint) -> (Browser, Page) {
    let options = LaunchOptions::builder()
        .headless(true)
        .fingerprint(persona)
        .build()
        .expect("options");
    let browser = Browser::launch(options).await.expect("Chrome starts");
    let page = browser.default_page().await.expect("a tab");
    page.goto(&serve()).await.expect("the loopback page loads");
    (browser, page)
}

/// Evaluates `expression` in a dedicated worker made from a Blob, as
/// `capture.js` does, and returns what the worker posts back.
async fn in_worker(page: &Page, expression: &str) -> Value {
    let worker_source = format!(
        "self.onmessage = async () => {{ try {{ postMessage({{ ok: true, value: await (async () => ({expression}))() }}); }} catch (error) {{ postMessage({{ ok: false, error: String(error) }}); }} }};"
    );
    let source = serde_json::to_string(&worker_source).expect("the worker source is text");
    let script = format!(
        "new Promise((resolve, reject) => {{ \
            const url = URL.createObjectURL(new Blob([{source}], {{ type: 'text/javascript' }})); \
            const worker = new Worker(url); \
            const timer = setTimeout(() => {{ worker.terminate(); reject(new Error('the worker did not answer')); }}, 8000); \
            worker.onmessage = (event) => {{ clearTimeout(timer); worker.terminate(); resolve(event.data); }}; \
            worker.onerror = (event) => {{ clearTimeout(timer); worker.terminate(); reject(new Error(event.message)); }}; \
            worker.postMessage(0); \
        }})"
    );
    let answer = page
        .evaluate(script)
        .await
        .expect("the worker runs and answers");
    assert_eq!(
        answer["ok"],
        json!(true),
        "the worker threw on {expression}: {answer}"
    );
    answer["value"].clone()
}

#[tokio::test]
#[ignore = "requires a local Chrome"]
#[serial_test::serial(chrome)]
async fn the_core_count_reaches_the_window_and_a_worker() {
    let persona = Fingerprint::windows_desktop()
        .to_builder()
        .hardware_concurrency(3)
        .build()
        .expect("a valid fingerprint");
    let (browser, page) = open(&persona).await;

    let window = page
        .evaluate("navigator.hardwareConcurrency")
        .await
        .expect("the window answers");
    let worker = in_worker(&page, "navigator.hardwareConcurrency").await;
    browser.close().await.expect("Chrome closes");

    assert_eq!(window, json!(3), "the window reports the persona's cores");
    // Chrome applies `Emulation.setHardwareConcurrencyOverride` to the page and
    // not to its dedicated workers, so a worker still reports the host's count
    // (step 2 of the plan; `worker_vs_window` stays open for the worker lane).
    assert_ne!(
        worker,
        json!(3),
        "flips when Chrome applies the override to workers"
    );
}

#[tokio::test]
#[ignore = "requires a local Chrome"]
#[serial_test::serial(chrome)]
async fn a_desktop_persona_has_a_frame_around_its_viewport() {
    let (browser, page) = open(&Fingerprint::windows_desktop()).await;

    let geometry = page
        .evaluate(
            "({ innerWidth: window.innerWidth, innerHeight: window.innerHeight, \
               outerWidth: window.outerWidth, outerHeight: window.outerHeight, \
               availHeight: screen.availHeight, height: screen.height })",
        )
        .await
        .expect("the window answers");
    browser.close().await.expect("Chrome closes");

    let read = |key: &str| {
        geometry[key]
            .as_u64()
            .unwrap_or_else(|| panic!("{key} is a number in {geometry}"))
    };
    assert!(
        read("innerHeight") < read("outerHeight"),
        "a browser window has a tab strip and a toolbar above its viewport: {geometry}"
    );
    assert!(
        read("outerHeight") <= read("availHeight"),
        "a window fits the available screen: {geometry}"
    );
    assert!(
        read("innerWidth") <= read("outerWidth"),
        "the viewport fits the window: {geometry}"
    );
    assert!(
        read("innerHeight") < read("height"),
        "the viewport is not the whole screen: {geometry}"
    );
}

#[tokio::test]
#[ignore = "requires a local Chrome"]
#[serial_test::serial(chrome)]
async fn a_phone_persona_has_a_coarse_pointer_that_cannot_hover_and_fires_touch_events() {
    let persona = Fingerprint::android_mobile();
    let (browser, page) = open(&persona).await;

    let seen = page
        .evaluate(
            "({ touchEvents: 'ontouchstart' in window, points: navigator.maxTouchPoints, \
               pointerCoarse: matchMedia('(pointer: coarse)').matches, \
               anyPointerCoarse: matchMedia('(any-pointer: coarse)').matches, \
               hoverNone: matchMedia('(hover: none)').matches, \
               anyHoverNone: matchMedia('(any-hover: none)').matches })",
        )
        .await
        .expect("the window answers");
    browser.close().await.expect("Chrome closes");

    assert_eq!(seen["touchEvents"], json!(true), "touch events: {seen}");
    assert_eq!(
        seen["points"],
        json!(persona.max_touch_points().value().copied().unwrap_or(5)),
        "the touch points of the persona: {seen}"
    );
    for key in [
        "pointerCoarse",
        "anyPointerCoarse",
        "hoverNone",
        "anyHoverNone",
    ] {
        assert_eq!(
            seen[key],
            json!(true),
            "{key} follows touch emulation: {seen}"
        );
    }
}

#[tokio::test]
#[ignore = "requires a local Chrome"]
#[serial_test::serial(chrome)]
async fn the_media_features_of_a_persona_reach_match_media_and_style_sheets() {
    let persona = Fingerprint::windows_desktop()
        .to_builder()
        .media_features(MediaFeatures {
            color_scheme: ColorScheme::Dark,
            reduced_motion: true,
            ..MediaFeatures::new(ColorGamut::Srgb)
        })
        .build()
        .expect("a valid fingerprint");
    let (browser, page) = open(&persona).await;

    let seen = page
        .evaluate(
            "(() => { \
               const style = document.createElement('style'); \
               style.textContent = '#probe { width: 1px } \
                 @media (prefers-color-scheme: dark) { #probe { width: 2px } } \
                 @media (prefers-reduced-motion: reduce) { #probe { height: 3px } } \
                 @media (color-gamut: p3) { #probe { min-width: 9px } }'; \
               document.head.appendChild(style); \
               const probe = document.createElement('div'); probe.id = 'probe'; \
               document.body.appendChild(probe); \
               const css = getComputedStyle(probe); \
               return { \
                 mqDark: matchMedia('(prefers-color-scheme: dark)').matches, \
                 mqReduce: matchMedia('(prefers-reduced-motion: reduce)').matches, \
                 mqP3: matchMedia('(color-gamut: p3)').matches, \
                 mqSrgb: matchMedia('(color-gamut: srgb)').matches, \
                 cssDark: css.width === '2px', \
                 cssReduce: css.height === '3px', \
                 cssP3: css.minWidth === '9px' }; })()",
        )
        .await
        .expect("the window answers");
    browser.close().await.expect("Chrome closes");

    assert_eq!(seen["mqDark"], json!(true), "{seen}");
    assert_eq!(
        seen["cssDark"],
        json!(true),
        "@media agrees with matchMedia: {seen}"
    );
    assert_eq!(seen["mqReduce"], json!(true), "{seen}");
    assert_eq!(seen["cssReduce"], json!(true), "{seen}");
    assert_eq!(seen["mqP3"], json!(false), "{seen}");
    assert_eq!(seen["cssP3"], json!(false), "{seen}");
    assert_eq!(seen["mqSrgb"], json!(true), "{seen}");
}
