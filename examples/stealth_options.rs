//! `StealthOptions`: the Chrome launch flags behind undetected (UC) mode.
//!
//! `StealthOptions` collects the choices that reduce a browser's automation
//! fingerprint (headless or not, window size, user agent, locale, proxy, UC
//! flags, ...) and turns them into Chromium command-line arguments with
//! `args()`. `BaseCase` builds one from your [`BrowserConfig`] every time it
//! starts a session, so the arguments printed for `StealthOptions::from(&config)`
//! are the ones a launch would use.
//!
//! * Part 1 builds options by hand, prints the arguments, and checks that the
//!   important ones are there. It needs no browser.
//! * Part 2, with `--launch`, starts a UC-mode session from a `BrowserConfig`,
//!   opens a local page, and reads the user agent and `navigator.webdriver`
//!   back from page JavaScript to prove the options took effect.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example stealth_options              # part 1
//! cargo run --example stealth_options -- --launch  # parts 1 and 2
//! ```
//!
//! Part 2 needs Chrome and a matching chromedriver on your `PATH` (`sbrs
//! doctor` checks both); neither part needs the network. The page is served
//! from `127.0.0.1` by `local_site/mod.rs`.

mod local_site;

use local_site::{ensure, Site, DEMO_PATH};
use seleniumbase_rs::stealth::dprocess::find_chromedriver;
use seleniumbase_rs::stealth::options::StealthOptions;
use seleniumbase_rs::{run_browser_test, BaseCase, BrowserConfig, DriverMode, Result};

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                          (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36";

#[tokio::main]
async fn main() -> Result<()> {
    options_by_hand()?;

    if std::env::args().any(|arg| arg == "--launch") {
        let url = Site::serve_demo()?.url(DEMO_PATH);
        let config = BrowserConfig {
            headless: true,
            user_agent: Some(USER_AGENT.to_owned()),
            mode: DriverMode::Uc,
            ..BrowserConfig::default()
        };
        run_browser_test(config, |sb| Box::pin(launched(sb, url))).await?;
    } else {
        println!("\n(pass --launch to start a browser with these options)");
    }
    Ok(())
}

/// Part 1: options to arguments, with no browser.
fn options_by_hand() -> Result<()> {
    let options = StealthOptions {
        uc: true,
        headless: true,
        user_agent: Some(USER_AGENT.to_owned()),
        window_size: Some("1920,1080".to_owned()),
        ..StealthOptions::default()
    };

    println!(
        "chromedriver on PATH: {}",
        find_chromedriver()
            .map_or_else(|| "not found".to_owned(), |path| path.display().to_string())
    );
    let args = options.args()?;
    println!("{} launch arguments:", args.len());
    for arg in &args {
        println!("  {arg}");
    }

    for expected in [
        "--headless=new",
        "--window-size=1920,1080",
        "--disable-blink-features=AutomationControlled",
    ] {
        ensure(
            args.iter().any(|arg| arg == expected),
            &format!("the arguments include {expected}"),
        )?;
    }
    ensure(
        args.iter()
            .any(|arg| arg.strip_prefix("--user-agent=") == Some(USER_AGENT)),
        "the arguments carry the user agent",
    )?;
    // UC mode also drops the flag that puts the "controlled by automated
    // software" banner up; it is excluded rather than passed, so it is absent.
    ensure(
        !args.iter().any(|arg| arg.contains("enable-automation")),
        "enable-automation is not passed",
    )?;

    // What `BaseCase` would launch with for a config.
    let config = BrowserConfig::default().with_mode(DriverMode::Uc);
    let from_config = StealthOptions::from(&config).args()?;
    println!(
        "\nfrom a default UC config: {} launch arguments",
        from_config.len()
    );
    Ok(())
}

/// Part 2: what the page sees.
async fn launched(sb: &mut BaseCase, url: String) -> Result<()> {
    sb.open(&url).await?;
    let user_agent = sb.execute_script("return navigator.userAgent").await?;
    let webdriver = sb.execute_script("return navigator.webdriver").await?;
    println!("\nnavigator.userAgent: {user_agent}");
    println!("navigator.webdriver: {webdriver}");

    ensure(
        user_agent.as_str() == Some(USER_AGENT),
        "the page reports the configured user agent",
    )?;
    ensure(
        webdriver.as_bool() != Some(true),
        "navigator.webdriver is not true",
    )
}
