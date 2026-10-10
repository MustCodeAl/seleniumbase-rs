//! The ad-blocking extension.

/// The "Simple Ad Blocker" extension, as a zip archive.
///
/// A manifest v3 extension with the `declarativeNetRequest` permission and one
/// ruleset that blocks requests to `doubleclick.net` and
/// `googleadservices.com`. Unpack it with
/// [`extract_extension`](super::extract_extension) to load it into Chrome.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::utils::extensions::get_ad_block_bytes;
///
/// assert!(get_ad_block_bytes().starts_with(b"PK"));
/// ```
#[must_use]
pub fn get_ad_block_bytes() -> &'static [u8] {
    include_bytes!("assets/ad_block.zip")
}
