//! The extensions that exist to test extension loading.

/// The "SBase Extension" Chromium extension, as a zip archive.
///
/// A manifest v3 extension that shows a notification when it is installed and
/// otherwise does nothing. SeleniumBase uses it to test that an extension
/// loads. Unpack it with [`extract_extension`](super::extract_extension) to
/// load it into Chrome.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::utils::extensions::get_sbase_ext_bytes;
///
/// assert!(get_sbase_ext_bytes().starts_with(b"PK"));
/// ```
#[must_use]
pub fn get_sbase_ext_bytes() -> &'static [u8] {
    include_bytes!("assets/sbase_ext.zip")
}

/// A signed Firefox add-on (`.xpi`) that does nothing a person would notice.
///
/// It puts a `div` with the id `webextensions-selenium-example` into every
/// web page, which is how a test can tell the add-on loaded. The archive
/// carries Mozilla's signature, so it must be passed to Firefox as it is, not
/// unpacked and re-zipped.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::utils::extensions::get_firefox_addon_bytes;
///
/// assert!(get_firefox_addon_bytes().starts_with(b"PK"));
/// ```
#[must_use]
pub fn get_firefox_addon_bytes() -> &'static [u8] {
    include_bytes!("assets/firefox_addon.xpi")
}
