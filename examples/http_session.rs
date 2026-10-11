//! Fetch from a loopback TLS shop with an `HttpSession`: plain HTTP requests that carry the
//! measured Chrome 155 handshake and headers, and a cookie jar that follows what the shop sets.
//!
//! Needs: the `impersonate` feature, which builds BoringSSL (`cmake` and a C/C++ toolchain), and a
//! system with a measured Chrome. Today that is Chrome 155 on macOS; on any other system the
//! example stops at once with the `NotMeasured` error. It needs no browser and no outside
//! network: the shop is started here, on `127.0.0.1`, with a certificate from an in-memory
//! authority that only this session trusts.
//!
//! ```text
//! cargo run --features impersonate --example http_session
//! ```
//!
//! The shop (`examples/stealth_support/shop.rs`) plays a site behind a script challenge. An
//! `HttpSession` does not run scripts, so it cannot pass the challenge itself. In real use a
//! browser does that and hands its cookies over (`HttpSession::from_page`). Here the example
//! calls the endpoint the challenge script would call, to show the jar carrying the cookie it
//! gets on to the next request. The shop speaks HTTP/1.1 only.
//!
//! The JA4 the shop records is the client's own. It is not the browser's: the pinned BoringSSL
//! cannot send everything Chrome 155 sends, so the two differ where the known gaps say (see
//! "Limits" in `docs/tutorials/stealth_scraping.md`), and the example prints both.

mod stealth_support;

use std::error::Error;
use std::process::ExitCode;

use seleniumbase_rs::scrape::EmulationRef;
use seleniumbase_rs::wire::emulation;
use seleniumbase_rs::{BrowserKind, Fingerprint, HttpSession, StealthIdentity};
use stealth_support::shop::Shop;

/// The seed of the made-up identity. The same seed gives the same identity.
const SEED: u64 = 7;

/// The requests, in order, and why each one is made.
const STEPS: [(&str, &str); 4] = [
    ("/open", "an unguarded page"),
    ("/", "the guarded page, without the cookie"),
    ("/__solve", "the endpoint the challenge script calls"),
    ("/", "the guarded page again"),
];

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("http_session: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    let (os, major) = stealth_support::measured_chrome()?;
    let shop = Shop::start().await?;

    // An identity that claims the measured Chrome: its fingerprint becomes the user agent, the
    // client hints and the languages of every request.
    let fingerprint = Fingerprint::randomized_with_major(os, SEED, major);
    let emulation_ref = EmulationRef {
        kind: BrowserKind::Chrome,
        major,
        os,
    };
    let identity = StealthIdentity::new(
        fingerprint,
        emulation_ref,
        None,
        stealth_support::unix_now(),
    )?;
    let session = HttpSession::from_identity(&identity, None, shop.http_options()?)?;
    println!("profile {}", session.profile().source());

    for (path, what) in STEPS {
        let reply = session.get(&shop.url(path)).await?;
        println!(
            "GET {path:<9} -> {} over {:?}, challenge {:?}, {} cookie(s) in the jar   ({what})",
            reply.status(),
            reply.protocol(),
            reply.detect().kind,
            session.cookie_count(),
        );
    }

    println!("\nwhat the shop saw");
    let seen = shop.seen();
    for request in &seen {
        println!(
            "  {:<9} cookies {:?}  JA4 {}",
            request.path,
            request.cookie_names(),
            request.ja4,
        );
    }
    if let Some(agent) = seen
        .first()
        .and_then(|request| request.user_agent.as_deref())
    {
        println!("  user agent: {agent}");
    }

    let measured = emulation::get(BrowserKind::Chrome, os)?.tls.ja4();
    println!("\nJA4 of the measured Chrome {major} (the client cannot send all of it): {measured}");
    println!(
        "left out of the profile because BoringSSL cannot send it: {:?}",
        session.profile().known_gaps()
    );
    Ok(())
}
