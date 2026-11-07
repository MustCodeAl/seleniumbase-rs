//! The browser agrees with itself: a spoofed persona captured in a real
//! headless Chrome shows no contradiction the evasions can avoid.
//!
//! One test per operating-system persona. Each launches the preset through the
//! pure CDP engine, opens a local page, captures and lists the contradictions
//! ([`CapturedFingerprint::contradictions`]). A contradiction whose rule is in
//! [`CLOSED`] fails the test: the evasions handle that surface and the browser
//! contradicted itself anyway. A rule in [`OPEN`] may fire: it is a gap the
//! ranked list of `docs/design/fingerprint_consistency.md` still holds, and the
//! host decides whether a run shows it (a machine without a GPU answers no
//! WebGPU adapter). When a step closes a gap, its rule moves from `OPEN` to
//! `CLOSED`, and the test flips from tolerating the contradiction to refusing
//! it. A rule in neither list is new and fails too.
//!
//! The tests need a Chrome, so they are ignored by default. Run them one at a
//! time with:
//!
//! ```text
//! SB_HEADLESS=true cargo test -p seleniumbase-rs --features "mcp-server test-util testing bdd" \
//!   --test chrome fingerprint_consistency_chrome:: -- --ignored --test-threads=1 --nocapture
//! ```

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use seleniumbase_rs::evasion::fingerprint::Contradiction;
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::Fingerprint;

const PAGE: &str = "<!doctype html><title>consistency</title><p>consistency</p>";

/// The surfaces the evasions make agree today. A contradiction here is a
/// regression.
const CLOSED: &[&str] = &[
    "platform_vs_user_agent",
    "client_hints_vs_user_agent",
    "vendor_vs_user_agent",
    "product_sub_vs_user_agent",
    "engine_vs_user_agent",
    "intl_vs_language",
    "language_vs_languages",
    "offset_vs_zone",
    "frame_vs_window",
    "codecs_disagree",
    "codecs_vs_browser",
    "webgl_vs_os",
    "webgpu_vs_webgl",
    "webgpu_vs_os",
    "forced_colors_vs_os",
    "pixel_ratio_vs_device",
    "headless_screen",
    "window_vs_screen",
];

/// The gaps the evasions leave open today, by rule, each with the rank it has
/// in the design note. A rule here may fire; the host decides which ones do.
const OPEN: &[(&str, &str)] = &[
    // Rank 1, WebGPU against WebGL and the system, is closed by the `webgpu` provider.
    // Rank 3's `window_vs_screen` and `pixel_ratio_vs_device` are closed by the geometry fixes.
    (
        "worker_vs_window",
        "rank 2: a worker reports the real cores, memory and GPU",
    ),
    (
        "no_taskbar",
        "rank 3: a headless shell keeps no taskbar out of the screen",
    ),
    (
        "viewport_is_screen",
        "rank 3: a headless shell has no frame",
    ),
    (
        "media_query_lie",
        "rank 4: matchMedia is patched and the @media rules are not",
    ),
    ("media_pointer_vs_device", "rank 4: the host's pointer"),
    ("color_gamut_vs_os", "rank 4: the host's display"),
    ("fonts_vs_os", "rank 6: the host's fonts"),
    ("api_presence_vs_persona", "rank 7: the host's APIs"),
    ("plugins_vs_device", "rank 8: Chrome's own plugin list"),
    ("pdf_viewer_vs_plugins", "rank 8: Chrome's own plugin list"),
    (
        "touch_vs_device",
        "rank 9: touch events follow the launch option, not the persona",
    ),
    ("voices_vs_os", "rank 13: the host's voices"),
    ("keyboard_vs_locale", "rank 14: the host's keyboard"),
    (
        "webgl_extensions_vs_renderer",
        "rank 15: the host's WebGL extensions",
    ),
    ("storage_quota_vs_device", "rank 16: the context's quota"),
    (
        "battery_shape",
        "rank 19: the preset's battery charges with no time to full",
    ),
];

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

/// Launches a headless Chrome as `persona`, captures the local page and returns
/// what the browser contradicts in itself.
async fn contradictions_of(persona: &Fingerprint) -> Vec<Contradiction> {
    let options = LaunchOptions::builder()
        .headless(true)
        .fingerprint(persona)
        .build()
        .expect("options");
    let browser = Browser::launch(options).await.expect("Chrome starts");
    let page = browser.default_page().await.expect("a tab");
    page.goto(&serve()).await.expect("the loopback page loads");

    let captured = page.capture_fingerprint().await.expect("the capture runs");
    assert_eq!(
        captured.fingerprint.user_agent(),
        persona.user_agent(),
        "the persona was applied"
    );
    let found = captured.contradictions();
    browser.close().await.expect("Chrome closes");
    found
}

/// Fails on a contradiction of a closed surface or of an unknown rule; prints
/// the open ones, so a run shows which gaps this host exposes.
fn assert_only_open_gaps(name: &str, found: &[Contradiction]) {
    let mut regressions = Vec::new();
    for contradiction in found {
        let rule = contradiction.rule;
        if CLOSED.contains(&rule) {
            regressions.push(format!("closed surface: {contradiction}"));
        } else if let Some((_, gap)) = OPEN.iter().find(|(open, _)| *open == rule) {
            println!("{name}: open gap ({gap}): {contradiction}");
        } else {
            regressions.push(format!("a rule in neither list: {contradiction}"));
        }
    }
    assert!(
        regressions.is_empty(),
        "{name} contradicts itself where the evasions should agree:\n  {}",
        regressions.join("\n  ")
    );
}

#[tokio::test]
#[ignore = "requires a local Chrome"]
#[serial_test::serial(chrome)]
async fn a_windows_persona_agrees_with_itself_where_the_evasions_reach() {
    let found = contradictions_of(&Fingerprint::windows_desktop()).await;
    assert_only_open_gaps("windows", &found);
}

#[tokio::test]
#[ignore = "requires a local Chrome"]
#[serial_test::serial(chrome)]
async fn a_macos_persona_agrees_with_itself_where_the_evasions_reach() {
    let found = contradictions_of(&Fingerprint::macos_desktop()).await;
    assert_only_open_gaps("macos", &found);
}

#[tokio::test]
#[ignore = "requires a local Chrome"]
#[serial_test::serial(chrome)]
async fn a_linux_persona_agrees_with_itself_where_the_evasions_reach() {
    let found = contradictions_of(&Fingerprint::linux_desktop()).await;
    assert_only_open_gaps("linux", &found);
}

#[tokio::test]
#[ignore = "requires a local Chrome"]
#[serial_test::serial(chrome)]
async fn an_android_persona_agrees_with_itself_where_the_evasions_reach() {
    let found = contradictions_of(&Fingerprint::android_mobile()).await;
    assert_only_open_gaps("android", &found);
}
