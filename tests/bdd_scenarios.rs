//! Gherkin scenarios through rstest-bdd on a mocked pure CDP browser: the
//! Background, a data table, an outline, and what the `sb_cdp` case does when a
//! step fails (the evidence, the closed browser). No Chrome starts;
//! `tests/bdd_chrome.rs` runs a scenario on a real one.
#![cfg(all(feature = "bdd", feature = "test-util"))]

use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

use base64::prelude::*;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use seleniumbase_rs::sb_cdp::Browser;
use seleniumbase_rs::testing::bdd::SbCdpCase;
use seleniumbase_rs::Settings;
use serde_json::json;

const PNG: &[u8] = b"\x89PNG-from-the-mock";

/// Where the evidence of every scenario goes.
static LOGS: LazyLock<tempfile::TempDir> =
    LazyLock::new(|| tempfile::tempdir().expect("a temporary folder"));

/// The mocked browsers, by the name of the thread that asked for them.
static MADE: Mutex<Vec<(String, Browser)>> = Mutex::new(Vec::new());

fn thread_name() -> String {
    std::thread::current().name().unwrap_or("?").to_owned()
}

#[fixture]
async fn page() -> SbCdpCase {
    let owner = thread_name();
    SbCdpCase::launch_with_browser(Settings::default(), LOGS.path(), move || {
        let owner = owner.clone();
        async move {
            let (browser, mock) = Browser::new_mocked();
            mock.reply(
                "Runtime.evaluate",
                json!({ "result": { "value": "https://shop.test/cart" } }),
            );
            mock.reply(
                "Page.captureScreenshot",
                json!({ "data": BASE64_STANDARD.encode(PNG) }),
            );
            MADE.lock().unwrap().push((owner, browser.clone()));
            Ok(browser)
        }
    })
    .await
    .expect("the mocked browser starts")
}

/// What the steps of a scenario keep between them.
#[derive(Default)]
struct World {
    count: u32,
    address: String,
}

#[fixture]
fn world() -> World {
    World::default()
}

#[given("the shop page is open")]
async fn shop_page_is_open(page: &SbCdpCase) {
    assert_eq!(page.url().await.unwrap(), "https://shop.test/cart");
}

#[when("I read the address of the page")]
async fn read_the_address(page: &SbCdpCase, world: &mut World) {
    world.address = page.url().await.unwrap();
}

#[then("the address is {address:string}")]
async fn the_address_is(world: &World, address: String) {
    assert_eq!(world.address, address, "the address of the page");
}

#[given("these items are in the cart:")]
fn items_in_the_cart(#[datatable] table: Vec<Vec<String>>, world: &mut World) {
    assert_eq!(table[0], ["name", "price"]);
    world.count = u32::try_from(table.len() - 1).unwrap();
}

#[then("the cart holds {n:u32} items")]
fn the_cart_holds(world: &World, n: u32) {
    assert_eq!(world.count, n);
}

#[when("I count {n:u32} items")]
async fn count_items(world: &mut World, n: u32) {
    world.count = n;
}

#[then("the count is {n:u32}")]
async fn the_count_is(world: &World, n: u32) {
    assert_eq!(world.count, n);
}

#[scenario(path = "tests/features/bdd_shop.feature", name = "The page answers")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_page_answers(#[future(awt)] page: SbCdpCase, world: World) {
    assert!(page.test_id().starts_with("bdd_scenarios."), "{page:?}");
    let _ = world;
}

#[scenario(path = "tests/features/bdd_shop.feature", name = "Items in a table")]
#[tokio::test]
async fn items_in_a_table(#[future(awt)] page: SbCdpCase, world: World) {
    let _ = (page, world);
}

#[scenario(path = "tests/features/bdd_shop.feature", name = "Counting")]
#[tokio::test]

async fn counting(#[future(awt)] page: SbCdpCase, world: World, n: u32) {
    let _ = (page, world);
}

/// Run by `a_failing_step_...` below, which expects it to fail.
#[scenario(path = "tests/features/bdd_shop.feature", name = "A step fails")]
#[tokio::test]
#[should_panic(expected = "Step failed")]
async fn a_step_fails(#[future(awt)] page: SbCdpCase, world: World) {
    let _ = (page, world);
}

fn names(folder: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .map(|entries| {
            entries
                .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// Whether every browser the current thread asked for was told to close.
fn closed_browsers(owner: &str) -> (usize, usize) {
    let browsers: Vec<Browser> = MADE
        .lock()
        .unwrap()
        .iter()
        .filter(|(who, _)| who == owner)
        .map(|(_, browser)| browser.clone())
        .collect();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let closed = runtime.block_on(async {
        let mut closed = 0;
        for browser in &browsers {
            // A closed mock answers nothing.
            let gone = match browser.default_page().await {
                Ok(page) => page.url().await.is_err(),
                Err(_) => true,
            };
            closed += usize::from(gone);
        }
        closed
    });
    (closed, browsers.len())
}

fn evidence_folder() -> PathBuf {
    LOGS.path().join(format!("bdd_scenarios.{}", thread_name()))
}

#[test]
fn a_failing_step_leaves_the_evidence_and_closes_the_browser() {
    let failed = std::panic::catch_unwind(a_step_fails);
    assert!(failed.is_err(), "the scenario fails");

    let folder = evidence_folder();
    assert_eq!(
        names(&folder),
        ["basic_test_info.txt", "page_source.html", "screenshot.png"]
    );
    assert_eq!(std::fs::read(folder.join("screenshot.png")).unwrap(), PNG);
    let info = std::fs::read_to_string(folder.join("basic_test_info.txt")).unwrap();
    assert!(info.contains("Last Page: https://shop.test/cart"), "{info}");
    assert!(
        info.contains("https://nowhere.test"),
        "the reason of the failure is in the info: {info}"
    );
    assert_eq!(closed_browsers(&thread_name()), (1, 1), "closed afterwards");
}

#[test]
fn a_passing_scenario_closes_its_browser_and_leaves_no_evidence() {
    the_page_answers();
    assert_eq!(closed_browsers(&thread_name()), (1, 1), "closed afterwards");
    assert!(
        names(&evidence_folder()).is_empty(),
        "a pass leaves nothing"
    );
}
