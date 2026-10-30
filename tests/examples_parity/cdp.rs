//! The Python `cdp_mode/` examples, on the pure CDP engine.
//!
//! Python's `sb_cdp.Chrome()` and `sb.activate_cdp_mode()` map to
//! [`Browser`](seleniumbase_rs::sb_cdp::Browser) and its
//! [`Page`](seleniumbase_rs::sb_cdp::Page); the `sb.assert_*` methods are
//! `expect()` assertions of a locator or the page, and a click, a type or a
//! hover is a method of the [`Locator`](seleniumbase_rs::sb_cdp::Locator) that
//! `page.locator(selector)` returns.

use seleniumbase_rs::api::runner::run_cdp_test;
use seleniumbase_rs::sb_cdp::{Page, SelectBy};
use seleniumbase_rs::Result;

use crate::support::{cdp_options, run_cdp_port, Mismatches};

// ---------------------------------------------------------------------------
// Steps shared by the ports
// ---------------------------------------------------------------------------

/// Python's `assert_text(text, selector)` on an `<input>` or `<textarea>`,
/// whose text is its value. Tries `expect().to_contain_text`, the equal this
/// crate lists for it, and when that fails reads the value with a script.
async fn field_text(
    page: &Page,
    mismatches: &mut Mismatches,
    selector: &str,
    expected: &str,
) -> Result<()> {
    const STEP: &str = "assert_text on an input or a textarea (Python reads its value)";
    if !mismatches.knows(STEP) {
        match page
            .locator(selector)
            .expect()
            .to_contain_text(expected)
            .await
        {
            Ok(()) => return Ok(()),
            Err(error) => mismatches.note(STEP, error),
        }
    }
    let script = format!(
        "return document.querySelector({}).value;",
        serde_json::to_string(selector)?
    );
    let value = page.evaluate(&script).await?;
    assert!(
        value.as_str().is_some_and(|value| value.contains(expected)),
        "{selector} holds {value}, not {expected:?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------

// cdp_mode/raw_demo_site.py
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "real headless Chrome; opens seleniumbase.io/demo_page"]
async fn raw_demo_site() -> Result<()> {
    run_cdp_port(async |page, mismatches| {
        page.goto("https://seleniumbase.io/demo_page").await?;

        // Assert various things
        page.expect().to_have_title("Web Testing Page").await?;
        page.locator("tbody#tbodyId")
            .expect()
            .to_be_visible()
            .await?;
        page.locator("h1")
            .expect()
            .to_contain_text("Demo Page")
            .await?;

        // Type text into various text fields and then assert
        page.locator("#myTextInput")
            .type_text("This is Automated")
            .await?;
        page.locator("textarea.area1")
            .type_text("Testing Time!\n")
            .await?;
        page.locator("[name=\"preText2\"]")
            .type_text("Typing Text!")
            .await?;
        field_text(page, mismatches, "#myTextInput", "This is Automated").await?;
        field_text(page, mismatches, "textarea.area1", "Testing Time!\n").await?;
        field_text(page, mismatches, "[name=\"preText2\"]", "Typing Text!").await?;

        // Hover & click a dropdown element and assert results
        page.locator("h3")
            .expect()
            .to_contain_text("Automation Practice")
            .await?;
        page.locator("#myDropdown").hover().await?;
        page.locator("#dropOption2").click().await?;
        page.locator("h3")
            .expect()
            .to_contain_text("Link Two Selected")
            .await?;

        // Click a button and then verify the expected results
        page.locator("#pText")
            .expect()
            .to_contain_text("This Text is Green")
            .await?;
        page.locator("button:contains(\"Click Me\")")
            .click()
            .await?;
        page.locator("#pText")
            .expect()
            .to_contain_text("This Text is Purple")
            .await?;

        // Verify that a slider control updates a progress bar
        page.locator("progress[value=\"50\"]")
            .expect()
            .to_be_visible()
            .await?;
        page.locator("input#mySlider").set_value("100").await?;
        page.locator("progress[value=\"100\"]")
            .expect()
            .to_be_visible()
            .await?;

        // Verify that a "select" option updates a meter bar
        page.locator("meter[value=\"0.25\"]")
            .expect()
            .to_be_visible()
            .await?;
        page.locator("#mySelect")
            .select_option(SelectBy::Text("Set to 75%"))
            .await?;
        page.locator("meter[value=\"0.75\"]")
            .expect()
            .to_be_visible()
            .await?;

        // Verify that clicking a radio button selects it
        assert!(!page.locator("#radioButton2").is_selected().await?);
        page.locator("#radioButton2").click().await?;
        assert!(page.locator("#radioButton2").is_selected().await?);

        // Verify that clicking a checkbox makes it selected
        page.locator("img#logo").expect().to_be_hidden().await?;
        assert!(!page.locator("#checkBox1").is_selected().await?);
        page.locator("#checkBox1").click().await?;
        assert!(page.locator("#checkBox1").is_selected().await?);
        page.locator("img#logo").expect().to_be_visible().await?;

        // Verify clicking on multiple elements with one call
        for id in ["#checkBox2", "#checkBox3", "#checkBox4"] {
            assert!(!page.locator(id).is_selected().await?);
        }
        for checkbox in page
            .locator("input.checkBoxClassB")
            .visible()
            .all_waiting()
            .await?
        {
            checkbox.click().await?;
        }
        for id in ["#checkBox2", "#checkBox3", "#checkBox4"] {
            assert!(page.locator(id).is_selected().await?);
        }

        // Verify Drag and Drop
        page.locator("div#drop2 img#logo")
            .expect()
            .to_be_hidden()
            .await?;
        page.locator("img#logo")
            .drag_to(&page.locator("div#drop2"))
            .await?;
        page.locator("div#drop2 img#logo")
            .expect()
            .to_be_visible()
            .await?;

        // Click inside an iframe and test highlighting
        page.locator("iframe#myFrame3").flash().await?;
        if let Err(error) = page
            .locator("iframe#myFrame3")
            .locator(".fBox")
            .click()
            .await
        {
            // The frame's src is a data: URL, so it is another origin than the
            // page, and the pure CDP engine reaches same-origin frames only.
            mismatches.note(
                "nested_click into an iframe whose src is a data: URL",
                error,
            );
        }
        page.locator("iframe#myFrame3").flash().await
    })
    .await
}

// ---------------------------------------------------------------------------
// cdp_mode/raw_cdp_tabs.py (and raw_tab_switching.py, the same steps on `sb.cdp`)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "real headless Chrome; opens data: pages in two tabs"]
async fn raw_cdp_tabs() -> Result<()> {
    run_cdp_test(cdp_options(), async |first| {
        first.goto("data:text/html,<h1>Page A</h1>").await?;
        first
            .locator("html")
            .expect()
            .to_contain_text("Page A")
            .await?;
        // open_new_tab() makes the new tab the active one in Python; here the
        // new tab is a Page of its own.
        let second = first.browser().new_page().await?;
        second.goto("data:text/html,<h1>Page B</h1>").await?;
        second
            .locator("html")
            .expect()
            .to_contain_text("Page B")
            .await?;
        // switch_to_tab(0) is the first of the browser's pages.
        let tabs = first.browser().pages().await?;
        assert_eq!(tabs.len(), 2);
        tabs[0]
            .locator("html")
            .expect()
            .to_contain_text("Page A")
            .await?;
        tabs[0]
            .locator("html")
            .expect()
            .not()
            .to_contain_text("Page B")
            .await?;
        tabs[1]
            .locator("html")
            .expect()
            .to_contain_text("Page B")
            .await?;
        tabs[1]
            .locator("html")
            .expect()
            .not()
            .to_contain_text("Page A")
            .await
    })
    .await
}

// ---------------------------------------------------------------------------
// cdp_mode/raw_cdp_xpath.py (and raw_xpath.py, the same steps on `sb.cdp`)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "real headless Chrome; opens seleniumbase.io/demo_page"]
async fn raw_cdp_xpath() -> Result<()> {
    run_cdp_test(cdp_options(), async |page| {
        page.goto("https://seleniumbase.io/demo_page").await?;
        page.locator("//input[@id=\"myTextInput\"]").flash().await?;
        page.locator("//*[@id=\"myTextInput\"]")
            .type_text("XPath Test!")
            .await?;
        page.locator("//input[@name=\"text\" or @name=\"preText2\"]")
            .type_text("Xpath Here!")
            .await?;
        page.locator("//button[contains(text(),\"(Green)\")]")
            .flash()
            .await?;
        page.locator("//button[starts-with(text(),\"Click Me\")]")
            .click()
            .await?;
        page.locator("//button[contains(., \"Purple\")]")
            .expect()
            .to_be_visible()
            .await?;
        page.locator("//table/tbody/tr/td/h3").flash().await?;
        page.locator("//table/tbody/tr[1]/td[2]/h2").flash().await?;
        page.locator("//table//h2")
            .expect()
            .to_contain_text("SeleniumBase")
            .await
    })
    .await
}

// ---------------------------------------------------------------------------
// cdp_mode/raw_basic_cdp.py
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "real headless Chrome; opens seleniumbase.io/simple/login"]
async fn raw_basic_cdp() -> Result<()> {
    run_cdp_test(cdp_options(), async |page| {
        page.goto("https://seleniumbase.io/simple/login").await?;
        page.locator("#username").type_text("demo_user").await?;
        page.locator("#password").type_text("secret_pass").await?;
        page.locator("a:contains(\"Sign in\")").click().await?;
        page.locator("h1").expect().to_have_text("Welcome!").await?;
        page.locator("img#image1").expect().to_be_visible().await?;
        page.locator("#image1").flash().await?;
        let top_nav = page.locator("div.topnav");
        let links = top_nav.locator("a").all_waiting().await?;
        assert!(!links.is_empty(), "the top navigation has links");
        for nav_item in links {
            println!("{}", nav_item.text().await?);
        }
        // save_as_pdf_to_logs, save_screenshot_to_logs and
        // save_page_source_to_logs write these three to the logs folder.
        assert!(page.pdf().await?.starts_with(b"%PDF"));
        assert!(page.screenshot().await?.starts_with(b"\x89PNG"));
        assert!(page.content().await?.contains("<html"));
        // MISMATCH: save_data_to_logs and append_data_to_logs have no equal on
        // the page (parity/api.toml: not-applicable).
        page.locator("a:contains(\"Sign out\")").click().await?;
        page.locator("#top_message")
            .expect()
            .to_contain_text("signed out")
            .await
    })
    .await
}

// ---------------------------------------------------------------------------
// cdp_mode/raw_drag_and_drop.py (the first block; the second drags on
// jqueryui.com, which is not a site for automation tests)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "real headless Chrome; opens seleniumbase.io/other/drag_and_drop"]
async fn raw_drag_and_drop() -> Result<()> {
    run_cdp_test(cdp_options(), async |page| {
        page.goto("https://seleniumbase.io/other/drag_and_drop")
            .await?;
        page.locator("#div1 img#drag1")
            .expect()
            .to_be_hidden()
            .await?;
        page.locator("#drag1")
            .drag_to(&page.locator("#div1"))
            .await?;
        page.locator("#div1 img#drag1")
            .expect()
            .to_be_visible()
            .await
    })
    .await
}

// ---------------------------------------------------------------------------
// cdp_mode/raw_cdp_swag.py
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "real headless Chrome; opens saucedemo.com"]
async fn raw_cdp_swag() -> Result<()> {
    run_cdp_test(cdp_options(), async |page| {
        page.goto("https://www.saucedemo.com").await?;
        page.locator("#user-name")
            .type_text("standard_user")
            .await?;
        page.locator("#password")
            .type_text("secret_sauce\n")
            .await?;
        page.locator("div.inventory_list")
            .expect()
            .to_be_visible()
            .await?;
        page.locator("button[name*=\"backpack\"]").click().await?;
        page.locator("#shopping_cart_container a").click().await?;
        page.locator("div.cart_item")
            .expect()
            .to_contain_text("Backpack")
            .await?;
        page.locator("button#checkout").click().await?;
        page.locator("input#first-name")
            .type_text("SeleniumBase")
            .await?;
        page.locator("input#last-name")
            .type_text("Automation")
            .await?;
        page.locator("input#postal-code").type_text("77123").await?;
        page.locator("input#continue").click().await?;
        page.locator("button#finish").click().await?;
        page.locator("html")
            .expect()
            .to_contain_text("Thank you for your order!")
            .await
    })
    .await
}
