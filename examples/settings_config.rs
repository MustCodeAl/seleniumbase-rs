//! Layered settings: built-in defaults, a config file, `SB_*` environment
//! variables, explicit overrides, and the conversion to a `BrowserConfig`.
//!
//! The layers are applied from lowest to highest priority. The example builds a
//! config file in a scratch folder (so it never reads or writes yours), then
//! shows each layer on top of the last and checks the result:
//!
//! 1. `Settings::default()`, the built-in values.
//! 2. `Settings::from_file(path)`, a TOML or JSON file you name.
//! 3. `Settings::load_global()`, which looks for `sbase_config.toml`,
//!    `.sbase_config.toml` or `.sbase_config` in the working directory.
//! 4. `Settings::load_from(path)`, the file plus any `SB_*` environment
//!    variables, which win over it.
//! 5. `SettingsOverrides::apply`, what a command line asked for explicitly,
//!    which wins over everything.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example settings_config
//! SB_HEADLESS=false SB_TIMEOUT=5 cargo run --example settings_config
//! ```
//!
//! The second command shows layer 4 changing the answer. The example needs no
//! browser and no network. It changes the working directory to the scratch
//! folder for layer 3, which is harmless in this single-threaded program.

use std::error::Error;
use std::fs;

use seleniumbase_rs::config::settings::{Settings, SettingsOverrides};

const CONFIG_FILE: &str = r#"
browser = "chromium"
headless = true
timeout_seconds = 12
locale = "de-DE"
mode = "cdp"
"#;

fn main() -> Result<(), Box<dyn Error>> {
    // 1. Built-in defaults.
    let defaults = Settings::default();
    show("defaults", &defaults);

    let dir = tempfile::tempdir()?;
    let path = dir.path().join("sbase_config.toml");
    fs::write(&path, CONFIG_FILE)?;

    // 2. A specific file. Keys the file leaves out keep their defaults.
    let from_file = Settings::from_file(&path)?;
    show("from_file", &from_file);
    if from_file.browser != "chromium" || from_file.window_width != defaults.window_width {
        return Err("the file should override browser and leave window_width alone".into());
    }

    // 3. The global file, found by name in the working directory.
    std::env::set_current_dir(dir.path())?;
    let global = Settings::load_global()?;
    if global != from_file {
        return Err("load_global should find sbase_config.toml".into());
    }

    // 4. The file, then the environment. Run with SB_HEADLESS or SB_TIMEOUT
    //    set to see them win.
    let overridden: Vec<String> = std::env::vars()
        .map(|(name, _)| name)
        .filter(|name| name.starts_with("SB_"))
        .collect();
    println!("SB_* variables in this environment: {overridden:?}");
    let mut settings = Settings::load_from(&path)?;
    show("load (file + environment)", &settings);

    // 5. Explicit overrides win over the file and the environment.
    let overrides = SettingsOverrides {
        headless: Some(false),
        threads: Some(4),
        ..SettingsOverrides::default()
    };
    overrides.apply(&mut settings);
    show("after explicit overrides", &settings);
    if settings.headless || settings.threads != Some(4) {
        return Err("explicit overrides should win".into());
    }

    // The same overrides can be handed to a child process as variables.
    println!("as environment variables: {:?}", overrides.env());

    // Finally, the BrowserConfig that BaseCase::new takes.
    let config = settings.to_browser_config();
    println!(
        "BrowserConfig: browser {:?}, mode {:?}, headless {}",
        config.browser, config.mode, config.headless
    );
    Ok(())
}

fn show(layer: &str, settings: &Settings) {
    println!(
        "{layer}: browser={}, headless={}, timeout={:?}s, locale={:?}, mode={:?}",
        settings.browser,
        settings.headless,
        settings.timeout_seconds,
        settings.locale,
        settings.mode
    );
}
