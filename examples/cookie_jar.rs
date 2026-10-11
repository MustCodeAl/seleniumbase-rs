//! The RFC 6265 cookie jar: store `Set-Cookie` headers, see which ones it refuses and why, and read
//! back the `Cookie` header it would send.
//!
//! Needs: the `scrape` feature, which is pure Rust. No browser, no network and no measured system:
//! the jar runs the same on every platform.
//!
//! ```text
//! cargo run --features scrape --example cookie_jar
//! ```
//!
//! The headers below are made up. The jar applies the storage model of RFC 6265 section 5.3 with
//! the RFC 6265bis rules Chrome ships, and checks domains against the Public Suffix List.

mod stealth_support;

use std::error::Error;

use seleniumbase_rs::scrape::jar::store::SiteContext;
use seleniumbase_rs::CookieJar;
use url::Url;

/// A made-up `Set-Cookie` header and what it is meant to show.
const HEADERS: [(&str, &str); 8] = [
    (
        "a session cookie for this host only",
        "sid=abc123; Path=/; Secure; HttpOnly; SameSite=Lax",
    ),
    (
        "a cookie for the whole site",
        "theme=dark; Domain=example.com; Path=/; Max-Age=3600",
    ),
    ("a cookie for one path", "cart=3; Path=/cart; Max-Age=600"),
    (
        "a domain on a public suffix (a supercookie)",
        "track=1; Domain=com",
    ),
    (
        "a domain of another site",
        "other=1; Domain=evil.example.org",
    ),
    (
        "a __Host- cookie that names a domain",
        "__Host-token=x; Secure; Path=/; Domain=example.com",
    ),
    ("SameSite=None without Secure", "embed=1; SameSite=None"),
    ("a cookie that expires at once", "old=1; Max-Age=0; Path=/"),
];

/// A request to build a `Cookie` header for, and where it comes from.
const REQUESTS: [(&str, &str, SiteContext); 5] = [
    (
        "a page under /cart",
        "https://shop.example.com/cart/items",
        SiteContext::SameSite { top_level: true },
    ),
    (
        "another host of the site",
        "https://www.example.com/",
        SiteContext::SameSite { top_level: true },
    ),
    (
        "the shop over plain http",
        "http://shop.example.com/",
        SiteContext::SameSite { top_level: true },
    ),
    (
        "from another site, not a navigation",
        "https://shop.example.com/",
        SiteContext::CrossSite {
            top_level_get: false,
        },
    ),
    (
        "a navigation from another site",
        "https://shop.example.com/",
        SiteContext::CrossSite {
            top_level_get: true,
        },
    ),
];

fn main() -> Result<(), Box<dyn Error>> {
    // Every function of the jar that needs the time takes it: seconds since the Unix epoch.
    let now = stealth_support::unix_now();
    let shop = Url::parse("https://shop.example.com/cart")?;
    let mut jar = CookieJar::new();

    println!("Set-Cookie headers received from {shop}");
    for (what, header) in HEADERS {
        let outcome = format!("{:?}", jar.store(&shop, header, now));
        println!("  {outcome:<34} {what}");
    }

    println!(
        "\nthe jar holds {} cookies (their values are not shown)",
        jar.len()
    );
    for cookie in jar.cookies() {
        let scope = if cookie.host_only {
            "this host only"
        } else {
            "the domain and its subdomains"
        };
        println!(
            "  {:<6} {:<18} path {:<6} secure {:<5} {scope}",
            cookie.name, cookie.domain, cookie.path, cookie.secure,
        );
    }

    println!("\nthe Cookie header for a request");
    for (what, url, context) in REQUESTS {
        let url = Url::parse(url)?;
        let header = jar.header_for(&url, context, now);
        println!(
            "  {what:<38} {}",
            header.as_deref().unwrap_or("(no Cookie header)")
        );
    }
    Ok(())
}
