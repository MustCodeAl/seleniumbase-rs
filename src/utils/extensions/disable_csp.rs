//! The extension that turns a site's Content-Security-Policy off.

/// The "Disable CSP" extension, as a zip archive.
///
/// A manifest v3 extension that adds a `declarativeNetRequest` rule removing
/// the `content-security-policy`, `x-content-security-policy` and
/// `x-webkit-csp` response headers from page and frame loads, so scripts the
/// page's policy would refuse can run. Clicking its toolbar button switches
/// the rule on and off. Unpack it with
/// [`extract_extension`](super::extract_extension) to load it into Chrome.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::utils::extensions::get_disable_csp_bytes;
///
/// assert!(get_disable_csp_bytes().starts_with(b"PK"));
/// ```
#[must_use]
pub fn get_disable_csp_bytes() -> &'static [u8] {
    include_bytes!("assets/disable_csp.zip")
}
