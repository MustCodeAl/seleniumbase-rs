//! Drive a `StealthSession` against a loopback TLS shop that guards its page with a script
//! challenge, and print how the session moved between plain HTTP and a real Chrome.
//!
//! Needs: the `impersonate` feature, which builds BoringSSL (`cmake` and a C/C++ toolchain); a
//! system with a measured Chrome; and that Chrome installed, headless, with the same major
//! version as the measurement. Today that is Chrome 155 on macOS. On any other system the example
//! stops at once with the `NotMeasured` error, and a Chrome of another major version is refused
//! before it loads a page. The shop is started here on `127.0.0.1`, so no outside network is
//! used. Only one Chrome-driving program should run on the machine at a time.
//!
//! ```text
//! cargo run --features impersonate --example stealth_session
//! ```
//!
//! What happens:
//!
//! 1. The session asks the shop for `/` over HTTP and meets the challenge page.
//! 2. It starts a MITM proxy and a headless Chrome behind it, with the same identity.
//! 3. Chrome runs the challenge page's script, which earns the clearance cookie, and reloads.
//! 4. The cookies move from Chrome into the identity, Chrome is closed, and the session asks
//!    again over HTTP with the clearance.
//!
//! The shop (`examples/stealth_support/shop.rs`) is a stand-in written for this example. It is
//! not any vendor's challenge, and passing it says nothing about how a real service behaves.

mod stealth_support;

use std::collections::BTreeSet;
use std::error::Error;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use seleniumbase_rs::sb_cdp::LaunchOptions;
use seleniumbase_rs::{CollectingEventSink, ScraperEvent, StealthSession};
use stealth_support::shop::{Shop, REAL_MARKER};

/// How long Chrome is given to run the challenge page's script.
const SOLVE_WITHIN: Duration = Duration::from_secs(30);

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("stealth_session: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    // Stop early, with a clear message, on a system that was never measured.
    stealth_support::measured_chrome()?;
    let shop = Shop::start().await?;
    let sink = CollectingEventSink::new();
    let mut session = StealthSession::builder()
        .launch_options(LaunchOptions::builder().headless(true))
        .http_options(shop.http_options()?)
        .mitm_options(shop.mitm_options()?)
        .with_event_sink(Arc::new(sink.clone()))
        .solve_timeout(SOLVE_WITHIN)
        .build()
        .await?;
    println!("the session starts in {:?} mode", session.mode());

    let fetched = session.fetch(&shop.url("/")).await;
    println!("the session ends in {:?} mode", session.mode());
    // Close the session whatever the fetch did: it may own a Chrome and a proxy.
    session.close().await?;
    let page = fetched?;

    println!(
        "\nfetched /: status {}, {} loads, served in {:?} mode, challenge solved on the way: {:?}",
        page.status, page.attempts, page.mode, page.solved
    );
    println!(
        "the page is the real one: {}",
        page.body.contains(REAL_MARKER)
    );

    println!("\nchanges of mode");
    for event in sink.events() {
        if let ScraperEvent::ModeChanged { from, to, reason } = event {
            println!("  {from:?} -> {to:?}   {reason:?}");
        }
    }

    println!("\nwhat the shop saw");
    let seen = shop.seen();
    for request in &seen {
        println!(
            "  {:<13} cookies {:?}",
            request.path,
            request.cookie_names()
        );
    }
    // The HTTP leg and Chrome, behind the proxy, reach the shop through the same client.
    let handshakes: BTreeSet<_> = seen.iter().map(|request| request.ja4.as_str()).collect();
    let agents: BTreeSet<_> = seen
        .iter()
        .filter_map(|request| request.user_agent.as_deref())
        .collect();
    println!(
        "  {} requests, {} distinct JA4, {} distinct user agent",
        seen.len(),
        handshakes.len(),
        agents.len()
    );
    Ok(())
}
