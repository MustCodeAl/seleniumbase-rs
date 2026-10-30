//! The Python examples built on `BaseCase`, on the WebDriver engine.
//!
//! Each test is named after its Python file (a Python test method of a file
//! with several adds its own name). A `MISMATCH` comment marks a step where
//! this crate does not do what the Python call does; `parity/examples.toml`
//! lists them all.

use seleniumbase_rs::{run_browser_test, BaseCase, Result};

use crate::support::{headless, run_port, Mismatches};

// The helpers below try the Python call as this crate spells it. When it
// fails they note the step and carry on with the nearest composition, so one
// run lists every difference of an example (see `Mismatches`). Once a step is
// noted the Python call is not tried again: it would only wait for its
// timeout once more.

/// Python's `switch_to_frame(frame)`: `frame` is a selector, or the id or the
/// name of a frame, as in `switch_to_frame("iframeResult")`.
async fn enter_frame(sb: &mut BaseCase, mismatches: &mut Mismatches, frame: &str) -> Result<()> {
    const STEP: &str = "switch_to_frame with the id or the name of a frame";
    if !mismatches.knows(STEP) {
        match sb.switch_to_frame(frame).await {
            Ok(()) => return Ok(()),
            Err(error) => mismatches.note(STEP, error),
        }
    }
    sb.switch_to_frame(&format!("#{frame}")).await
}

/// Python's `frame_switch(frame)`, which takes a frame id or name as well.
async fn enter_frame_switch(
    sb: &mut BaseCase,
    mismatches: &mut Mismatches,
    frame: &str,
) -> Result<()> {
    const STEP: &str = "frame_switch with the id or the name of a frame";
    if !mismatches.knows(STEP) {
        match sb.frame_switch(frame).await {
            Ok(()) => return Ok(()),
            Err(error) => mismatches.note(STEP, error),
        }
    }
    sb.frame_switch(&format!("#{frame}")).await
}

/// The note for `::shadow` in the regular methods. The helpers below share it.
const SHADOW: &str = "::shadow selectors in the regular methods (get_text, assert_exact_text, \
                      assert_element, type_text, assert_text, click)";

/// Python's `get_text("host::shadow inner")`.
async fn shadow_text(
    sb: &mut BaseCase,
    mismatches: &mut Mismatches,
    selector: &str,
) -> Result<String> {
    if !mismatches.knows(SHADOW) {
        match sb.get_text(selector).await {
            Ok(text) => return Ok(text),
            Err(error) => mismatches.note(SHADOW, error),
        }
    }
    let text = sb.shadow_get_text(selector).await?;
    if !text.trim().is_empty() {
        return Ok(text);
    }
    // A container that only slots its content has no text of its own.
    mismatches.note(
        "shadow_get_text on a container of slotted content (it reads textContent, Python the rendered text)",
        "it returned an empty string",
    );
    rendered_shadow_text(sb, selector).await
}

/// Python's `assert_exact_text(text, "host::shadow inner")`.
async fn assert_shadow_exact(
    sb: &mut BaseCase,
    mismatches: &mut Mismatches,
    expected: &str,
    selector: &str,
) -> Result<()> {
    if !mismatches.knows(SHADOW) {
        match sb.assert_exact_text(expected, selector).await {
            Ok(()) => return Ok(()),
            Err(error) => mismatches.note(SHADOW, error),
        }
    }
    let text = shadow_text(sb, mismatches, selector).await?;
    sb.assert_equal(text.trim(), expected.trim())
}

/// Python's `assert_element("host::shadow inner")`.
async fn shadow_assert_element(
    sb: &mut BaseCase,
    mismatches: &mut Mismatches,
    selector: &str,
) -> Result<()> {
    if !mismatches.knows(SHADOW) {
        match sb.assert_element(selector).await {
            Ok(()) => return Ok(()),
            Err(error) => mismatches.note(SHADOW, error),
        }
    }
    sb.assert_shadow_element(selector).await
}

/// Python's `type("host::shadow inner", text)`.
async fn shadow_type(
    sb: &mut BaseCase,
    mismatches: &mut Mismatches,
    selector: &str,
    text: &str,
) -> Result<()> {
    if !mismatches.knows(SHADOW) {
        match sb.type_text(selector, text).await {
            Ok(()) => return Ok(()),
            Err(error) => mismatches.note(SHADOW, error),
        }
    }
    sb.shadow_type(selector, text).await
}

/// Python's `assert_text(text, "host::shadow inner")`.
async fn shadow_assert_text(
    sb: &mut BaseCase,
    mismatches: &mut Mismatches,
    text: &str,
    selector: &str,
) -> Result<()> {
    if !mismatches.knows(SHADOW) {
        match sb.assert_text(text, selector).await {
            Ok(()) => return Ok(()),
            Err(error) => mismatches.note(SHADOW, error),
        }
    }
    if sb.assert_shadow_text(text, selector).await.is_ok() {
        return Ok(());
    }
    // Python's assert_text waits for the text to appear; assert_shadow_text
    // looks once.
    for _ in 0..40 {
        sb.sleep(0.25).await;
        if sb.assert_shadow_text(text, selector).await.is_ok() {
            mismatches.note(
                "assert_shadow_text looks once (Python's assert_text waits for the text)",
                "the text was not there at the first look",
            );
            return Ok(());
        }
    }
    sb.assert_shadow_text(text, selector).await
}

/// Python's `click("host::shadow inner")`.
async fn shadow_click(
    sb: &mut BaseCase,
    mismatches: &mut Mismatches,
    selector: &str,
) -> Result<()> {
    if !mismatches.knows(SHADOW) {
        match sb.click(selector).await {
            Ok(()) => return Ok(()),
            Err(error) => mismatches.note(SHADOW, error),
        }
    }
    sb.shadow_click(selector).await
}

/// The rendered text (`innerText`) of the element that a `host::shadow inner`
/// selector names, found by a script that enters each open shadow root.
async fn rendered_shadow_text(sb: &mut BaseCase, selector: &str) -> Result<String> {
    // The text of the element, or else of the visible nodes that its slots
    // show: the browser's innerText stops at a slot.
    let script = format!(
        "{} if (!el) return '';\
         if (el.innerText) return el.innerText;\
         var parts = [];\
         for (const slot of el.querySelectorAll('slot')) {{\
             for (const node of slot.assignedNodes({{flatten: true}})) {{\
                 if (node.nodeType === 1 && node.getClientRects().length) parts.push(node.innerText);\
             }}\
         }}\
         return parts.join('');",
        shadow_find_script(selector)?
    );
    let text = sb.execute_script(&script).await?;
    Ok(text.as_str().unwrap_or_default().to_owned())
}

/// A script that finds the element a `host::shadow inner` selector names, in
/// the variable `el` (`null` when there is none), entering each open shadow
/// root.
fn shadow_find_script(selector: &str) -> Result<String> {
    Ok(format!(
        "var root = document, el = null;\
         for (const part of {}.split('::shadow')) {{\
             el = root.querySelector(part.trim());\
             if (!el) break;\
             root = el.shadowRoot || el;\
         }}",
        serde_json::to_string(selector)?
    ))
}

// ---------------------------------------------------------------------------
// my_first_test.py
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "real headless Chrome; opens saucedemo.com"]
async fn my_first_test() -> Result<()> {
    run_browser_test(headless(), async |sb| {
        sb.goto("https://www.saucedemo.com").await?;
        sb.type_text("#user-name", "standard_user").await?;
        sb.type_text("#password", "secret_sauce\n").await?;
        sb.assert_element("div.inventory_list").await?;
        sb.assert_exact_text("Products", "span.title").await?;
        sb.click("button[name*=\"backpack\"]").await?;
        sb.click("#shopping_cart_container a").await?;
        sb.assert_exact_text("Your Cart", "span.title").await?;
        sb.assert_text("Backpack", "div.cart_item").await?;
        sb.click("button#checkout").await?;
        sb.type_text("#first-name", "SeleniumBase").await?;
        sb.type_text("#last-name", "Automation").await?;
        sb.type_text("#postal-code", "77123").await?;
        sb.click("input#continue").await?;
        // Python: assert_text("Checkout: Overview"), whose selector is "html".
        sb.assert_text("Checkout: Overview", "html").await?;
        sb.assert_text("Backpack", "div.cart_item").await?;
        sb.assert_text("29.99", "div.inventory_item_price").await?;
        sb.click("button#finish").await?;
        sb.assert_exact_text("Thank you for your order!", "h2")
            .await?;
        sb.assert_element("img[alt=\"Pony Express\"]").await?;
        sb.js_click("a#logout_sidebar_link").await?;
        sb.assert_element("div#login_button_container").await
    })
    .await
}

// ---------------------------------------------------------------------------
// test_demo_site.py
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "real headless Chrome; opens seleniumbase.io/demo_page"]
async fn test_demo_site() -> Result<()> {
    run_port(async |sb, _| {
        // Open a web page in the active browser window
        sb.goto("https://seleniumbase.io/demo_page").await?;

        // Assert the title of the current web page
        sb.assert_title("Web Testing Page").await?;

        // Assert that an element is visible on the page
        sb.assert_element("tbody#tbodyId").await?;

        // Assert that a text substring appears in an element
        sb.assert_text("Demo Page", "h1").await?;

        // Type text into various text fields and then assert
        sb.type_text("#myTextInput", "This is Automated").await?;
        sb.type_text("textarea.area1", "Testing Time!\n").await?;
        sb.type_text("[name=\"preText2\"]", "Typing Text!").await?;
        sb.assert_text("This is Automated", "#myTextInput").await?;
        sb.assert_text("Testing Time!\n", "textarea.area1").await?;
        sb.assert_text("Typing Text!", "[name=\"preText2\"]")
            .await?;

        // Hover & click a dropdown element and assert results
        sb.assert_text("Automation Practice", "h3").await?;
        if sb
            .hover_and_click("#myDropdown", "#dropOption2")
            .await
            .is_err()
        {
            // Someone probably moved the mouse while the test ran
            sb.hover_and_js_click("#myDropdown", "#dropOption2").await?;
        }
        sb.assert_text("Link Two Selected", "h3").await?;

        // Click a button and then verify the expected results
        sb.assert_text("This Text is Green", "#pText").await?;
        sb.click("button:contains(\"Click Me\")").await?;
        sb.assert_text("This Text is Purple", "#pText").await?;

        // Assert that the given SVG is visible on the page
        sb.assert_element("svg[name=\"svgName\"]").await?;

        // Verify that a slider control updates a progress bar
        sb.assert_element("progress[value=\"50\"]").await?;
        sb.set_value("input#mySlider", "100").await?;
        sb.assert_element("progress[value=\"100\"]").await?;

        // Verify that a "select" option updates a meter bar
        sb.assert_element("meter[value=\"0.25\"]").await?;
        sb.select_option_by_text("#mySelect", "Set to 75%").await?;
        sb.assert_element("meter[value=\"0.75\"]").await?;

        // Assert an element located inside an iframe
        sb.assert_false(sb.is_element_visible("img").await?)?;
        sb.switch_to_frame("#myFrame1").await?;
        sb.assert_true(sb.is_element_visible("img").await?)?;
        sb.switch_to_default_content().await?;

        // Assert text located inside an iframe
        sb.assert_false(sb.is_text_visible("iFrame Text", "html").await?)?;
        sb.switch_to_frame("#myFrame2").await?;
        sb.assert_true(sb.is_text_visible("iFrame Text", "html").await?)?;
        sb.switch_to_default_content().await?;

        // Verify that clicking a radio button selects it
        sb.assert_false(sb.is_selected("#radioButton2").await?)?;
        sb.click("#radioButton2").await?;
        sb.assert_true(sb.is_selected("#radioButton2").await?)?;

        // Verify that clicking a checkbox makes it selected
        sb.assert_element_not_visible("img#logo").await?;
        sb.assert_false(sb.is_selected("#checkBox1").await?)?;
        sb.click("#checkBox1").await?;
        sb.assert_true(sb.is_selected("#checkBox1").await?)?;
        sb.assert_element("img#logo").await?;

        // Verify clicking on multiple elements with one call
        sb.assert_false(sb.is_selected("#checkBox2").await?)?;
        sb.assert_false(sb.is_selected("#checkBox3").await?)?;
        sb.assert_false(sb.is_selected("#checkBox4").await?)?;
        sb.click_visible_elements("input.checkBoxClassB").await?;
        sb.assert_true(sb.is_selected("#checkBox2").await?)?;
        sb.assert_true(sb.is_selected("#checkBox3").await?)?;
        sb.assert_true(sb.is_selected("#checkBox4").await?)?;

        // Verify that clicking an iframe checkbox selects it
        sb.assert_false(sb.is_element_visible(".fBox").await?)?;
        sb.switch_to_frame("#myFrame3").await?;
        sb.assert_true(sb.is_element_visible(".fBox").await?)?;
        sb.assert_false(sb.is_selected(".fBox").await?)?;
        sb.click(".fBox").await?;
        sb.assert_true(sb.is_selected(".fBox").await?)?;
        sb.switch_to_default_content().await?;

        // Verify Drag and Drop
        sb.assert_element_not_visible("div#drop2 img#logo").await?;
        sb.drag_and_drop("img#logo", "div#drop2").await?;
        sb.assert_element("div#drop2 img#logo").await?;

        // Assert link text
        sb.assert_link_text("seleniumbase.com").await?;
        sb.assert_link_text("SeleniumBase on GitHub").await?;
        sb.assert_link_text("seleniumbase.io").await?;

        // Click link text
        sb.click_link("SeleniumBase Demo Page").await?;

        // Assert exact text
        sb.assert_exact_text("Demo Page", "h1").await?;

        // Highlight a page element (Also asserts visibility)
        sb.highlight("h2").await?;

        // (Python: activate_demo_mode() only when headed.)
        sb.type_text("input", "Have a Nice Day!").await?;
        sb.assert_text("SeleniumBase", "h2").await
    })
    .await
}

// ---------------------------------------------------------------------------
// iframe_tests.py
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "real headless Chrome; opens seleniumbase.io/w3schools/iframes.html"]
async fn iframe_tests_iframe_basics() -> Result<()> {
    run_port(async |sb, mismatches| {
        sb.goto("https://seleniumbase.io/w3schools/iframes.html")
            .await?;
        sb.assert_title("iframe Testing").await?;
        sb.click("button#runbtn").await?;
        enter_frame(sb, mismatches, "iframeResult").await?; // Enter the iframe
        sb.assert_text("HTML Iframes", "h2").await?;
        sb.switch_to_frame("[title*=\"Iframe\"]").await?; // Enter iframe inside iframe
        sb.assert_text("This page is displayed in an iframe", "h1")
            .await?;
        sb.switch_to_parent_frame().await?; // Exit only the inner iframe
        sb.assert_text("Use CSS width & height to specify", "p")
            .await?;
        sb.switch_to_frame("[title*=\"Iframe\"]").await?; // Enter iframe inside iframe
        sb.assert_text("seleniumbase.io/w3schools/iframes", "a")
            .await?;
        sb.switch_to_default_content().await?; // Exit all iframes
        sb.click("button#runbtn").await?;
        enter_frame(sb, mismatches, "iframeResult").await?; // Go back inside 1st iframe
        sb.highlight("iframe[title=\"Iframe Example\"]").await
    })
    .await
}

#[tokio::test]
#[ignore = "real headless Chrome; opens seleniumbase.io/w3schools/iframes.html"]
async fn iframe_tests_iframes_with_context_manager() -> Result<()> {
    run_port(async |sb, mismatches| {
        sb.goto("https://seleniumbase.io/w3schools/iframes.html")
            .await?;
        sb.assert_title("iframe Testing").await?;
        sb.click("button#runbtn").await?;
        // MISMATCH: Python's frame_switch is a context manager that returns to
        // the parent frame when the `with` block ends. frame_switch here only
        // enters the frame, so each block ends with switch_to_parent_frame.
        enter_frame_switch(sb, mismatches, "iframeResult").await?;
        sb.assert_text("HTML Iframes", "h2").await?;
        sb.frame_switch("[title*=\"Iframe\"]").await?;
        sb.assert_text("This page is displayed in an iframe", "h1")
            .await?;
        sb.switch_to_parent_frame().await?;
        sb.assert_text("Use CSS width & height to specify", "p")
            .await?;
        sb.frame_switch("[title*=\"Iframe\"]").await?;
        sb.assert_text("seleniumbase.io/w3schools/iframes", "a")
            .await?;
        sb.switch_to_parent_frame().await?;
        sb.switch_to_parent_frame().await?;
        sb.click("button#runbtn").await?;
        enter_frame_switch(sb, mismatches, "iframeResult").await?;
        sb.highlight("iframe[title=\"Iframe Example\"]").await?;
        sb.switch_to_parent_frame().await
    })
    .await
}

#[tokio::test]
#[ignore = "real headless Chrome; opens seleniumbase.io/w3schools/iframes.html"]
async fn iframe_tests_set_content_to_frame() -> Result<()> {
    run_browser_test(headless(), async |sb| {
        sb.goto("https://seleniumbase.io/w3schools/iframes.html")
            .await?;
        sb.assert_title("iframe Testing").await?;
        sb.click("button#runbtn").await?;
        sb.set_content_to_frame("iframeResult").await?;
        sb.highlight("iframe[title=\"Iframe Example\"]").await?;
        sb.set_content_to_frame("iframe").await?;
        sb.assert_element_not_visible("iframe").await?;
        sb.highlight("body").await?;
        sb.set_content_to_parent().await?;
        sb.highlight("iframe[title=\"Iframe Example\"]").await?;
        sb.set_content_to_default().await?;
        sb.click("button#runbtn").await?;
        sb.highlight("#iframeResult").await
    })
    .await
}

// ---------------------------------------------------------------------------
// test_window_switching.py
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "real headless Chrome; opens data: pages and a second browser"]
async fn test_window_switching() -> Result<()> {
    run_browser_test(headless(), async |sb| {
        sb.goto("about:blank").await?;
        sb.get_new_driver().await?;
        sb.goto("data:text/html,<h1>Page A</h1>").await?;
        sb.assert_text("Page A", "html").await?;
        sb.open_new_window().await?;
        sb.goto("data:text/html,<h1>Page B</h1>").await?;
        sb.assert_text("Page B", "html").await?;
        // MISMATCH: switch_to_window takes only a window handle. Python takes
        // an index too (0 for the first tab, -1 for the newest). The nearest
        // are switch_to_tab(index) and switch_to_newest_tab.
        sb.switch_to_tab(0).await?;
        sb.assert_text("Page A", "html").await?;
        sb.assert_text_not_visible("Page B", "html").await?;
        sb.switch_to_newest_tab().await?;
        sb.assert_text("Page B", "html").await?;
        sb.assert_text_not_visible("Page A", "html").await
    })
    .await
}

// ---------------------------------------------------------------------------
// test_coffee_cart.py
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "real headless Chrome; opens seleniumbase.io/coffee"]
async fn test_coffee_cart() -> Result<()> {
    run_browser_test(headless(), async |sb| {
        sb.goto("https://seleniumbase.io/coffee/").await?;
        sb.assert_title("Coffee Cart").await?;
        sb.assert_element("button:contains(\"Total: $0.00\")")
            .await?;
        sb.click("div[data-sb=\"Cappuccino\"]").await?;
        sb.assert_exact_text("cart (1)", "a[aria-label=\"Cart page\"]")
            .await?;
        sb.click("div[data-sb=\"Flat-White\"]").await?;
        sb.assert_exact_text("cart (2)", "a[aria-label=\"Cart page\"]")
            .await?;
        sb.click("div[data-sb=\"Cafe-Latte\"]").await?;
        sb.assert_exact_text("cart (3)", "a[aria-label=\"Cart page\"]")
            .await?;
        sb.click("a[aria-label=\"Cart page\"]").await?;
        sb.assert_exact_text("Total: $53.00", "button.pay").await?;
        sb.click("button.pay").await?;
        sb.type_text("input#name", "Selenium Coffee").await?;
        sb.type_text("input#email", "test@test.test").await?;
        sb.click("button#submit-payment").await?;
        sb.assert_text("Thanks for your purchase.", "#app .success")
            .await
    })
    .await
}

// ---------------------------------------------------------------------------
// test_assert_elements.py
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "real headless Chrome; opens seleniumbase.io/demo_page"]
async fn test_assert_elements() -> Result<()> {
    run_browser_test(headless(), async |sb| {
        sb.goto("https://seleniumbase.io/demo_page").await?;
        sb.assert_elements_present(&["head", "style", "script"])
            .await?;
        sb.assert_elements(&["h1", "h2", "h3"]).await?;
        let my_list = ["#myDropdown", "#myButton", "#mySlider"];
        sb.assert_elements(&my_list).await
    })
    .await
}

// ---------------------------------------------------------------------------
// test_checkboxes.py
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "real headless Chrome; opens seleniumbase.io/w3schools pages"]
async fn test_checkboxes() -> Result<()> {
    run_port(async |sb, mismatches| {
        sb.goto("https://seleniumbase.io/w3schools/checkboxes")
            .await?;
        sb.click("button#runbtn").await?;
        enter_frame(sb, mismatches, "iframeResult").await?;
        let checkbox = "input#vehicle2";
        sb.assert_false(sb.is_selected(checkbox).await?)?;
        sb.click(checkbox).await?;
        sb.assert_true(sb.is_selected(checkbox).await?)?;
        sb.goto("https://seleniumbase.io/w3schools/radio_buttons")
            .await?;
        sb.click("button#runbtn").await?;
        enter_frame(sb, mismatches, "iframeResult").await?;
        let option_button = "input#css";
        sb.assert_false(sb.is_selected(option_button).await?)?;
        sb.click(option_button).await?;
        sb.assert_true(sb.is_selected(option_button).await?)
    })
    .await
}

// ---------------------------------------------------------------------------
// test_select_options.py
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "real headless Chrome; opens seleniumbase.io/demo_page"]
async fn test_select_options() -> Result<()> {
    run_port(async |sb, _| {
        sb.goto("https://seleniumbase.io/demo_page").await?;

        let expected_option_texts = ["Set to 25%", "Set to 50%", "Set to 75%", "Set to 100%"]
            .map(str::to_owned)
            .to_vec();
        let option_texts = sb.get_select_options("select#mySelect").await?;
        sb.assert_equal(&option_texts, &expected_option_texts)?;

        let expected_option_indexes = ["0", "1", "2", "3"].map(str::to_owned).to_vec();
        let option_indexes = sb
            .get_select_options("select#mySelect")
            .attribute("index")
            .await?;
        sb.assert_equal(&option_indexes, &expected_option_indexes)?;

        let expected_option_values = ["25%", "50%", "75%", "100%"].map(str::to_owned).to_vec();
        let option_values = sb
            .get_select_options("select#mySelect")
            .attribute("value")
            .await?;

        sb.assert_equal(&option_values, &expected_option_values)?;

        for (index, option_text) in option_texts.iter().enumerate() {
            sb.select_option_by_text("#mySelect", option_text).await?;
            let selected_value = sb.get_attribute("#mySelect", "value").await?;
            sb.assert_equal(
                selected_value.as_deref(),
                Some(option_values[index].as_str()),
            )?;
        }

        for (index, option_value) in option_values.iter().enumerate() {
            sb.select_option_by_value("#mySelect", option_value).await?;
            let selected_value = sb.get_attribute("#mySelect", "value").await?;
            sb.assert_equal(
                selected_value.as_deref(),
                Some(option_values[index].as_str()),
            )?;
        }

        for (index, option_index) in option_indexes.iter().enumerate() {
            sb.select_option_by_index("#mySelect", option_index.parse().unwrap_or(0))
                .await?;
            // assert_attribute() combines get_attribute() and assert_equal()
            sb.assert_attribute("#mySelect", "value", &option_values[index])
                .await?;
        }
        Ok(())
    })
    .await
}

// ---------------------------------------------------------------------------
// shadow_root_test.py
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "real headless Chrome; opens seleniumbase.io/other/shadow_dom"]
async fn shadow_root_test() -> Result<()> {
    run_port(async |sb, mismatches| {
        sb.goto("https://seleniumbase.io/other/shadow_dom").await?;
        sb.click("button.tab_1").await?;
        let panel = shadow_text(sb, mismatches, "fancy-tabs::shadow #panels").await?;
        println!("{panel}");
        assert_shadow_exact(
            sb,
            mismatches,
            "Content Panel 1",
            "fancy-tabs::shadow #panels",
        )
        .await?;
        sb.click("button.tab_2").await?;
        let panel = shadow_text(sb, mismatches, "fancy-tabs::shadow #panels").await?;
        println!("{panel}");
        assert_shadow_exact(
            sb,
            mismatches,
            "Content Panel 2",
            "fancy-tabs::shadow #panels",
        )
        .await?;
        sb.click("button.tab_3").await?;
        let panel = shadow_text(sb, mismatches, "fancy-tabs::shadow #panels").await?;
        println!("{panel}");
        assert_shadow_exact(
            sb,
            mismatches,
            "Content Panel 3",
            "fancy-tabs::shadow #panels",
        )
        .await
    })
    .await
}

// ---------------------------------------------------------------------------
// test_shadow_dom.py
// ---------------------------------------------------------------------------

/// The Python test skips itself in headless mode, and its first half
/// downloads two packages from pypi.org (not a site made for automation
/// tests). This port keeps the second half: piercing the five levels of
/// shadow roots of `chrome://downloads/` with `::shadow` selectors.
#[tokio::test]
#[ignore = "real headless Chrome; opens chrome://downloads/ (the Python test skips headless)"]
async fn test_shadow_dom() -> Result<()> {
    run_port(async |sb, mismatches| {
        sb.goto("chrome://downloads/").await?;

        // Shadow DOM selectors
        let search_icon = "downloads-manager::shadow downloads-toolbar::shadow \
     cr-toolbar::shadow cr-toolbar-search-field::shadow cr-icon-button";
        let search_input = "downloads-manager::shadow downloads-toolbar::shadow \
     cr-toolbar::shadow cr-toolbar-search-field::shadow #searchInput";
        let clear_search_icon = "downloads-manager::shadow downloads-toolbar::shadow \
     cr-toolbar::shadow cr-toolbar-search-field::shadow #clearSearch";
        let no_downloads_area = "downloads-manager::shadow #no-downloads";

        shadow_assert_element(sb, mismatches, search_icon).await?;
        shadow_type(sb, mismatches, search_input, "fake-file.zzz").await?;
        shadow_assert_text(sb, mismatches, "No search results found", no_downloads_area).await?;
        shadow_click(sb, mismatches, clear_search_icon).await
    })
    .await
}

// ---------------------------------------------------------------------------
// test_double_click.py
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "real headless Chrome; opens seleniumbase.io/w3schools/double_click"]
async fn test_double_click_switch_to_frame_and_double_click() -> Result<()> {
    run_browser_test(headless(), async |sb| {
        sb.goto("https://seleniumbase.io/w3schools/double_click")
            .await?;
        sb.assert_title("Double Click Testing").await?;
        sb.click("button#runbtn").await?;
        sb.switch_to_frame("iframe#iframeResult").await?;
        sb.double_click("[ondblclick=\"myFunction()\"]").await?;
        sb.assert_text("Hello World", "#demo").await
    })
    .await
}

#[tokio::test]
#[ignore = "real headless Chrome; opens seleniumbase.io/w3schools/double_click"]
async fn test_double_click_switch_to_frame_of_element_and_double_click() -> Result<()> {
    run_browser_test(headless(), async |sb| {
        sb.goto("https://seleniumbase.io/w3schools/double_click")
            .await?;
        sb.assert_title("Double Click Testing").await?;
        sb.click("button#runbtn").await?;
        sb.switch_to_frame_of_element("[ondblclick=\"myFunction()\"]")
            .await?;
        sb.double_click("[ondblclick=\"myFunction()\"]").await?;
        sb.assert_text("Hello World", "#demo").await
    })
    .await
}

// ---------------------------------------------------------------------------
// test_drag_and_drop.py
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "real headless Chrome; opens seleniumbase.io/other/drag_and_drop"]
async fn test_drag_and_drop_drag_and_drop() -> Result<()> {
    run_browser_test(headless(), async |sb| {
        sb.goto("https://seleniumbase.io/other/drag_and_drop")
            .await?;
        sb.assert_element_not_visible("#div1 img#drag1").await?;
        sb.drag_and_drop("#drag1", "#div1").await?;
        sb.assert_element("#div1 img#drag1").await?;
        sb.sleep(0.8).await;
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "real headless Chrome; opens seleniumbase.io/w3schools/drag_drop"]
async fn test_drag_and_drop_w3schools_drag_and_drop() -> Result<()> {
    run_port(async |sb, mismatches| {
        sb.goto("https://seleniumbase.io/w3schools/drag_drop")
            .await?;
        sb.assert_url_contains("drag_drop").await?;
        sb.click("button#runbtn").await?;
        enter_frame(sb, mismatches, "iframeResult").await?;
        sb.assert_element_not_visible("#div1 img#drag1").await?;
        sb.drag_and_drop("#drag1", "#div1").await?;
        sb.assert_element("#div1 img#drag1").await?;
        sb.sleep(0.8).await;
        Ok(())
    })
    .await
}

// ---------------------------------------------------------------------------
// test_url_asserts.py
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "real headless Chrome; opens seleniumbase.io/help_docs/how_it_works"]
async fn test_url_asserts() -> Result<()> {
    run_port(async |sb, _| {
        sb.goto("https://seleniumbase.io/help_docs/how_it_works/")
            .await?;
        sb.assert_url("https://seleniumbase.io/help_docs/how_it_works/")
            .await?;
        sb.assert_title_contains("How it Works").await?;
        sb.js_click("nav a:contains(\"Coffee Cart\")").await?;
        sb.assert_url_contains("/coffee").await?;
        sb.assert_title("Coffee Cart").await
    })
    .await
}

// ---------------------------------------------------------------------------
// test_deferred_asserts.py
// ---------------------------------------------------------------------------

/// The Python test is marked `expected_failure`: four of its eight deferred
/// asserts fail on purpose. The port runs the same steps and checks that
/// `process_deferred_asserts` reports those four, and only those.
#[tokio::test]
#[ignore = "real headless Chrome; opens xkcd.com/993"]
async fn test_deferred_asserts() -> Result<()> {
    run_browser_test(headless(), async |sb| {
        sb.goto("https://xkcd.com/993/").await?;
        sb.wait_for_element("#comic").await?;
        sb.deferred_assert_element("img[alt=\"Brand Identity\"]")
            .await?;
        sb.deferred_assert_element("img[alt=\"Rocket Ship\"]")
            .await?; // Will Fail
        sb.deferred_assert_element("#comicmap").await?;
        sb.deferred_assert_text("Fake Item", "ul.comicNav").await?; // Will Fail
        sb.deferred_assert_text("Random", "ul.comicNav").await?;
        sb.deferred_assert_element("a[name=\"Super Fake !!!\"]")
            .await?; // Will Fail
        sb.deferred_assert_exact_text("Brand Identity", "#ctitle")
            .await?;
        sb.deferred_assert_exact_text("Fake Food", "#comic").await?; // Will Fail
        let report = sb
            .process_deferred_asserts()
            .await
            .expect_err("four deferred asserts fail")
            .to_string();
        for failed in ["Rocket Ship", "Fake Item", "Super Fake", "Fake Food"] {
            assert!(report.contains(failed), "{failed} is in: {report}");
        }
        for passed in ["#comicmap", "\"Random\"", "#ctitle"] {
            assert!(!report.contains(passed), "{passed} is not in: {report}");
        }
        Ok(())
    })
    .await
}

// ---------------------------------------------------------------------------
// handle_alert_test.py
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "real headless Chrome; opens about:blank and raises JavaScript alerts"]
async fn handle_alert_test() -> Result<()> {
    run_browser_test(headless(), async |sb| {
        sb.goto("about:blank").await?;
        sb.execute_script("window.alert(\"ALERT!!!\");").await?;
        sb.sleep(1.0).await; // Not needed (Lets you see the alert pop up)
        sb.assert_true(sb.is_alert_present().await?)?;
        sb.accept_alert().await?;
        sb.sleep(1.0).await; // Not needed (Lets you see the alert go away)
        sb.execute_script("window.prompt(\"My Prompt\",\"defaultText\");")
            .await?;
        sb.sleep(1.0).await; // Not needed (Lets you see the alert pop up)
                             // MISMATCH: switch_to_alert returns nothing; Python returns the alert
                             // and the example reads `alert.text`. get_alert_text is the nearest.
        sb.switch_to_alert().await?;
        let text = sb.get_alert_text().await?;
        sb.assert_equal(text.as_str(), "My Prompt")?; // Not input field
        sb.dismiss_alert().await?;
        sb.sleep(1.0).await; // Not needed (Lets you see the alert go away)
        sb.assert_false(sb.is_alert_present().await?)
    })
    .await
}

// ---------------------------------------------------------------------------
// xpath_test.py
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "real headless Chrome; opens seleniumbase.io/demo_page"]
async fn xpath_test() -> Result<()> {
    run_browser_test(headless(), async |sb| {
        sb.goto("https://seleniumbase.io/demo_page").await?;
        sb.assert_element("//h1[(text()=\"Demo Page\")]").await?;
        sb.type_text("//*[@id=\"myTextInput\"]", "XPath Test!")
            .await?;
        sb.click("//button[starts-with(text(),\"Click Me\")]")
            .await?;
        sb.assert_element("//button[contains(., \"Purple\")]")
            .await?;
        sb.assert_text("SeleniumBase", "//table/tbody/tr[1]/td[2]/h2")
            .await
    })
    .await
}

// ---------------------------------------------------------------------------
// test_simple_login.py
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "real headless Chrome; opens seleniumbase.io/simple/login"]
async fn test_simple_login() -> Result<()> {
    run_browser_test(headless(), async |sb| {
        sb.goto("https://seleniumbase.io/simple/login").await?;
        sb.type_text("#username", "demo_user").await?;
        sb.type_text("#password", "secret_pass").await?;
        sb.click("a:contains(\"Sign in\")").await?;
        sb.assert_exact_text("Welcome!", "h1").await?;
        sb.assert_element("img#image1").await?;
        sb.highlight("#image1").await?;
        sb.click_link("Sign out").await?;
        sb.assert_text("signed out", "#top_message").await
    })
    .await
}

// ---------------------------------------------------------------------------
// test_contains_selector.py
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "real headless Chrome; opens xkcd.com/2207"]
async fn test_contains_selector() -> Result<()> {
    run_browser_test(headless(), async |sb| {
        sb.goto("https://xkcd.com/2207/").await?;
        sb.assert_element("div.box div:contains(\"Math Work\")")
            .await?;
        sb.click("a:contains(\"Next\")").await?;
        sb.assert_element("div div:contains(\"Drone Fishing\")")
            .await
    })
    .await
}

// ---------------------------------------------------------------------------
// test_cycle_elements.py
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "real headless Chrome; opens seleniumbase.io/demo_page"]
async fn test_cycle_elements_with_tab_and_press_enter() -> Result<()> {
    run_browser_test(headless(), async |sb| {
        // Python: goto("seleniumbase.io/demo_page"), a URL with no scheme.
        sb.goto("seleniumbase.io/demo_page").await?;
        sb.assert_text("This Text is Green", "#pText").await?;
        sb.send_keys("html", "\t\t\t\t\n").await?;
        sb.assert_text("This Text is Purple", "#pText").await
    })
    .await
}

// ---------------------------------------------------------------------------
// test_highlight_elements.py
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "real headless Chrome; opens seleniumbase.io/demo_page"]
async fn test_highlight_elements_highlight_inputs() -> Result<()> {
    run_browser_test(headless(), async |sb| {
        sb.goto("https://seleniumbase.io/demo_page").await?;
        // Python (headless): highlight_elements("input", loops=1, limit=3)
        sb.highlight_elements("input").loops(1).limit(3).await
    })
    .await
}

// ---------------------------------------------------------------------------
// test_tinymce.py
// ---------------------------------------------------------------------------

/// The Python test skips itself in headless mode. The port runs its steps
/// anyway, to see whether this crate can drive the TinyMCE editor headless.
#[tokio::test]
#[ignore = "real headless Chrome; opens seleniumbase.io/tinymce (the Python test skips headless)"]
async fn test_tinymce() -> Result<()> {
    run_browser_test(headless(), async |sb| {
        sb.goto("https://seleniumbase.io/tinymce/").await?;
        sb.wait_for_element("div.mce-container-body").await?;
        sb.click("span:contains(\"File\")").await?;
        sb.click("span:contains(\"New document\")").await?;
        sb.click("span:contains(\"Paragraph\")").await?;
        sb.click("span:contains(\"Heading 2\")").await?;
        sb.switch_to_frame("iframe").await?;
        sb.add_text("#tinymce", "Automate anything with SeleniumBase!\n")
            .await?;
        sb.switch_to_parent_frame().await?;
        sb.click("button i.mce-i-image").await?;
        sb.type_text("input[aria-label=\"Width\"].mce-textbox", "300")
            .await?;
        let image_url = "https://seleniumbase.github.io/img/sb_logo_10.png";
        sb.type_text("input.mce-textbox", &format!("{image_url}\n"))
            .await?;
        sb.frame_switch("iframe").await?;
        sb.click("h2").await?;
        sb.post_message("Automate anything with SeleniumBase!")
            .await?;
        sb.switch_to_parent_frame().await?;
        sb.click("span:contains(\"File\")").await?;
        sb.click("span:contains(\"Preview\")").await?;
        sb.switch_to_frame("iframe[sandbox=\"allow-scripts\"]")
            .await?;
        sb.post_message("Learn SeleniumBase Today!").await
    })
    .await
}

// ---------------------------------------------------------------------------
// test_todomvc.py (parameterized over jquery, react and vue)
// ---------------------------------------------------------------------------

/// The body of `test_todomvc(framework)`, with the `setUp` that opens a new
/// window and the `tearDown` that closes it.
async fn todomvc(framework: &str) -> Result<()> {
    run_port(async |sb, _| {
        sb.open_new_window().await?;
        sb.goto("https://todomvc.com/").await?;
        sb.clear_local_storage().await?;
        sb.click(&format!("a[href*=\"examples/{framework}/dist\"]"))
            .await?;
        sb.assert_element("section.todoapp").await?;
        sb.assert_text("todos", "header h1").await?;
        sb.wait_for_ready_state_complete().await?;
        let title = sb.get_title().await?;
        // Python: self.assert_in(framework, title.lower())
        sb.assert_in(framework, &title.to_lowercase())?;
        let new_todo_input = "input.new-todo";
        let todo_count_span = "span.todo-count";
        sb.wait_for_ready_state_complete().await?;
        sb.type_text(new_todo_input, "Learn Python\n").await?;
        sb.type_text(new_todo_input, "Learn JavaScript\n").await?;
        sb.type_text(new_todo_input, "Learn SeleniumBase\n").await?;
        sb.assert_text("3 items left", todo_count_span).await?;
        sb.check_if_unchecked("ul.todo-list li input").await?;
        sb.check_if_unchecked("ul.todo-list li:nth-of-type(2) input")
            .await?;
        sb.check_if_unchecked("ul.todo-list li:nth-of-type(3) input")
            .await?;
        sb.assert_text("0 items left", todo_count_span).await?;
        sb.click("button.clear-completed").await?;
        sb.assert_element_not_visible(todo_count_span).await
    })
    .await
}

#[tokio::test]
#[ignore = "real headless Chrome; opens todomvc.com"]
async fn test_todomvc_jquery() -> Result<()> {
    todomvc("jquery").await
}

#[tokio::test]
#[ignore = "real headless Chrome; opens todomvc.com"]
async fn test_todomvc_react() -> Result<()> {
    todomvc("react").await
}

#[tokio::test]
#[ignore = "real headless Chrome; opens todomvc.com"]
async fn test_todomvc_vue() -> Result<()> {
    todomvc("vue").await
}
