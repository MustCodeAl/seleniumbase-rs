//! The stealth layer against public bot-detection test pages.
//!
//! These pages grade a browser the way a site would, so they say more than any
//! single probe. They also change without notice, so the assertions are the
//! ones that can be stated without knowing how a page scores:
//!
//! - SannySoft lists named checks, each marked passed or failed, and none may
//!   fail;
//! - CreepJS gives percentages for how headless and how patched a browser
//!   looks, and the persona must look no more headless than the same Chrome
//!   with no fingerprint at all.
//!
//! The full reports are printed for a person to read (`--nocapture`).
//!
//! They need a Chrome or Chromium and the network, so they are ignored by
//! default and never run in CI. Run them one at a time:
//!
//! ```text
//! cargo test --features "mcp-server test-util" --test stealth_audit_network \
//!     -- --ignored --test-threads=1 --nocapture
//! ```
//!
//! Known from the first run (Chrome 155 on macOS, headless): CreepJS lists two
//! stealth findings, `hasToStringProxy` (its lie detector marks the replaced
//! `Function.prototype.toString`) and `hasBadWebGL` (a Worker still reports the
//! real renderer, because the page script does not run inside workers).

use std::time::{Duration, Instant};

use seleniumbase_rs::evasion::fingerprint::Fingerprint;
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, Page};
use seleniumbase_rs::OsType;
use serde_json::Value;

/// Seed of the persona every test applies.
const SEED: u64 = 20_261_011;

/// How long a test page gets to finish grading the browser.
const GRADING_TIME: Duration = Duration::from_secs(60);

/// How often the page is asked whether grading has finished.
const POLL: Duration = Duration::from_millis(500);

/// A macOS persona: the only OS whose values this project measures.
fn persona() -> Fingerprint {
    Fingerprint::randomized(OsType::Macos, SEED)
}

/// A headless Chrome, with the persona applied to every tab if one is given.
async fn browser(fingerprint: Option<&Fingerprint>) -> (Browser, Page) {
    let mut builder = LaunchOptions::builder()
        .headless(true)
        .no_sandbox(std::env::var_os("CI").is_some());
    if let Some(fingerprint) = fingerprint {
        builder = builder.fingerprint(fingerprint);
    }
    let browser = Browser::launch(builder.build().expect("valid launch options"))
        .await
        .expect("Chrome starts");
    let page = browser
        .default_page()
        .await
        .expect("a page")
        .with_timeout(Duration::from_secs(30));
    (browser, page)
}

/// Evaluates `probe` until it returns something other than `null`, or until
/// the grading time is over, and returns the last answer.
async fn until_ready(page: &Page, probe: &str) -> Value {
    let started = Instant::now();
    loop {
        let answer = page.evaluate(probe).await.unwrap_or(Value::Null);
        if !answer.is_null() || started.elapsed() > GRADING_TIME {
            return answer;
        }
        tokio::time::sleep(POLL).await;
    }
}

#[tokio::test]
#[ignore = "launches a real Chrome and needs the network"]
async fn sannysoft_fails_no_check() {
    let fingerprint = persona();
    let (browser, page) = browser(Some(&fingerprint)).await;
    page.goto("https://bot.sannysoft.com/")
        .await
        .expect("the test page loads");

    let rows = until_ready(
        &page,
        r##"(() => {
            const verdicts = Array.from(document.querySelectorAll('td'))
                .filter((td) => /\b(passed|failed|warn)\b/.test(td.className));
            if (verdicts.length < 20 || verdicts.some((td) => td.innerText.trim() === '')) {
                return null;
            }
            return verdicts.map((td) => {
                const first = td.parentElement.children[0];
                return {
                    name: first.innerText.replace(/\s+/g, ' ').trim(),
                    result: td.innerText.split('\n')[0].trim(),
                    failed: /\bfailed\b/.test(td.className),
                };
            });
        })()"##,
    )
    .await;
    browser.close().await.expect("Chrome closes");

    let rows = rows.as_array().expect("the page graded the browser");
    println!("SannySoft: {} checks", rows.len());
    let mut failed = Vec::new();
    for row in rows {
        let name = row["name"].as_str().unwrap_or_default();
        let result = row["result"].as_str().unwrap_or_default();
        let bad = row["failed"].as_bool().unwrap_or(false);
        println!("  {} {name}: {result}", if bad { "FAIL" } else { "ok  " });
        if bad {
            failed.push(format!("{name}: {result}"));
        }
    }
    assert!(failed.is_empty(), "SannySoft fails {failed:?}");
}

/// What CreepJS says about how a browser looks.
#[derive(Debug)]
struct Creep {
    like_headless: u32,
    headless: u32,
    stealth: u32,
}

/// Reads the rating on the line of CreepJS's report that reads
/// `<percent>% <label>: <hash>`.
fn rating(text: &str, label: &str) -> Option<u32> {
    text.lines().find_map(|line| {
        let (percent, rest) = line.trim().split_once("% ")?;
        rest.strip_prefix(label)?.starts_with(':').then_some(())?;
        percent.parse().ok()
    })
}

/// Loads CreepJS in a browser and reads its headless and stealth ratings.
async fn creep(fingerprint: Option<&Fingerprint>) -> Creep {
    let (browser, page) = browser(fingerprint).await;
    page.goto("https://abrahamjuliot.github.io/creepjs/")
        .await
        .expect("the test page loads");
    let text = until_ready(
        &page,
        r##"(() => {
            const text = document.body ? document.body.innerText : '';
            return /\d+% stealth:/.test(text) && text.includes('Navigator') && text.includes('Status')
                ? text : null;
        })()"##,
    )
    .await;
    browser.close().await.expect("Chrome closes");

    let text = text.as_str().expect("the page finished grading");
    let start = text.find("Headless").unwrap_or(0);
    println!(
        "CreepJS ({}):\n{}",
        if fingerprint.is_some() {
            "persona"
        } else {
            "plain Chrome"
        },
        &text[start..text.len().min(start + 220)]
    );
    Creep {
        like_headless: rating(text, "like headless").expect("a like-headless rating"),
        headless: rating(text, "headless").expect("a headless rating"),
        stealth: rating(text, "stealth").expect("a stealth rating"),
    }
}

#[tokio::test]
#[ignore = "launches a real Chrome and needs the network"]
async fn creepjs_finds_the_persona_no_more_headless_than_plain_chrome() {
    let plain = creep(None).await;
    let fingerprint = persona();
    let disguised = creep(Some(&fingerprint)).await;

    println!("plain Chrome: {plain:?}\npersona:      {disguised:?}");
    assert!(
        disguised.headless <= plain.headless,
        "headless: {disguised:?} against {plain:?}"
    );
    assert!(
        disguised.like_headless <= plain.like_headless,
        "like headless: {disguised:?} against {plain:?}"
    );
    assert!(
        disguised.stealth <= 100 && plain.stealth <= 100,
        "ratings are percentages"
    );
}
