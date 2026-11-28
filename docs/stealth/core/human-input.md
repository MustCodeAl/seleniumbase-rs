---
title: "Human-like input"
sidebarTitle: "Human-like input"
---

Detection scripts watch how input arrives as well as what the browser claims:
events that are not trusted, a pointer that jumps straight to its target, keys
typed at a constant rate. The pure CDP engine sends trusted input through the
DevTools Protocol, and UC mode has `uc_click` and `uc_type` with human timing
(see [UC mode](/stealth/core/uc-mode)). This page is the model behind the pacing.


Detectors look for input that is too regular. The
`sb_cdp::human::behavior` module plans input that is not: curved
pointer paths with a bell-shaped speed profile, Fitts's-law timing, tremor and
overshoot, and Gaussian typing with word and sentence pauses. It is pure and
seedable:

```rust
use seleniumbase_rs::sb_cdp::human::behavior::{Behavior, Humanizer};
use seleniumbase_rs::sb_cdp::Point;

fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    let mut person = Humanizer::new(Behavior::builder().typing_wpm(70.0).seed(7).build()?);
    let path = person.mouse_path(Point::new(0.0, 0.0), Point::new(200.0, 90.0), 60.0);
    let keys = person.typing_plan("hello");
    assert_eq!(path.last().map(|step| step.at), Some(Point::new(200.0, 90.0)));
    assert_eq!(keys.len(), 5);
    Ok(())
}
```

To play a plan out on a page, use `page.human(behavior)` from the Pure CDP
engine (see [Pure CDP](/guides/browser-engines/pure-cdp#human-paced-input)). On `BaseCase`,
`sb.human_click(selector)` and `sb.human_type(selector, text)` dispatch
human-like Bézier cursor movements and natural Gaussian keystroke pacing (50-120ms
intervals with natural jitter), taking their pace from `Fingerprint::humanize()`
when the configuration has a fingerprint.

`Behavior::from_config(fingerprint.humanize())` turns a fingerprint's
`HumanizeConfig` (its minimum and maximum keystroke delay) into the matching
behaviour:

<CodeGroup>

```rust,no_run Pure CDP with Fingerprint humanize pacing
use seleniumbase_rs_stealth::client::{LaunchOptionsFingerprintExt, StealthLayer};
use seleniumbase_rs_stealth::identity::{Fingerprint, OsType};
use seleniumbase_rs_stealth::sb_cdp::human::behavior::{Behavior, Humanizer};
use seleniumbase_rs_stealth::sb_cdp::{Browser, LaunchOptions};

async fn fill_with_human_pacing() -> Result<(), seleniumbase_rs_stealth::SeleniumBaseError> {
    let fp = Fingerprint::randomized(OsType::Macos, 42);
    let mut person = Humanizer::new(Behavior::from_config(fp.humanize())?);
    let options = LaunchOptions::builder()
        .build()?
        .with_stealth(&StealthLayer::new(fp))?;

    let browser = Browser::launch(options).await?;
    let page = browser.default_page().await?;
    page.goto("https://example.com").await?;
    page.human(&mut person).click("#email").await?;
    page.human(&mut person).type_text("#email", "alice@example.com").await?;
    browser.close().await
}
```

```rust,no_run BaseCase human click and typing
use seleniumbase_rs_stealth::client::BrowserConfigFingerprintExt;
use seleniumbase_rs_stealth::identity::{Fingerprint, OsType};
use seleniumbase_rs_stealth::{BaseCase, BrowserConfig, DriverMode};

async fn uc_human_input() -> Result<(), seleniumbase_rs_stealth::SeleniumBaseError> {
    let fp = Fingerprint::randomized(OsType::Windows, 9);
    let config = BrowserConfig::builder()
        .mode(DriverMode::Uc)
        .build()?
        .with_fingerprint(&fp)?;

    let mut sb = BaseCase::new(config).await?;
    sb.open("https://example.com").await?;
    sb.human_click("#email").await?;
    sb.human_type("#email", "alice@example.com").await?;
    sb.quit().await
}
```

</CodeGroup>

<Tip>

The older `sb_cdp::human::humanize` helpers (`bezier_mouse_path`,
`keystroke_delays`) remain, but they sample a Bézier curve at evenly spaced
parameter values and draw delays uniformly between two bounds; prefer
`behavior`.

</Tip>

