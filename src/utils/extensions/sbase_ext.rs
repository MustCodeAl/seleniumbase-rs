pub fn get_sbase_ext_bytes() -> &'static [u8] {
    include_bytes!("assets/sbase_ext.zip")
}
pub fn get_firefox_addon_bytes() -> &'static [u8] {
    include_bytes!("assets/firefox_addon.xpi")
}
