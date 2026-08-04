use seleniumbase_rs::config::settings::Settings;

/// Demonstrates loading SeleniumBase settings from a global config file,
/// a specific file, or environment variables.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load from the default global config file (sbase_config.toml,
    // .sbase_config.toml, or .sbase_config) if one exists.
    let settings = Settings::load::<&str>(None)?;
    println!("Loaded settings: {:?}", settings);

    // Convert settings into a BrowserConfig and spawn a BaseCase.
    // let config = settings.to_browser_config();
    // let mut sb = seleniumbase_rs::BaseCase::new(config).await?;

    Ok(())
}
