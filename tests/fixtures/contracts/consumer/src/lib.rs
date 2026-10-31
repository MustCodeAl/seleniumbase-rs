// A made-up consumer of this crate, for `tests/contract_script.rs`: it is read as text by
// `scripts/check-consumers.sh` and never compiled.
//
// Every shape of import the script has to read: a group over two lines with a nested group and
// an `as`, a `self` in a group, a one-line group, a plain path, a path in an expression, a path
// in a rustdoc link, and a list the program writes at run time.

use seleniumbase_rs::{
    error::SeleniumBaseError,
    sb_cdp::{Browser, Page as Tab},
};
use seleniumbase_rs::evasion::fingerprint::{self, Fingerprint, OsType};
use seleniumbase_rs::utils::urls::normalize_page_url;
use seleniumbase_rs::imaginary::Widget;

/// Reads a page like [`Page::goto`](seleniumbase_rs::sb_cdp::Page::goto) does.
fn calls() {
    let _ = seleniumbase_rs::sb_cdp::Proxy::has_credentials;
    let _ = seleniumbase_rs::ProxyConfig {
        host: String::new(),
        ..seleniumbase_rs::ProxyConfig::default()
    };
    // A program that writes its own import list is not read: `use seleniumbase_rs::sb_cdp::{${names}};`
}
