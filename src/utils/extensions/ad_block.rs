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
/// use seleniumbase_rs::utils::extensions::AD_BLOCK_BYTES;
///
/// assert!(AD_BLOCK_BYTES.starts_with(b"PK"));
/// ```
pub const AD_BLOCK_BYTES: &[u8] = include_bytes!("assets/ad_block.zip");
