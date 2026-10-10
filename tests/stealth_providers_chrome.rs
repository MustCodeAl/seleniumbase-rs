//! The built-in evasion providers against a real Chrome.
//!
//! Until these tests existed, no provider's JavaScript had ever been parsed or
//! run. Each test installs one or more providers into a fresh headless Chrome
//! through the Pure CDP engine, then asks the page what a detection script
//! would ask: is the replaced function native-looking, do two reads agree,
//! does the API return what Chrome would. Where Chrome itself is the reference
//! the probe runs before and after the install and compares.
//!
//! They need a Chrome or Chromium, so they are ignored by default:
//!
//! ```text
//! cargo test --test stealth_providers_chrome -- --ignored
//! ```

use std::collections::HashSet;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, Page};
use seleniumbase_rs::stealth::evasions::bootstrap_script;
use seleniumbase_rs::stealth::fingerprint::{
    BatteryProfile, ConnectionProfile, Fingerprint, MaskingMode,
};
use seleniumbase_rs::stealth::providers::{
    builtin, EvasionConfig, EvasionContext, EvasionProvider, EvasionRegistry,
};
use seleniumbase_rs::OsType;
use serde_json::{json, Value};

const FIXTURE: &str = "<!doctype html><html><head><title>Providers</title></head>\
                       <body><h1 id=\"title\">fixture</h1></body></html>";

/// Serves the fixture, and `ok` for `/ok`, on a loopback port. Every answer
/// allows cross-origin reads, so a page on `127.0.0.1` can fetch from
/// `localhost`.
fn serve() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let port = listener.local_addr().expect("bound address").port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buffer = [0_u8; 2048];
            let read = stream.read(&mut buffer).unwrap_or(0);
            let request = String::from_utf8_lossy(&buffer[..read]);
            let (kind, body) = if request.starts_with("GET /ok") {
                ("text/plain", "ok")
            } else {
                ("text/html", FIXTURE)
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nAccess-Control-Allow-Origin: *\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
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
    port: u16,
}

async fn lab() -> Lab {
    let port = serve();
    let options = LaunchOptions::builder()
        .headless(true)
        .no_sandbox(std::env::var_os("CI").is_some())
        .window_size(1000, 800)
        .build()
        .expect("valid launch options");
    let browser = Browser::launch(options).await.expect("Chrome starts");
    let page = browser.default_page().await.expect("a page");
    page.goto(format!("http://127.0.0.1:{port}/"))
        .await
        .expect("the fixture loads");
    let page = page.with_timeout(Duration::from_secs(10));
    // The engine installs its own helpers on first use; do that now so a
    // before/after comparison of window sees only the providers.
    page.evaluate("1").await.expect("the page runs script");
    Lab {
        browser,
        page,
        port,
    }
}

impl Lab {
    async fn eval(&self, js: &str) -> Value {
        let js = js.replace("PORT", &self.port.to_string());
        self.page
            .evaluate(&js)
            .await
            .unwrap_or_else(|error| panic!("script failed: {error}\n{js}"))
    }

    /// Installs the named providers (and the `toString` masking they rely on),
    /// failing loudly if their JavaScript throws.
    async fn install(&self, fingerprint: &Fingerprint, names: &[&str]) {
        let script = script_of(fingerprint, names);
        self.page
            .evaluate(&script)
            .await
            .unwrap_or_else(|error| panic!("a provider threw: {error}\n{script}"));
    }

    async fn close(self) {
        self.browser.close().await.expect("Chrome closes");
    }
}

fn script_of(fingerprint: &Fingerprint, names: &[&str]) -> String {
    let mut registry = EvasionRegistry::empty();
    for provider in builtin::all() {
        if provider.name() == "native_tostring" || names.contains(&provider.name()) {
            registry.register_provider(provider);
        }
    }
    let config = EvasionConfig {
        isolate_failures: false,
        annotate: false,
    };
    registry.bootstrap(&EvasionContext::with_config(fingerprint, config))
}

fn persona() -> Fingerprint {
    Fingerprint::windows_desktop()
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn replaced_functions_and_getters_look_native() {
    let lab = lab().await;
    let probe = r##"(() => {
        const d = (fn) => JSON.stringify([fn.name, fn.length, 'prototype' in fn,
            Function.prototype.toString.call(fn), Object.getOwnPropertyNames(fn).sort()]);
        const g = (o, p) => Object.getOwnPropertyDescriptor(o, p).get;
        return {
            query: d(navigator.permissions.query),
            canPlayType: d(HTMLMediaElement.prototype.canPlayType),
            toDataURL: d(HTMLCanvasElement.prototype.toDataURL),
            toString: d(Function.prototype.toString),
            userAgent: d(g(Navigator.prototype, 'userAgent')),
            languages: d(g(Navigator.prototype, 'languages')),
            webdriver: d(g(Navigator.prototype, 'webdriver')),
        };
    })()"##;
    let before = lab.eval(probe).await;
    let mut fp = persona();
    fp.user_agent = Some("Mozilla/5.0 (Windows NT 10.0) Chrome/155.0.0.0".to_owned());
    lab.install(
        &fp,
        &[
            "permissions",
            "media_codecs",
            "canvas_noise",
            "navigator_props",
            "localization",
            "webdriver",
        ],
    )
    .await;
    let after = lab.eval(probe).await;

    for key in before.as_object().unwrap().keys() {
        assert_eq!(
            before[key], after[key],
            "{key} no longer matches what Chrome reports for a native function"
        );
    }
    lab.close().await;
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn no_provider_leaves_a_name_or_property_a_page_can_see() {
    let lab = lab().await;
    let probe = r##"(() => {
        const own = (o) => Object.getOwnPropertyNames(o);
        const analyser = new (window.AudioContext || window.webkitAudioContext)().createAnalyser();
        return {
            window: own(window), document: own(document), navigator: own(navigator),
            error: own(Error), webgl: own(WebGLRenderingContext.prototype),
            fonts: own(document.fonts), permissions: own(navigator.permissions),
            mediaDevices: own(navigator.mediaDevices), geolocation: own(navigator.geolocation),
            speech: own(speechSynthesis), analyser: own(analyser),
            documentProto: own(Document.prototype),
        };
    })()"##;
    let before = lab.eval(probe).await;

    let mut fp = persona();
    fp.fonts = vec!["Arial".to_owned()];
    fp.latitude = Some(48.85);
    fp.longitude = Some(2.35);
    fp.webgl_vendor_id = Some("0x10de".to_owned());
    fp.webgl_renderer_id = Some("0x1f91".to_owned());
    fp.flags.audio_masking = MaskingMode::Mask;
    fp.flags.media_devices_masking = MaskingMode::Mask;
    fp.flags.block_trackers = true;
    lab.page
        .evaluate(bootstrap_script(&fp))
        .await
        .expect("the bootstrap runs");
    let after = lab.eval(probe).await;

    for key in before.as_object().unwrap().keys() {
        let known: HashSet<&str> = before[key]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        let added: Vec<&str> = after[key]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .filter(|name| !known.contains(name))
            // `window.chrome` is the one object the providers are meant to add.
            .filter(|name| !(key == "window" && *name == "chrome"))
            .collect();
        assert!(
            added.is_empty(),
            "{key} gained properties a detection script can list: {added:?}"
        );
    }
    lab.close().await;
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn the_font_check_understands_css_shorthand() {
    let lab = lab().await;
    let mut fp = persona();
    fp.fonts = vec!["Arial".to_owned(), "Segoe UI".to_owned()];
    lab.install(&fp, &["fonts"]).await;

    let result = lab
        .eval(
            r##"({
            plain: document.fonts.check('Arial'),
            shorthand: document.fonts.check('12px Arial'),
            quoted: document.fonts.check('bold 14px "Segoe UI", sans-serif'),
            slash: document.fonts.check('italic 12px/1.5 Arial'),
            generic: document.fonts.check('12px sans-serif'),
            absent: document.fonts.check('12px NotInstalledFont'),
            own: Object.getOwnPropertyNames(document.fonts),
        })"##,
        )
        .await;

    assert_eq!(result["plain"], true);
    assert_eq!(
        result["shorthand"], true,
        "12px Arial names an allowed font"
    );
    assert_eq!(
        result["quoted"], true,
        "the first family of the list counts"
    );
    assert_eq!(result["slash"], true, "size/line-height is one token");
    assert_eq!(result["generic"], true, "generic families always exist");
    assert_eq!(result["absent"], false);
    assert_eq!(
        result["own"],
        json!([]),
        "check and load belong on FontFaceSet.prototype, not on the instance"
    );
    lab.close().await;
}

const DATES: &str = r##"(() => {
    const winter = new Date(Date.UTC(2024, 0, 15, 12, 0, 0));
    const summer = new Date(Date.UTC(2024, 6, 15, 12, 0, 0));
    const beforeShift = new Date(Date.UTC(2024, 10, 3, 5, 30, 0));
    const afterShift = new Date(Date.UTC(2024, 10, 3, 6, 30, 0));
    const options = { hour: 'numeric' };
    new Intl.DateTimeFormat('en-US', options);
    return {
        zone: Intl.DateTimeFormat().resolvedOptions().timeZone,
        explicitUtc: new Intl.DateTimeFormat('en-US', { timeZone: 'UTC' }).resolvedOptions().timeZone,
        callerOptionsUntouched: !('timeZone' in options),
        sameConstructor: new Intl.DateTimeFormat().constructor === Intl.DateTimeFormat,
        winterOffset: winter.getTimezoneOffset(),
        summerOffset: summer.getTimezoneOffset(),
        beforeShift: beforeShift.getTimezoneOffset(),
        afterShift: afterShift.getTimezoneOffset(),
        hours: winter.getHours(), day: winter.getDate(), weekday: winter.getDay(),
        month: winter.getMonth(), year: winter.getFullYear(),
        winterString: winter.toString(), summerString: summer.toString(),
        timeString: winter.toTimeString(), dateString: winter.toDateString(),
        locale: winter.toLocaleString('en-US').replace(/ /g, ' '),
        localeTime: winter.toLocaleTimeString('en-US').replace(/ /g, ' '),
        localeUtc: winter.toLocaleString('en-US', { timeZone: 'UTC' }).replace(/ /g, ' '),
        asString: String(winter), concat: '' + winter,
        invalid: new Date(NaN).toString(), invalidOffset: new Date(NaN).getTimezoneOffset(),
        iso: winter.toISOString(),
        toStringNative: Function.prototype.toString.call(Date.prototype.getTimezoneOffset),
    };
})()"##;

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn the_timezone_provider_makes_date_and_intl_agree() {
    let lab = lab().await;
    let mut fp = persona();
    fp.timezone = Some("Asia/Tokyo".to_owned());
    lab.install(&fp, &["timezone"]).await;

    let r = lab.eval(DATES).await;

    assert_eq!(r["zone"], "Asia/Tokyo");
    assert_eq!(r["explicitUtc"], "UTC", "a zone the caller asked for stays");
    assert_eq!(r["callerOptionsUntouched"], true);
    assert_eq!(r["sameConstructor"], true);
    assert_eq!(r["winterOffset"], -540, "Date must agree with Intl");
    assert_eq!(r["summerOffset"], -540);
    assert_eq!(r["hours"], 21);
    assert_eq!(r["day"], 15);
    assert_eq!(r["weekday"], 1);
    assert_eq!(r["month"], 0);
    assert_eq!(r["year"], 2024);
    assert_eq!(
        r["winterString"],
        "Mon Jan 15 2024 21:00:00 GMT+0900 (Japan Standard Time)"
    );
    assert_eq!(r["timeString"], "21:00:00 GMT+0900 (Japan Standard Time)");
    assert_eq!(r["dateString"], "Mon Jan 15 2024");
    // Chrome puts a narrow no-break space before PM in recent versions; the
    // probe turns it into a plain one.
    assert_eq!(r["locale"], "1/15/2024, 9:00:00 PM");
    assert_eq!(r["localeTime"], "9:00:00 PM");
    assert_eq!(r["localeUtc"], "1/15/2024, 12:00:00 PM");
    assert_eq!(r["asString"], r["winterString"]);
    assert_eq!(r["concat"], r["winterString"]);
    assert_eq!(r["invalid"], "Invalid Date");
    assert!(r["invalidOffset"].is_null(), "NaN does not survive JSON");
    assert_eq!(r["iso"], "2024-01-15T12:00:00.000Z");
    assert_eq!(
        r["toStringNative"],
        "function getTimezoneOffset() { [native code] }"
    );
    lab.close().await;
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn the_timezone_provider_follows_daylight_saving_time() {
    let lab = lab().await;
    let mut fp = persona();
    fp.timezone = Some("America/New_York".to_owned());
    lab.install(&fp, &["timezone"]).await;

    let r = lab.eval(DATES).await;

    assert_eq!(r["zone"], "America/New_York");
    assert_eq!(r["winterOffset"], 300);
    assert_eq!(r["summerOffset"], 240);
    assert_eq!(r["beforeShift"], 240, "05:30 UTC on 3 Nov is still EDT");
    assert_eq!(r["afterShift"], 300, "06:30 UTC on 3 Nov is EST");
    assert_eq!(r["hours"], 7);
    assert_eq!(
        r["summerString"],
        "Mon Jul 15 2024 08:00:00 GMT-0400 (Eastern Daylight Time)"
    );
    assert_eq!(
        r["winterString"],
        "Mon Jan 15 2024 07:00:00 GMT-0500 (Eastern Standard Time)"
    );
    lab.close().await;
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn the_permission_state_is_a_real_permission_state() {
    let lab = lab().await;
    lab.install(&persona(), &["permissions"]).await;

    let r = lab
        .eval(
            r##"(async () => {
            const status = await navigator.permissions.query({ name: 'notifications' });
            return { state: status.state, notification: Notification.permission };
        })()"##,
        )
        .await;

    let expected = match r["notification"].as_str().unwrap() {
        "default" => "prompt",
        other => other,
    };
    assert_eq!(
        r["state"], expected,
        "PermissionState is granted, denied or prompt, never default"
    );
    lab.close().await;
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn languages_and_plugins_are_the_same_object_on_every_read() {
    let lab = lab().await;
    let mut fp = persona();
    fp.languages = Some("de-DE,de;q=0.9,en;q=0.8".to_owned());
    lab.install(&fp, &["localization", "plugins"]).await;

    let r = lab
        .eval(
            r##"({
            sameLanguages: navigator.languages === navigator.languages,
            frozen: Object.isFrozen(navigator.languages),
            languages: Array.from(navigator.languages),
            language: navigator.language,
            samePlugins: navigator.plugins === navigator.plugins,
            sameMimeTypes: navigator.mimeTypes === navigator.mimeTypes,
            pluginCount: navigator.plugins.length,
            firstPlugin: navigator.plugins[0].name,
            byName: navigator.plugins.namedItem('PDF Viewer') === navigator.plugins[0],
            pluginIsPlugin: navigator.plugins[0] instanceof Plugin,
        })"##,
        )
        .await;

    assert_eq!(r["sameLanguages"], true);
    assert_eq!(r["frozen"], true, "Chrome hands out a frozen array");
    assert_eq!(r["languages"], json!(["de-DE", "de", "en"]));
    assert_eq!(r["language"], "de-DE");
    assert_eq!(r["samePlugins"], true);
    assert_eq!(r["sameMimeTypes"], true);
    assert_eq!(r["pluginCount"], 5);
    assert_eq!(r["firstPlugin"], "PDF Viewer");
    assert_eq!(r["byName"], true);
    assert_eq!(r["pluginIsPlugin"], true);
    lab.close().await;
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn the_connection_provider_changes_values_not_the_shape() {
    let lab = lab().await;
    let probe = r##"(() => ({
        moz: 'mozConnection' in navigator,
        webkit: 'webkitConnection' in navigator,
        instance: navigator.connection instanceof NetworkInformation,
        own: Object.getOwnPropertyNames(navigator.connection),
        proto: Object.getOwnPropertyNames(NetworkInformation.prototype).sort(),
        type: 'type' in navigator.connection,
        max: 'downlinkMax' in navigator.connection,
        effectiveType: navigator.connection.effectiveType,
        rtt: navigator.connection.rtt,
        downlink: String(navigator.connection.downlink),
        saveData: navigator.connection.saveData,
        listens: (() => { navigator.connection.addEventListener('change', () => {}); return true; })(),
    }))()"##;
    let before = lab.eval(probe).await;
    let mut fp = persona();
    fp.connection = Some(ConnectionProfile {
        effective_type: "3g".to_owned(),
        rtt: 300,
        downlink: 1.5,
        save_data: true,
    });
    lab.install(&fp, &["connection"]).await;

    let after = lab.eval(probe).await;

    assert_eq!(after["effectiveType"], "3g");
    assert_eq!(after["rtt"], 300);
    assert_eq!(after["downlink"], "1.5");
    assert_eq!(after["saveData"], true);
    for shape in [
        "moz", "webkit", "instance", "own", "proto", "type", "max", "listens",
    ] {
        assert_eq!(
            before[shape], after[shape],
            "{shape}: the object must stay what Chrome made it"
        );
    }
    assert_eq!(after["moz"], false, "Chrome has no mozConnection");
    assert_eq!(after["webkit"], false, "Chrome has no webkitConnection");

    // A value f64 prints as `inf` must still reach the page as a number.
    let mut fp = persona();
    fp.connection = Some(ConnectionProfile {
        effective_type: "4g".to_owned(),
        rtt: 50,
        downlink: f64::INFINITY,
        save_data: false,
    });
    lab.install(&fp, &["connection"]).await;
    let infinite = lab.eval("String(navigator.connection.downlink)").await;
    assert_eq!(infinite, "Infinity");
    lab.close().await;
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn the_battery_provider_reports_infinite_times() {
    let lab = lab().await;
    let mut fp = persona();
    fp.battery = Some(BatteryProfile {
        charging: false,
        level: 0.5,
        charging_time: f64::INFINITY,
        discharging_time: 7200.0,
    });
    lab.install(&fp, &["battery"]).await;

    let r = lab
        .eval(
            r##"(async () => {
            const b = await navigator.getBattery();
            return { charging: b.charging, level: b.level, chargingTime: String(b.chargingTime),
                     dischargingTime: b.dischargingTime };
        })()"##,
        )
        .await;

    assert_eq!(r["charging"], false);
    assert_eq!(r["level"], 0.5);
    assert_eq!(r["chargingTime"], "Infinity");
    assert_eq!(r["dischargingTime"], 7200);
    lab.close().await;
}

const CANVAS: &str = r##"(() => {
    const make = () => {
        const c = document.createElement('canvas');
        c.width = 64; c.height = 32;
        const x = c.getContext('2d');
        x.fillStyle = 'rgb(100,150,200)'; x.fillRect(0, 0, 64, 32);
        x.fillStyle = '#f60'; x.font = '14px Arial'; x.fillText('probe', 2, 20);
        return [c, x];
    };
    const same = (a, b) => a.length === b.length && a.every((v, i) => v === b[i]);
    const [c, x] = make();
    const a = c.toDataURL(); const b = c.toDataURL();
    const first = Array.from(x.getImageData(0, 0, 64, 32).data);
    const second = Array.from(x.getImageData(0, 0, 64, 32).data);
    const region = Array.from(x.getImageData(10, 10, 4, 4).data);
    let regionAgrees = true;
    for (let row = 0; row < 4; row++) for (let col = 0; col < 4; col++)
        if (first[((10 + row) * 64 + 10 + col) * 4] !== region[(row * 4 + col) * 4]) regionAgrees = false;
    const blob = new Promise((resolve) => c.toBlob((blob) => blob.arrayBuffer().then((buf) => resolve(buf.byteLength)), 'image/png'));
    return blob.then((blobSize) => ({
        urlStable: a === b, pixelsStable: same(first, second), regionAgrees,
        url: a, red: first.filter((_, i) => i % 4 === 0).join(','), blobSize,
    }));
})()"##;

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn canvas_noise_is_the_same_on_every_read() {
    let lab = lab().await;
    let before = lab.eval(CANVAS).await;
    lab.install(&persona(), &["canvas_noise"]).await;
    // The canvas's own pixels, read through the browser's untouched method.
    let after = lab.eval(CANVAS).await;

    assert_eq!(before["urlStable"], true, "Chrome itself is stable");
    assert_eq!(
        after["urlStable"], true,
        "two toDataURL calls on one canvas must agree"
    );
    assert_eq!(
        after["pixelsStable"], true,
        "two getImageData reads must agree"
    );
    assert_eq!(
        after["regionAgrees"], true,
        "a sub-rectangle must carry the noise of the same pixels"
    );
    assert_ne!(
        before["url"], after["url"],
        "noise is supposed to change the fingerprint"
    );
    assert_ne!(before["red"], after["red"]);
    assert!(
        after["blobSize"].as_u64().unwrap() > 0,
        "toBlob still produces an image"
    );

    // A second install of the same fingerprint gives the same noise: it is a
    // function of the seed, not of how many reads came before.
    let again = lab.eval(CANVAS).await;
    assert_eq!(after["url"], again["url"]);
    lab.close().await;
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn canvas_reads_do_not_change_the_canvas() {
    let lab = lab().await;
    lab.eval(
        r##"window.draw = () => {
        const c = document.createElement('canvas'); c.width = 16; c.height = 16;
        const x = c.getContext('2d'); x.fillStyle = 'rgb(10,20,30)'; x.fillRect(0, 0, 16, 16);
        return [c, x];
    }; window.rawRead = CanvasRenderingContext2D.prototype.getImageData; 1"##,
    )
    .await;
    lab.install(&persona(), &["canvas_noise"]).await;

    let r = lab
        .eval(
            r##"(() => {
            const [c, x] = draw();
            for (let i = 0; i < 5; i++) c.toDataURL();
            const px = rawRead.call(x, 0, 0, 16, 16).data;
            return Array.from(px).filter((_, i) => i % 4 === 0).every((v) => v === 10);
        })()"##,
        )
        .await;

    assert_eq!(r, true, "toDataURL must not write noise into the canvas");
    lab.close().await;
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn audio_noise_is_the_same_on_every_read() {
    let lab = lab().await;
    let render = r##"(async () => {
        const ctx = new OfflineAudioContext(1, 44100, 44100);
        const osc = ctx.createOscillator(); osc.type = 'triangle'; osc.frequency.value = 10000;
        const comp = ctx.createDynamicsCompressor();
        osc.connect(comp); comp.connect(ctx.destination); osc.start(0);
        const buffer = await ctx.startRendering();
        const a = Array.from(buffer.getChannelData(0));
        const b = Array.from(buffer.getChannelData(0));
        const copy = new Float32Array(a.length); buffer.copyFromChannel(copy, 0);
        return { stable: a.length === b.length && a.every((v, i) => v === b[i]),
                 copyAgrees: a.every((v, i) => v === copy[i]),
                 sum: a.reduce((s, v) => s + v, 0), first: a.slice(4500, 4510).join(',') };
    })()"##;
    let before = lab.eval(render).await;
    let mut fp = persona();
    fp.flags.audio_masking = MaskingMode::Mask;
    lab.install(&fp, &["audio_noise"]).await;

    let after = lab.eval(render).await;
    let again = lab.eval(render).await;

    assert_eq!(before["stable"], true);
    assert_eq!(
        after["stable"], true,
        "reading a channel twice must not add noise twice"
    );
    assert_eq!(
        after["copyAgrees"], true,
        "copyFromChannel sees the same samples"
    );
    // Samples are 32-bit floats, so noise this small can vanish on a loud
    // sample; over the whole second it must still move the total.
    assert_ne!(before["sum"], after["sum"], "the noise should be applied");
    assert_eq!(
        after["sum"], again["sum"],
        "and be the same for a new buffer"
    );
    assert_eq!(after["first"], again["first"]);

    let analyser = lab
        .eval(
            "Object.getOwnPropertyNames(new (window.AudioContext || window.webkitAudioContext)().createAnalyser())",
        )
        .await;
    assert_eq!(
        analyser,
        json!([]),
        "an analyser node has no own methods in Chrome"
    );
    lab.close().await;
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn tracker_blocking_matches_hosts_and_covers_fetch_xhr_and_beacons() {
    let lab = lab().await;
    let mut fp = persona();
    fp.flags.block_trackers = true;
    fp.blocked_trackers = vec!["localhost".to_owned()];
    lab.install(&fp, &["tracker_block"]).await;

    let r = lab
        .eval(
            r##"(async () => {
            const run = async (target) => {
                try { const r = await fetch(target); return 'ok:' + await r.text(); }
                catch (e) { return e.constructor.name + ':' + e.message; }
            };
            const xhr = (target) => new Promise((resolve) => {
                const x = new XMLHttpRequest();
                x.open('GET', target);
                x.onerror = () => resolve('error:' + x.status);
                x.onload = () => resolve('load:' + x.responseText);
                x.send();
            });
            return {
                blocked: await run('http://localhost:PORT/ok'),
                subdomain: await run('http://a.localhost:PORT/ok'),
                urlObject: await run(new URL('http://localhost:PORT/ok')),
                request: await run(new Request('http://localhost:PORT/ok')),
                otherHost: await run('http://127.0.0.1:PORT/ok'),
                inQuery: await run('http://127.0.0.1:PORT/ok?ref=localhost'),
                xhrBlocked: await xhr('http://localhost:PORT/ok'),
                xhrAllowed: await xhr('http://127.0.0.1:PORT/ok'),
                beaconBlocked: navigator.sendBeacon('http://localhost:PORT/ok'),
                globals: Object.getOwnPropertyNames(window).filter((n) => n.startsWith('__sb') && n !== '__sbcdp'),
                fetchSource: Function.prototype.toString.call(fetch),
            };
        })()"##,
        )
        .await;

    assert_eq!(r["blocked"], "TypeError:Failed to fetch");
    assert_eq!(
        r["subdomain"], "TypeError:Failed to fetch",
        "a subdomain of a tracker"
    );
    assert_eq!(
        r["urlObject"], "TypeError:Failed to fetch",
        "a URL object is a target too"
    );
    assert_eq!(r["request"], "TypeError:Failed to fetch");
    assert_eq!(r["otherHost"], "ok:ok");
    assert_eq!(
        r["inQuery"], "ok:ok",
        "the tracker's name in a query string is not the tracker"
    );
    assert_eq!(r["xhrBlocked"], "error:0");
    assert_eq!(r["xhrAllowed"], "load:ok");
    assert_eq!(r["beaconBlocked"], false);
    assert_eq!(r["globals"], json!([]));
    assert_eq!(r["fetchSource"], "function fetch() { [native code] }");
    lab.close().await;
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn providers_that_cannot_work_leave_the_page_untouched() {
    let lab = lab().await;
    let probe = r##"(() => ({
        prepareStackTrace: Object.getOwnPropertyDescriptor(Error, 'prepareStackTrace') === undefined,
        createElementOwn: Object.getOwnPropertyNames(document).includes('createElement'),
        iframeWindowBeforeInsert: document.createElement('iframe').contentWindow,
    }))()"##;
    let before = lab.eval(probe).await;
    lab.install(&persona(), &["prepare_stack_trace", "iframe"])
        .await;

    let after = lab.eval(probe).await;

    assert_eq!(before, after, "these providers must not be observable");
    assert_eq!(after["prepareStackTrace"], true);
    assert_eq!(after["createElementOwn"], false);
    assert_eq!(after["iframeWindowBeforeInsert"], Value::Null);
    lab.close().await;
}

const CODECS: [&str; 9] = [
    "video/mp4",
    "video/mp4; codecs=\"avc1.42E01E\"",
    "video/mp4; codecs=\"avc1.42E01E, mp4a.40.2\"",
    "video/mp4; codecs=\"bogus\"",
    "audio/mpeg",
    "audio/aac",
    "video/webm; codecs=\"vp8\"",
    "video/x-unknown",
    "",
];

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn codec_support_matches_chrome_for_what_it_can_play() {
    let lab = lab().await;
    let probe = format!(
        "(() => {{ const v = document.createElement('video'); \
         return {}.map((t) => v.canPlayType(t)); }})()",
        serde_json::to_string(&CODECS).unwrap()
    );
    let before = lab.eval(&probe).await;
    lab.install(&persona(), &["media_codecs"]).await;

    let after = lab.eval(&probe).await;

    assert_eq!(
        before, after,
        "a Chrome that already answers must not be second-guessed"
    );
    lab.close().await;
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn codec_support_is_filled_in_only_for_a_chromium_without_proprietary_codecs() {
    let lab = lab().await;
    let probe = format!(
        "(() => {{ const v = document.createElement('video'); \
         return {}.map((t) => v.canPlayType(t)); }})()",
        serde_json::to_string(&CODECS).unwrap()
    );
    // What a Chrome with the proprietary codecs says: the reference.
    let real = lab.eval(&probe).await;
    assert_eq!(real[0], "maybe", "a bare container is only 'maybe'");
    assert_eq!(real[3], "", "a codec nobody has is not playable");

    // Now make this Chrome behave like a Chromium build that cannot play
    // MP4, AAC or MP3, and install the provider on top of that.
    lab.eval(
        r##"(() => {
        const real = HTMLMediaElement.prototype.canPlayType;
        HTMLMediaElement.prototype.canPlayType = function canPlayType(t) {
            return /mp4|mpeg|aac/i.test(String(t)) ? '' : real.call(this, t);
        };
        return 1;
    })()"##,
    )
    .await;
    let crippled = lab.eval(&probe).await;
    assert_eq!(crippled[1], "", "the stand-in really cannot play MP4");
    lab.install(&persona(), &["media_codecs"]).await;

    let after = lab.eval(&probe).await;

    assert_eq!(
        after, real,
        "the provider must answer as Chrome with proprietary codecs does"
    );
    lab.close().await;
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn window_geometry_and_headless_fixes_do_not_fight_over_the_outer_size() {
    let lab = lab().await;
    let probe = r##"(() => {
        const d = Object.getOwnPropertyDescriptor(window, 'outerWidth');
        return {
            descriptor: [typeof d.get, typeof d.set, d.enumerable, d.configurable],
            screenOwn: Object.getOwnPropertyNames(screen),
            documentOwn: Object.getOwnPropertyNames(document),
            dprDescriptor: (({ get, set, enumerable, configurable }) =>
                [typeof get, typeof set, enumerable, configurable])(
                Object.getOwnPropertyDescriptor(window, 'devicePixelRatio')),
        };
    })()"##;
    let before = lab.eval(probe).await;
    lab.install(&persona(), &["window_geometry", "headless"])
        .await;

    let after = lab
        .eval(
            r##"(() => ({
            screen: [screen.width, screen.height, screen.availWidth, screen.colorDepth],
            outer: [outerWidth, outerHeight], inner: [innerWidth, innerHeight],
            hidden: document.hidden, visibility: document.visibilityState,
            documentOwn: Object.getOwnPropertyNames(document),
        }))()"##,
        )
        .await;
    let shape = lab.eval(probe).await;

    assert_eq!(after["screen"], json!([1920, 1080, 1920, 24]));
    assert_eq!(
        after["outer"][0], 1920,
        "window_geometry owns the outer size; headless must not undo it"
    );
    assert!(
        after["outer"][1].as_u64() >= after["inner"][1].as_u64(),
        "a window is never smaller than its viewport: {after}"
    );
    assert_eq!(after["hidden"], false);
    assert_eq!(after["visibility"], "visible");
    assert_eq!(
        after["documentOwn"], before["documentOwn"],
        "hidden and visibilityState belong on Document.prototype"
    );
    assert_eq!(before["descriptor"], shape["descriptor"]);
    assert_eq!(before["dprDescriptor"], shape["dprDescriptor"]);
    assert_eq!(before["screenOwn"], shape["screenOwn"]);
    lab.close().await;
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn text_in_a_fingerprint_cannot_break_out_of_its_string() {
    let lab = lab().await;
    let hostile = "x'; window.pwned = 1; '\\ \u{2028} \"q\" `t` ${1}";
    let mut fp = persona();
    fp.user_agent = Some(hostile.to_owned());
    fp.platform = Some(hostile.to_owned());
    fp.vendor = Some(hostile.to_owned());
    // A semicolon in a language list introduces a quality value, so it is not
    // part of the name.
    let language = hostile.replace(';', ",");
    fp.languages = Some(format!("{},en", language.replace(',', " ")));
    lab.install(&fp, &["navigator_props", "localization"]).await;

    let r = lab
        .eval(
            r##"({ ua: navigator.userAgent, platform: navigator.platform, vendor: navigator.vendor,
                  language: navigator.language, pwned: typeof window.pwned })"##,
        )
        .await;

    assert_eq!(r["ua"], hostile);
    assert_eq!(r["platform"], hostile);
    assert_eq!(r["vendor"], hostile);
    assert_eq!(r["language"], language.replace(',', " "));
    assert_eq!(r["pwned"], "undefined");
    lab.close().await;
}

struct Multiline;

impl EvasionProvider for Multiline {
    fn name(&self) -> &str {
        "multiline"
    }
    fn script(&self, _ctx: &EvasionContext) -> Option<String> {
        Some("window.template = `first\n  second\n    third`;".to_owned())
    }
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn assembling_a_script_does_not_change_a_template_literal() {
    let lab = lab().await;
    let mut registry = EvasionRegistry::empty();
    registry.register_provider(Box::new(Multiline));
    let fp = persona();
    let script = registry.bootstrap(&EvasionContext::new(&fp));

    lab.page.evaluate(&script).await.expect("the script runs");

    let value = lab.eval("window.template").await;
    assert_eq!(value, "first\n  second\n    third");
    lab.close().await;
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn a_whole_fingerprint_applied_over_cdp_is_consistent() {
    let lab = lab().await;
    let fp = Fingerprint::randomized(OsType::Windows, 7);
    lab.page
        .apply_fingerprint(&fp)
        .await
        .expect("the browser accepts every override");
    lab.page
        .goto(format!("http://127.0.0.1:{}/", lab.port))
        .await
        .expect("the page reloads");

    let r = lab
        .eval(
            r##"(() => {
            const zone = Intl.DateTimeFormat().resolvedOptions().timeZone;
            const now = new Date();
            const p = {};
            new Intl.DateTimeFormat('en-US', { timeZone: zone, hourCycle: 'h23', year: 'numeric',
                month: 'numeric', day: 'numeric', hour: 'numeric', minute: 'numeric',
                second: 'numeric' }).formatToParts(now).forEach((x) => { p[x.type] = x.value; });
            const asUtc = Date.UTC(+p.year, +p.month - 1, +p.day, +p.hour % 24, +p.minute, +p.second);
            const fromIntl = 0 - Math.round((asUtc - Math.floor(now.getTime() / 1000) * 1000) / 60000);
            return {
                zone, offsetAgrees: fromIntl === now.getTimezoneOffset(),
                userAgent: navigator.userAgent, platform: navigator.platform,
                sameLanguages: navigator.languages === navigator.languages,
                language: navigator.language, webdriver: navigator.webdriver,
                screen: [screen.width, screen.height],
                cores: navigator.hardwareConcurrency,
            };
        })()"##,
        )
        .await;

    assert_eq!(r["zone"], json!(fp.timezone));
    assert_eq!(
        r["offsetAgrees"], true,
        "Date and Intl must name one time zone"
    );
    assert_eq!(r["userAgent"], json!(fp.user_agent));
    assert_eq!(r["platform"], json!(fp.platform));
    assert_eq!(r["sameLanguages"], true);
    assert_eq!(r["webdriver"], false);
    assert_eq!(r["screen"], json!([fp.screen_width, fp.screen_height]));
    assert_eq!(r["cores"], json!(fp.hardware_concurrency));
    lab.close().await;
}
