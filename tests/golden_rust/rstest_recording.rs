use rstest::rstest;
use seleniumbase_rs::testing::{sb, Sb};
use seleniumbase_rs::Result;
use seleniumbase_rs::api::recorder::recorded_secret;

#[rstest]
#[tokio::test]
async fn test_recording(sb: Sb) -> Result<()> {
    sb.run(async |sb| {
        sb.goto("https://a.test/demo_page").await?;
        sb.click("#myButton").await?;
        sb.type_text("#myTextInput", "hello").await?;
        sb.select_option_by_text("#mySelect", "Set to 75%").await?;
        sb.check_if_unchecked("#checkBox1").await?;
        sb.assert_element("h1").await?;
        sb.assert_text("Demo Page", "h1").await?;
        sb.assert_exact_text("Demo Page", "h1").await?;
        sb.assert_exact_text("Demo Page", "html").await?;
        sb.double_click("#dbl").await?;
        sb.type_text("#myTextInput", "again").await?;
        sb.goto("https://a.test/form").await?;
        sb.js_click("label[for=\"cb\"]").await?;
        sb.click("#cb2").await?;
        sb.uncheck_if_checked("#cb2").await?;
        sb.click("#cb3").await?;
        sb.js_click("#cb4").await?;
        sb.jquery_click("#cb5").await?;
        sb.jquery_click("#cb6").await?;
        sb.uncheck_if_checked("#cb6").await?;
        sb.goto("https://a.test/").await?;
        sb.type_text("#q", "cats").await?;
        sb.type_text("#q", "cats\n").await?;
        sb.goto_if_not_url("https://a.test/search?q=cats").await?;
        sb.click("a[href=\"/images\"]").await?;
        sb.goto_if_not_url("https://a.test/images").await?;
        sb.click("h3 a").await?;
        sb.goto_if_not_url("https://other.test/page").await?;
        sb.assert_element("h1").await?;
        sb.goto("https://a.test/a b?c=é").await?;
        sb.click("button#go").await?;
        sb.goto_if_not_url("https://a.test/done").await?;
        sb.goto("https://a.test/api").await?;
        sb.double_click("#a").await?;
        sb.context_click("#ctx").await?;
        sb.hover("#h").await?;
        sb.highlight("#h").await?;
        sb.press_keys("#q", "abc").await?;
        sb.js_type("#j", "jj").await?;
        sb.jquery_type("#k", "kk").await?;
        sb.js_click("#jc").await?;
        sb.js_click_all(".all").await?;
        sb.jquery_click("#qc").await?;
        sb.jquery_click_all(".qall").await?;
        sb.assert_element_present("#e").await?;
        sb.assert_element_not_visible("#gone").await?;
        sb.wait_for_element("#slow", 10).await?;
        sb.assert_non_empty_text("#nonempty").await?;
        sb.assert_link_text("Home").await?;
        sb.assert_title("Title").await?;
        sb.assert_title_contains("Tit").await?;
        sb.assert_url("https://a.test/api").await?;
        sb.assert_url_contains("/api").await?;
        sb.assert_attribute("#a", "href", "/x").await?;
        sb.assert_attribute("#a", "disabled", "").await?;
        sb.set_attribute("#a", "title", "t").await?;
        sb.set_attributes(".a", "title", "t").await?;
        sb.accept_alert().await?;
        sb.dismiss_alert().await?;
        sb.sleep(1.5).await;
        sb.save_screenshot("shot.png").await?;
        // skipped: save_screenshot_to_folder: BaseCase::save_screenshot takes a file name only, not a folder
        sb.save_screenshot_to_logs().await?;
        // skipped: save_as_pdf_to_logs: BaseCase::save_as_pdf_to_logs needs a file name, which the recording does not hold
        sb.save_page_source_to_logs().await?;
        sb.go_back().await?;
        sb.go_forward().await?;
        sb.clear_local_storage().await?;
        sb.clear_session_storage().await?;
        sb.delete_all_cookies().await?;
        sb.activate_demo_mode().await?;
        sb.deactivate_demo_mode().await?;
        sb.process_deferred_asserts().await?;
        sb.deferred_assert_element("#d").await?;
        sb.deferred_assert_element_present("#d").await?;
        sb.deferred_assert_text("Text", "#t").await?;
        sb.deferred_assert_exact_text("Text", "#t").await?;
        sb.deferred_assert_non_empty_text("#n").await?;
        sb.assert_text_not_visible("Gone", "#g").await?;
        sb.assert_exact_text_not_visible("Gone", "#g").await?;
        // skipped: download_file: BaseCase::download_file takes a url, a file name and a timeout here, not Python's url and folder
        // skipped: download_file: BaseCase::download_file takes a url, a file name and a timeout here, not Python's url and folder
        sb.assert_downloaded_file("f.zip").await?;
        sb.enter_mfa_code("#mfa", "JBSWY3DPEHPK3PXP").await?;
        sb.set_value("#range", "7").await?;
        sb.show_file_choosers().await?;
        sb.switch_to_frame("iframe#f").await?;
        sb.set_content_to_frame("iframe#g").await?;
        sb.set_content_to_parent().await?;
        sb.set_content_to_default().await?;
        sb.switch_to_parent_frame().await?;
        sb.switch_to_default_content().await?;
        sb.goto("https://a.test/widgets").await?;
        sb.click_with_offset("#cv", 11, 21, false).await?;
        sb.click_with_offset("#cv", 5, 3, false).await?;
        sb.hover_and_click(".menu", ".menu .item").await?;
        sb.hover_and_click(".menu2", ".menu2 .item").await?;
        sb.drag_and_drop("#drag", "#drop").await?;
        sb.choose_file("#file", "/tmp/a b.txt").await?;
        sb.choose_file("#file2", "/tmp/c.txt").await?;
        sb.set_value("#range", "7").await?;
        sb.select_option_by_text("#sel", "Two \"quoted\"").await?;
        sb.goto("https://a.test/").await?;
        sb.type_text("#q", "rust\n").await?;
        sb.type_text("#q2", "go\n").await?;
        sb.type_text("#q3", "abc\n").await?;
        sb.type_text("#q4", "xyz\n").await?;
        sb.type_text("#ta", "textarea text").await?;
        sb.type_text("#ta", "textarea text\n").await?;
        sb.type_text("#password", &format!("{}{}", recorded_secret()?, "\n")).await?;
        Ok(())
    })
    .await
}
