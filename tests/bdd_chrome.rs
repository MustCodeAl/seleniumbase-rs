//! Gherkin scenarios on a real headless Chrome, pure CDP. `#[ignore]`d: run them
//! with `cargo test --features "testing bdd" --test bdd_chrome -- --ignored`, one
//! binary at a time. `tests/bdd_scenarios.rs` runs the same machinery on a mock.
#![cfg(feature = "bdd")]

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then};
use seleniumbase_rs::testing::bdd::SbCdpCase;
use seleniumbase_rs::Settings;

#[fixture]
async fn page() -> SbCdpCase {
    let settings = Settings {
        headless: true,
        ..Settings::default()
    };
    SbCdpCase::launch_with(settings)
        .await
        .expect("a headless Chrome starts")
}

#[given("I open a page titled {title:string}")]
async fn open_a_page(page: &SbCdpCase, title: String) -> seleniumbase_rs::Result<()> {
    page.goto(&format!(
        "data:text/html,<title>{title}</title><h1>{title}</h1>"
    ))
    .await
}

#[then("the title is {title:string}")]
async fn the_title_is(page: &SbCdpCase, title: String) -> seleniumbase_rs::Result<()> {
    assert_eq!(page.title().await?, title);
    Ok(())
}

#[scenario(
    path = "tests/features/bdd_chrome.feature",
    name = "The title of a page"
)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "starts a real Chrome"]
async fn the_title_of_a_page(#[future(awt)] page: SbCdpCase) {
    let _ = page;
}

#[scenario(path = "tests/features/bdd_chrome.feature", name = "The wrong title")]
#[tokio::test]
#[should_panic(expected = "Step failed")]
#[ignore = "starts a real Chrome"]
async fn the_wrong_title(#[future(awt)] page: SbCdpCase) {
    let _ = page;
}

#[test]
#[ignore = "starts a real Chrome"]
fn a_failed_scenario_leaves_the_three_files_of_its_page() {
    let id = format!(
        "bdd_chrome.{}",
        std::thread::current().name().unwrap_or("?")
    );
    let folder = std::path::Path::new("latest_logs").join(&id);
    drop(std::fs::remove_dir_all(&folder));

    let failed = std::panic::catch_unwind(the_wrong_title);

    assert!(failed.is_err(), "the scenario fails");
    let mut names: Vec<String> = std::fs::read_dir(&folder)
        .expect("the evidence folder")
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        ["basic_test_info.txt", "page_source.html", "screenshot.png"]
    );
    let png = std::fs::read(folder.join("screenshot.png")).unwrap();
    assert!(png.starts_with(b"\x89PNG"), "a real screenshot");
    let source = std::fs::read_to_string(folder.join("page_source.html")).unwrap();
    assert!(source.contains("<h1>Hello BDD</h1>"), "{source}");
    drop(std::fs::remove_dir_all(&folder));
}
