//! Build a client from the measured Chrome 155 emulation, override one part of it with
//! `CustomProfileBuilder`, and see what the stack accepts and what it refuses.
//!
//! Needs: the `impersonate` feature, which builds BoringSSL (`cmake` and a C/C++ toolchain), and a
//! system with a measured Chrome. Today that is Chrome 155 on macOS; on any other system the
//! example stops at once with the `NotMeasured` error and does not borrow the values of another
//! system. It opens no socket and sends nothing.
//!
//! ```text
//! cargo run --features impersonate --example custom_profile
//! ```
//!
//! A profile is a browser as data: the TLS `ClientHello`, the HTTP/2 start, the header lists and
//! the identity headers. The builder layers your inputs over the measured base and checks each
//! one against what BoringSSL and wreq can actually emit. What they cannot emit is refused with a
//! named error before a client exists, instead of being approximated into a different
//! fingerprint than the one you named.

mod stealth_support;

use std::error::Error;
use std::process::ExitCode;

use seleniumbase_rs::{
    impersonation_client, BrowserKind, CustomProfileBuilder, ImpersonateError, ImpersonationProfile,
};

/// An Akamai HTTP/2 fingerprint other than Chrome's: its own SETTINGS, connection window,
/// PRIORITY frames and pseudo-header order.
const OTHER_HTTP2_START: &str = "1:65536;4:131072;5:16384|12517377|3:0:0:201,5:0:0:101|m,p,a,s";

/// An Akamai string with a GREASE SETTINGS id (2570), which wreq cannot send.
const GREASE_SETTING: &str = "2570:1;4:1048576|0|0|m,a,s,p";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("custom_profile: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let (os, _major) = stealth_support::measured_chrome()?;
    let base = ImpersonationProfile::for_system(BrowserKind::Chrome, os)?;
    let ja3 = base.tls_spec().ja3().to_string();
    println!("the measured base: {}", base.source());
    println!("  user agent  {}", base.user_agent());
    println!("  JA4         {}", base.tls_spec().ja4());
    println!("  JA3         {ja3}");
    println!("  HTTP/2      {}", base.http2_spec().akamai());
    println!(
        "  not sendable by BoringSSL, so left out of the profile: {:?}",
        base.known_gaps()
    );

    // A builder over the same measured base. Each input is checked when it is given.
    let custom = || ImpersonationProfile::custom().base_on(BrowserKind::Chrome, os);

    println!("\naccepted");
    let other_h2 = custom().akamai(OTHER_HTTP2_START)?.build()?;
    println!(
        "  an Akamai string:  HTTP/2 -> {}",
        other_h2.http2_spec().akamai()
    );
    println!(
        "                     the TLS spec is still the base's: {}",
        other_h2.tls_spec() == base.tls_spec()
    );

    // The two oldest RSA key-exchange suites left out of the hello.
    let smaller = ja3.replacen("-47-53,", ",", 1);
    if smaller == ja3 {
        return Err(
            "the base JA3 no longer ends its ciphers with 47-53; update this example".into(),
        );
    }
    let fewer_ciphers = custom().ja3(&smaller)?.build()?;
    println!(
        "  a JA3 string:      JA3 -> {}",
        fewer_ciphers.tls_spec().ja3()
    );
    println!(
        "                     extension shuffling {} -> {} (a JA3 pins the order, so it arrives as given)",
        base.tls_spec().permute_extensions,
        fewer_ciphers.tls_spec().permute_extensions
    );

    // Chrome 155 offers `trust_anchors` (0xCA34); BoringSSL as pinned here cannot send it.
    let with_trust_anchors = ja3.replacen("-43-", "-51764-43-", 1);
    let refusals: [(&str, Result<CustomProfileBuilder, ImpersonateError>); 4] = [
        (
            "a JA3 with the trust_anchors extension",
            custom().ja3(&with_trust_anchors),
        ),
        (
            "a signature algorithm BoringSSL cannot write",
            custom().signature_algorithms(&[0x0904]),
        ),
        (
            "an Akamai string with a GREASE setting",
            custom().akamai(GREASE_SETTING),
        ),
        ("a JA3 that is not well formed", custom().ja3("771,4865")),
    ];
    println!("\nrefused before any client exists");
    for (what, result) in refusals {
        match result {
            Ok(_) => println!("  {what}: accepted, which this example did not expect"),
            Err(error) => println!("  {what}:\n      {error}"),
        }
    }

    // A profile becomes a wreq client builder. Building the client sends nothing.
    let client = impersonation_client(&other_h2)?.build()?;
    drop(client);
    println!("\nbuilt a wreq client from the HTTP/2 override; nothing was sent");
    Ok(())
}
