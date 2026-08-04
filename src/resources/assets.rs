use std::path::PathBuf;

/// Directory where external assets are stored relative to the crate root.
pub const ASSETS_DIR: &str = "assets";

/// Return the path to an asset by name.
pub fn asset_path(name: &str) -> PathBuf {
    PathBuf::from(ASSETS_DIR).join(name)
}

/// Return the expected path to a bundled chromedriver binary.
pub fn chromedriver_asset() -> PathBuf {
    asset_path("chromedriver")
}
