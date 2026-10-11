//! Build anti-detection fingerprints (personas), check that they are
//! consistent, and optionally launch a browser that presents one.
//!
//! A [`Fingerprint`] describes the browser a page should believe it is talking
//! to: user agent, platform, screen, GPU, languages, time zone, and more. Part 1
//! needs no browser:
//!
//! * the ready-made personas (`windows_desktop`, `macos_desktop`,
//!   `linux_desktop`, `android_mobile`, `ios_mobile_safari`),
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
use seleniumbase_rs::{Fingerprint, OsType, Result, SeleniumBaseError};

#[tokio::main]
async fn main() -> Result<()> {
    let personas = [
        ("windows", Fingerprint::windows_desktop()),
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
        if !report.is_coherent() {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "the {name} preset should be coherent"
            )));
        }
    }

    // Mixing a Windows persona with a macOS platform is exactly what a
    // detector looks for.
    let mut mismatched = Fingerprint::windows_desktop();
    mismatched.platform = Some("MacIntel".to_owned());
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
        first.user_agent.as_deref().unwrap_or("no user agent")
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

        ensure(
            platform.as_str() == persona.platform.as_deref(),
            "navigator.platform matches the persona",
        )?;
        ensure(
            width.as_u64() == persona.screen_width.map(u64::from)
                && height.as_u64() == persona.screen_height.map(u64::from),
            "screen size matches the persona",
        )
    }
    .await;
    let closed = session.close().await;
    outcome.and(closed)
}
