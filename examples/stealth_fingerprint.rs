//! Build anti-detection fingerprints (personas), check that they are
//! consistent, and optionally launch a browser that presents one.
//!
//! A [`Fingerprint`] describes the browser a page should believe it is talking
//! to: user agent, platform, screen, GPU, languages, time zone, and more. Every
//! setting is one `Spoof`: leave the browser's own value (`Natural`), draw a
//! coherent one (`Random`), use exactly this one (`Value`, which a plain value
//! converts to) or turn the API off (`Off`, where that means something). How
//! the claims are delivered is a `StealthPolicy`, not a setting. Part 1 needs no
//! browser:
//!
//! * the ready-made personas (`windows_desktop`, `edge_desktop`,
//!   `macos_desktop`, `linux_desktop`, `android_mobile`, `ios_mobile_safari`),
//! * the builder, where giving a value is all it takes and a built fingerprint
//!   is never changed (`to_builder` starts the next one),
//! * `validate`, which catches combinations that give a persona away (a Windows
//!   user agent on a `MacIntel` platform, say),
//! * `Fingerprint::randomized`, which makes the same persona for the same seed,
//! * the Chromium launch flags and the page script a persona produces.
//!
//! Part 2 only exists with the `playwright` feature. It launches headless
//! Chromium with the persona, loads a page, injects the persona's script, and
//! reads `navigator.userAgent`, `navigator.platform` and `screen` back from
//! page JavaScript. The script is injected after the navigation because it
//! patches the current document only, so loading a new page discards it.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example stealth_fingerprint                       # part 1
//! cargo run --example stealth_fingerprint --features playwright # parts 1 and 2
//! ```
//!
//! Part 1 needs nothing. Part 2 needs the `rustwright` engine's Chromium, which
//! it may download on first launch. The page is served from `127.0.0.1` by
//! `local_site/mod.rs`.

#[cfg(feature = "playwright")]
mod local_site;

use seleniumbase_rs::stealth::evasions::{bootstrap_script, launch_args};
use seleniumbase_rs::{
    CanvasNoise, Dimension, Fingerprint, OsType, Result, SeleniumBaseError, Spoof, StealthPolicy,
};

#[tokio::main]
async fn main() -> Result<()> {
    let personas = [
        ("windows", Fingerprint::windows_desktop()),
        ("edge", Fingerprint::edge_desktop()),
        ("macos", Fingerprint::macos_desktop()),
        ("linux", Fingerprint::linux_desktop()),
        ("android", Fingerprint::android_mobile()),
        ("ios", Fingerprint::ios_mobile_safari()),
    ];
    for (name, persona) in &personas {
        let report = persona.validate();
        println!(
            "{name:<8} coherent={} errors={:?} warnings={}",
            report.is_coherent(),
            report.errors,
            report.warnings.len()
        );
        if !report.is_coherent() || !report.warnings.is_empty() {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "the {name} preset should be coherent, with nothing to warn about"
            )));
        }
    }

    // One call per intent. A time zone is given; the position is drawn in that
    // time zone's city; WebRTC is turned off; the policy delivers the claims.
    let berlin = Fingerprint::builder()
        .timezone("Europe/Berlin")
        .screen(1920, 1080)
        .geolocation(Spoof::Random)
        .webrtc(Spoof::Off)
        .policy(StealthPolicy::balanced())
        .canvas_noise(CanvasNoise::Random)
        .build()?;
    let place = berlin.geolocation().value().expect("a drawn position");
    println!(
        "berlin: position {:.2}, {:.2}; masks the time zone: {}, the screen: {}; WebRTC off: {}",
        place.latitude,
        place.longitude,
        berlin.is_masked(Dimension::Timezone),
        berlin.is_masked(Dimension::Screen),
        berlin.webrtc().is_off(),
    );
    if !berlin.is_masked(Dimension::Timezone) || (place.latitude - 52.5).abs() > 0.5 {
        return Err(SeleniumBaseError::AssertionFailed(
            "a drawn position should be in the city of the given time zone".to_owned(),
        ));
    }

    // A built fingerprint is not changed: take the builder back and build again.
    let tokyo = berlin
        .to_builder()
        .timezone("Asia/Tokyo")
        .geolocation((35.68, 139.65))
        .build()?;
    println!("tokyo: {:?}", tokyo.timezone().as_str());

    // An `Off` where it means nothing is refused, and says which setting.
    match Fingerprint::builder().timezone(Spoof::Off).build() {
        Err(error) => println!("refused: {error}"),
        Ok(_) => {
            return Err(SeleniumBaseError::AssertionFailed(
                "a time zone has no off state".to_owned(),
            ))
        }
    }

    // Mixing a Windows persona with a macOS platform is exactly what a
    // detector looks for.
    let mismatched = Fingerprint::windows_desktop()
        .to_builder()
        .platform("MacIntel")
        .build()?;
    let report = mismatched.validate();
    println!("mismatched persona: {:?}", report.errors);
    if report.is_coherent() {
        return Err(SeleniumBaseError::AssertionFailed(
            "a Windows user agent on MacIntel should be reported".to_owned(),
        ));
    }

    // The same seed makes the same persona, so a run can be reproduced.
    let first = Fingerprint::randomized(OsType::Windows, 42);
    if first != Fingerprint::randomized(OsType::Windows, 42) {
        return Err(SeleniumBaseError::AssertionFailed(
            "the same seed should give the same persona".to_owned(),
        ));
    }
    println!(
        "seed 42 persona: {}",
        first.user_agent().as_str().unwrap_or("no user agent")
    );

    let persona = Fingerprint::windows_desktop();
    println!("launch flags: {}", launch_args(&persona).len());
    println!("page script: {} bytes", bootstrap_script(&persona).len());

    #[cfg(feature = "playwright")]
    launch_with(&persona).await?;
    #[cfg(not(feature = "playwright"))]
    println!("\nPart 2 needs the playwright feature:\n  cargo run --example stealth_fingerprint --features playwright");
    Ok(())
}

/// Part 2: what page JavaScript sees under the persona.
#[cfg(feature = "playwright")]
async fn launch_with(persona: &Fingerprint) -> Result<()> {
    use local_site::{ensure, Site, DEMO_PATH};
    use seleniumbase_rs::browser::playwright::PlaywrightSession;

    let url = Site::serve_demo()?.url(DEMO_PATH);
    let session = PlaywrightSession::launch_headless_with_fingerprint(persona).await?;
    let outcome = async {
        session.goto(&url).await?;
        // The launch applied the script to the blank first page; apply it to
        // this one too.
        session.evaluate(&bootstrap_script(persona)).await?;

        let user_agent = session.evaluate("navigator.userAgent").await?;
        let platform = session.evaluate("navigator.platform").await?;
        let width = session.evaluate("screen.width").await?;
        let height = session.evaluate("screen.height").await?;
        println!("\nuserAgent: {user_agent}");
        println!("platform: {platform}");
        println!("screen: {width}x{height}");

        let screen = persona.screen().value().copied();
        ensure(
            platform.as_str() == persona.platform().as_str(),
            "navigator.platform matches the persona",
        )?;
        ensure(
            width.as_u64() == screen.map(|s| u64::from(s.width))
                && height.as_u64() == screen.map(|s| u64::from(s.height)),
            "screen size matches the persona",
        )
    }
    .await;
    let closed = session.close().await;
    outcome.and(closed)
}
