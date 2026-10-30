//! Checks of the form page.

mod page_objects;
use seleniumbase_rs::{BaseCase, SeleniumBaseError};

/// Fills in the form and opens the menu.
#[rustfmt::skip]
pub async fn fill_in_the_form(sb: &mut BaseCase) -> Result<(), SeleniumBaseError> {
    sb.open("https://example.com/form").await?;
    sb.type_text(page_objects::example_com_form::NAME, "Ada").await?;
    sb.click(page_objects::example_com_form::BUTTON_SEND).await?;
    sb.assert_text("Thanks", page_objects::example_com_form::H2).await?;
    sb.hover_and_click(page_objects::example_com_form::MENU, page_objects::example_com_form::MENU_ITEM).await?;
    sb.select_option_by_text(page_objects::example_com_form::COUNTRY, "Sweden").await?;
    sb.assert_element(page_objects::example_com_form::A_HREF_OTHER).await?;
    Ok(())
}

/// Logs in on the second page.
#[rustfmt::skip]
pub async fn log_in(sb: &mut BaseCase) -> Result<(), SeleniumBaseError> {
    sb.open("https://example.com/login").await?;
    sb.type_text(page_objects::example_com_login::USER, "ada").await?;
    sb.type_text(page_objects::example_com_login::PASSWORD, "secret").await?;
    sb.click(page_objects::example_com_login::BUTTON_SEND).await?;
    Ok(())
}

#[test]
fn the_pages_exist() {
    let _ = (fill_in_the_form, log_in);
}
