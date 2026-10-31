// The contract file of the fixture consumer, with every path it uses.

use seleniumbase_rs::error::SeleniumBaseError;
use seleniumbase_rs::evasion::fingerprint::{self, Fingerprint, OsType};
use seleniumbase_rs::imaginary::Widget;
use seleniumbase_rs::sb_cdp::{Browser, Page};
use seleniumbase_rs::utils::urls::normalize_page_url;

fn pins() {
    let _ = seleniumbase_rs::sb_cdp::Proxy::has_credentials;
    let _ = seleniumbase_rs::sb_cdp::Page::goto;
    let _ = seleniumbase_rs::ProxyConfig::default();
    let _: Option<seleniumbase_rs::ProxyConfig> = None;
}
