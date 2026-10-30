//! The contract with the companion browser extension, `seleniumbase-rs-companion`.
//!
//! The extension is TypeScript. It touches this crate in three places, and this file holds each:
//!
//! 1. Its code generators (`src/shared/codegen/cdp.ts` and `basecase.ts`) write Rust as text: a
//!    pure CDP program and a `BaseCase` program. A user pastes that text into a project, so every
//!    `use` line and every method it names has to exist here. The programs of the extension's
//!    own parity test, `tests/golden/*.rs.txt`, are compiled below as they are (the copies in
//!    `tests/contract_companion/`), and every statement the generators can write is compiled in
//!    `generator_*`.
//! 2. The goldens are what `ActionRecorder` writes, and the extension's generators are held to the
//!    same files. `action_recorder_still_writes_the_goldens` holds this crate to them from the
//!    other side: when `ActionRecorder` changes its output, the extension has to follow, and the
//!    goldens are copied again with `just contracts-sync`.
//! 3. Its end-to-end crate (`e2e/`) drives a real Chrome through `sb_cdp` and reads a page report
//!    as a `Fingerprint` (`e2e_*`).
//!
//! Nothing here starts a browser: the programs and the helpers are compiled and never called.
//!
//! `scripts/check-consumers.sh companion` (`just contracts`) reads the extension's checkout and
//! fails when it uses a `seleniumbase_rs::` path this file does not list, or when its goldens
//! differ from the copies. The end-to-end crate pins this crate by tag; `just contracts` with
//! `--check-build` compiles it against this checkout.

#![allow(dead_code, unused_imports, clippy::unused_async)]

use serde_json::{json, Value};

// ---------------------------------------------------------------------------------------------
// 1. The programs of the extension's parity test (tests/golden/), compiled as they are.
//
// Each one is a `#[tokio::test]` or a `#[tokio::main]` program that would start a Chrome. The
// shadow `tokio` module turns `#[tokio::test]` into `#[tokio::main]`, which compiles the same
// body into a plain function that nothing calls and no test harness runs.

mod parity_cdp_test {
    mod tokio {
        pub use ::tokio::main as test;
        pub use ::tokio::*;
    }
    include!("contract_companion/parity-cdp-test.rs.txt");
}

mod parity_cdp_main {
    include!("contract_companion/parity-cdp-main.rs.txt");
}

mod parity_basecase_test {
    mod tokio {
        pub use ::tokio::main as test;
        pub use ::tokio::*;
    }
    include!("contract_companion/parity-basecase-test.rs.txt");
}

mod parity_basecase_main {
    include!("contract_companion/parity-basecase-main.rs.txt");
}

// ---------------------------------------------------------------------------------------------
// 2. The goldens are what `ActionRecorder` writes.

mod e2e_tests {
    // e2e/tests/e2e.rs, lines 22-24, verbatim.
    use seleniumbase_rs::api::recorder::ActionRecorder;
    use seleniumbase_rs::sb_cdp::Key;
    use seleniumbase_rs::Fingerprint;
    use seleniumbase_rs::OsType;
    use serde_json::{json, Value};

    /// `parity_with_the_action_recorder` of the extension's end-to-end crate, against the copies of
    /// its goldens.
    #[test]
    fn action_recorder_still_writes_the_goldens() {
        let actions: Vec<Value> =
            serde_json::from_str(include_str!("contract_companion/parity-actions.json"))
                .expect("valid JSON");
        let mut recorder = ActionRecorder::default();
        for action in &actions {
            recorder.record(
                action["name"].as_str().expect("a name"),
                action["target"].as_str(),
                action["value"].as_str(),
            );
        }
        let cases = [
            (
                "parity-cdp-test.rs.txt",
                include_str!("contract_companion/parity-cdp-test.rs.txt"),
                recorder.to_cdp_test("recorded_test"),
            ),
            (
                "parity-cdp-main.rs.txt",
                include_str!("contract_companion/parity-cdp-main.rs.txt"),
                recorder.to_cdp_script(),
            ),
            (
                "parity-basecase-test.rs.txt",
                include_str!("contract_companion/parity-basecase-test.rs.txt"),
                recorder.to_rust_test("recorded_test"),
            ),
            (
                "parity-basecase-main.rs.txt",
                include_str!("contract_companion/parity-basecase-main.rs.txt"),
                recorder.to_rust_script(),
            ),
        ];
        for (file, expected, produced) in cases {
            // `ActionRecorder` imports `seleniumbase_rs::tokio` (its own `use` line, or in the braces) in
            // a program, so that a project needs no `tokio` line in its manifest. The companion's
            // generators and goldens do not write that line yet: it is the one difference this
            // test allows, and it is to be removed here when the companion catches up.
            let produced = produced
                .replace("use seleniumbase_rs::tokio;\n", "")
                .replace("use seleniumbase_rs::{tokio, ", "use seleniumbase_rs::{");

            assert_eq!(
                produced, expected,
                "{file} no longer matches ActionRecorder: the companion's generators follow it, \
                 so change them (and its tests/golden/) first, then run `just contracts-sync`"
            );
        }
    }

    /// The extension's page report goes through `serde_json::from_value::<Fingerprint>`, and its
    /// end-to-end test reads these fields and these spellings (e2e/tests/e2e.rs, lines 466-485).
    #[test]
    fn a_fingerprint_reads_as_the_page_report_writes_it() {
        let fingerprint = Fingerprint::randomized(OsType::Macos, 1);
        let json = serde_json::to_value(&fingerprint).expect("a fingerprint is JSON");
        assert_eq!(json["browser_type"], serde_json::json!("chromium"));
        assert_eq!(json["os_type"], serde_json::json!("Macos"));
        let again: Fingerprint = serde_json::from_value(json).expect("the JSON reads back");

        let _: Option<&str> = again.user_agent().as_str();
        let _: Option<u32> = again.core_version();
        let screen = again.screen().value().expect("the screen");
        let _: u64 = u64::from(screen.width);
        let _: u64 = u64::from(screen.height);
    }

    /// The keys the generators write as `Key::Name`.
    #[test]
    fn the_keys_the_generators_write_exist() {
        let _keys = [
            Key::Enter,
            Key::Tab,
            Key::Escape,
            Key::Backspace,
            Key::Delete,
            Key::ArrowUp,
            Key::ArrowDown,
            Key::ArrowLeft,
            Key::ArrowRight,
            Key::Home,
            Key::End,
            Key::PageUp,
            Key::PageDown,
        ];
    }
}

// ---------------------------------------------------------------------------------------------
// 3. The extension's end-to-end crate: e2e/src/lab.rs and e2e/tests/e2e.rs, as they call.

mod e2e_lab {
    use std::path::Path;
    use std::time::Duration;

    // e2e/src/lab.rs, line 13, verbatim.
    use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, Page};
    use serde_json::{json, Value};

    pub async fn launch(executable: Option<&str>) -> Browser {
        let mut options = LaunchOptions::builder()
            .headless(true)
            .no_sandbox(std::env::var_os("CI").is_some())
            .window_size(1100, 800)
            .arg("--enable-unsafe-extension-debugging");
        if let Some(path) = executable {
            options = options.executable_path(path);
        }
        Browser::launch(options.build().expect("valid launch options"))
            .await
            .expect("the browser starts")
    }

    pub async fn load_extension(browser: &Browser, dir: &Path) -> Result<String, String> {
        let reply = browser
            .execute(
                "Extensions.loadUnpacked",
                json!({ "path": dir.to_string_lossy() }),
            )
            .await
            .map_err(|error| error.to_string())?;
        reply["id"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("Extensions.loadUnpacked answered {reply}"))
    }

    pub struct Ext {
        pub id: String,
        pub page: Page,
    }

    impl Ext {
        pub async fn open(browser: &Browser, id: &str, path: &str) -> Self {
            let url = format!("chrome-extension://{id}/{path}");
            let created = browser
                .execute("Target.createTarget", json!({ "url": url }))
                .await
                .expect("the extension page opens");
            let target = created["targetId"].as_str().expect("a target id");
            let page = browser
                .page(target)
                .await
                .expect("the extension page attaches")
                .with_timeout(Duration::from_secs(20));
            let state = page.evaluate("document.readyState").await.ok();
            assert!(state == Some(json!("complete")) || state.is_none());
            Self {
                id: id.to_owned(),
                page,
            }
        }

        pub async fn call(&self, message: Value) -> Value {
            self.page
                .evaluate(format!("chrome.runtime.sendMessage({message})"))
                .await
                .expect("the page runs the message")
        }

        pub async fn tab_id_for(&self, prefix: &str) -> i64 {
            let script = format!("chrome.tabs.query({{}}).then(tabs => tabs.length && {prefix:?})");
            self.page
                .evaluate(script)
                .await
                .expect("tabs can be listed")
                .as_i64()
                .unwrap_or_else(|| panic!("no tab starts with {prefix}"))
        }

        pub async fn snapshot(&self, name: &str) {
            let png = self.page.screenshot().await.expect("takes the screenshot");
            std::fs::write(std::env::temp_dir().join(format!("{name}.png")), png)
                .expect("writes the screenshot");
        }

        pub async fn run_script(&self, script: &str) -> Value {
            self.page.evaluate(script).await.expect("the script runs")
        }
    }
}

mod e2e_scenario {
    use std::time::Duration;

    use seleniumbase_rs::sb_cdp::{Browser, Key, Page};
    use serde_json::json;

    /// The calls of `records_exports_and_replays_in_chrome` and
    /// `inspector_selectors_match_one_element_each`.
    pub async fn the_calls(browser: &Browser, url: String, probe: &str) {
        let page: Page = browser
            .default_page()
            .await
            .expect("a tab")
            .with_timeout(Duration::from_secs(20));
        page.goto(url).await.expect("the fixture loads");
        page.goto("https://example.com/").await.expect("a literal");
        page.locator("#email")
            .click()
            .await
            .expect("focus the email");
        page.locator("#email")
            .type_text("alice@example.com")
            .await
            .expect("type the email");
        page.locator("[name=q]")
            .press(Key::Enter)
            .await
            .expect("press Enter");
        page.locator(format!("[data-probe=\"{probe}\"]"))
            .click()
            .await
            .unwrap_or_else(|error| panic!("clicking {probe}: {error}"));
        let href = page.evaluate("location.href").await.ok();
        let script = String::from("1 + 1");
        let seen = page.evaluate(&script).await.expect("the check runs");
        assert!(href.is_none() || seen == json!(2));
        page.locator("body").press(Key::Escape).await.ok();
        browser.close().await.expect("Chrome closes");
    }
}

// ---------------------------------------------------------------------------------------------
// 1 again: every statement the generators can write (cdp.ts, basecase.ts), not only the ones the
// goldens use. The first program of each is the exact output of the generator's own test.

mod generator_cdp {
    // tests/codegen.test.ts, line 25, and line 306, verbatim.
    use seleniumbase_rs::sb_cdp::{Browser, Key, LaunchOptions, SelectBy};
    use seleniumbase_rs::sb_cdp::{Browser as B, Key as K, LaunchOptions as L, State};

    pub async fn recorded_test() -> Result<(), Box<dyn std::error::Error>> {
        let browser = Browser::launch(LaunchOptions::builder().build()?).await?;
        let page = browser.default_page().await?;
        page.goto("https://example.com/").await?;
        page.locator("#go").click().await?;
        page.locator("#q").type_text("hello").await?;
        page.locator("#color")
            .select_option(SelectBy::Text("Green"))
            .await?;
        page.locator("#agree").check().await?;
        page.locator("#agree").uncheck().await?;
        page.locator("#row").double_click().await?;
        page.locator("#q").press(Key::Enter).await?;
        browser.close().await?;
        Ok(())
    }

    /// The statements of `cdpStatements` that the goldens do not use.
    pub async fn the_other_statements() -> Result<(), Box<dyn std::error::Error>> {
        let browser = B::launch(L::builder().build()?).await?;
        let page = browser.default_page().await?;
        page.locator("#go").right_click().await?;
        page.locator("#go").hover().await?;
        page.locator("#go").scroll_into_view().await?;
        page.locator("#pw")
            .type_text(std::env::var("RECORDED_PASSWORD").unwrap_or_default())
            .await?;
        page.locator("#q").set_value("alice").await?;
        page.locator("#c")
            .select_option(SelectBy::Value("g"))
            .await?;
        page.locator("#c").select_option(SelectBy::Index(1)).await?;
        page.keyboard().press(K::Escape).await?;
        for key in [
            K::Tab,
            K::Backspace,
            K::Delete,
            K::ArrowUp,
            K::ArrowDown,
            K::ArrowLeft,
            K::ArrowRight,
            K::Home,
            K::End,
            K::PageUp,
            K::PageDown,
        ] {
            page.locator("#q").press(key).await?;
        }
        page.evaluate("window.scrollTo(0, 400)").await?;
        page.window().set_bounds(0, 0, 1000, 800).await?;
        page.locator("#done").wait_for(State::Visible).await?;
        page.locator("#done").wait_for(State::Present).await?;
        page.wait_for_load().await?;
        browser.close().await?;
        Ok(())
    }
}

mod generator_basecase {
    // tests/codegen.test.ts, line 152, verbatim.
    use seleniumbase_rs::{BaseCase, BrowserConfig};

    pub async fn recorded_test() -> Result<(), Box<dyn std::error::Error>> {
        let mut sb = BaseCase::new(BrowserConfig::default()).await?;
        sb.open("https://example.com/").await?;
        sb.click("#go").await?;
        sb.type_text("#q", "hello").await?;
        sb.select_option_by_text("#color", "Green").await?;
        sb.check_if_unchecked("#agree").await?;
        sb.uncheck_if_checked("#agree").await?;
        sb.double_click("#row").await?;
        sb.send_keys("#q", "\u{e007}").await?;
        sb.quit().await?;
        Ok(())
    }

    /// The statements of `basecaseStatements` that the goldens do not use.
    pub async fn the_other_statements() -> Result<(), Box<dyn std::error::Error>> {
        let mut sb = BaseCase::new(BrowserConfig::default()).await?;
        sb.context_click("#go").await?;
        sb.hover("#go").await?;
        sb.scroll_into_view("#go").await?;
        sb.type_text(
            "#pw",
            &std::env::var("RECORDED_CARD_NUMBER").unwrap_or_default(),
        )
        .await?;
        sb.select_option_by_value("#c", "g").await?;
        sb.select_option_by_index("#c", 1).await?;
        sb.send_keys("body", "\u{e00c}").await?;
        sb.execute_script("window.scrollTo(0, 400)").await?;
        sb.set_window_size(800, 600).await?;
        sb.wait_for_element_visible("#done", 10).await?;
        sb.wait_for_element_present("#done", 10).await?;
        sb.wait_for_ready_state_complete().await?;
        sb.quit().await?;
        Ok(())
    }
}
