//! Dropdown select helper matching Python Selenium's `selenium.webdriver.support.select.Select`.

use crate::browser::element::Element;
use crate::error::SeleniumBaseError;
use crate::sb_cdp::SelectBy;

/// Dropdown `<select>` element helper matching Python Selenium's `Select(webelement)`.
///
/// # Examples
///
/// ```no_run
/// use seleniumbase_rs::{BaseCase, Select};
///
/// # async fn demo(mut sb: BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
/// let dropdown = sb.find_element("#pick").await?;
/// let select = Select::new(&dropdown);
///
/// select.select_by_visible_text("Option 1").await?;
/// select.select_by_value("opt1").await?;
/// select.select_by_index(1).await?;
///
/// let all = select.options().await?;
/// let chosen = select.first_selected_option().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Select<'a> {
    element: &'a Element,
}

impl<'a> Select<'a> {
    /// Wraps an `<select>` element.
    #[must_use]
    pub fn new(element: &'a Element) -> Self {
        Self { element }
    }

    /// Selects an option by its displayed visible text.
    ///
    /// Corresponds to Python Selenium's `select_by_visible_text(text)`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError`] if selecting fails or element is not found.
    pub async fn select_by_visible_text(&self, text: &str) -> Result<(), SeleniumBaseError> {
        self.element.select_option(SelectBy::Text(text)).await
    }

    /// Selects an option by its `value` attribute.
    ///
    /// Corresponds to Python Selenium's `select_by_value(value)`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError`] if selecting fails or element is not found.
    pub async fn select_by_value(&self, value: &str) -> Result<(), SeleniumBaseError> {
        self.element.select_option(SelectBy::Value(value)).await
    }

    /// Selects an option by its 0-based index.
    ///
    /// Corresponds to Python Selenium's `select_by_index(index)`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError`] if selecting fails or element is not found.
    pub async fn select_by_index(&self, index: usize) -> Result<(), SeleniumBaseError> {
        self.element.select_option(SelectBy::Index(index)).await
    }

    /// Returns all `<option>` elements belonging to this select tag.
    ///
    /// Corresponds to Python Selenium's `options`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError`] if element query fails.
    pub async fn options(&self) -> Result<Vec<Element>, SeleniumBaseError> {
        self.element.find_all("option").await
    }

    /// Returns all selected `<option>` elements.
    ///
    /// Corresponds to Python Selenium's `all_selected_options`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError`] if element query fails.
    pub async fn all_selected_options(&self) -> Result<Vec<Element>, SeleniumBaseError> {
        self.element.find_all("option:checked").await
    }

    /// Returns the first selected `<option>` element, or `None` if none is selected.
    ///
    /// Corresponds to Python Selenium's `first_selected_option`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError`] if element query fails.
    pub async fn first_selected_option(&self) -> Result<Option<Element>, SeleniumBaseError> {
        let selected = self.all_selected_options().await?;
        Ok(selected.into_iter().next())
    }

    /// Deselects all selected options (for multi-select dropdowns).
    ///
    /// Corresponds to Python Selenium's `deselect_all()`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError`] if deselecting fails.
    pub async fn deselect_all(&self) -> Result<(), SeleniumBaseError> {
        let selected = self.all_selected_options().await?;
        for opt in selected {
            opt.click().await?;
        }
        Ok(())
    }

    /// Deselects all options that display text matching `text`.
    ///
    /// Corresponds to Python Selenium's `deselect_by_visible_text(text)`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError`] if deselecting fails.
    pub async fn deselect_by_visible_text(&self, text: &str) -> Result<(), SeleniumBaseError> {
        let opts = self.all_selected_options().await?;
        for opt in opts {
            if opt.text().await? == text {
                opt.click().await?;
            }
        }
        Ok(())
    }

    /// Deselects all options whose `value` attribute matches `value`.
    ///
    /// Corresponds to Python Selenium's `deselect_by_value(value)`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError`] if deselecting fails.
    pub async fn deselect_by_value(&self, value: &str) -> Result<(), SeleniumBaseError> {
        let opts = self.all_selected_options().await?;
        for opt in opts {
            if opt.attribute("value").await?.as_deref() == Some(value) {
                opt.click().await?;
            }
        }
        Ok(())
    }

    /// Deselects the option at the given index.
    ///
    /// Corresponds to Python Selenium's `deselect_by_index(index)`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError`] if deselecting fails.
    pub async fn deselect_by_index(&self, index: usize) -> Result<(), SeleniumBaseError> {
        let all_opts = self.options().await?;
        if let Some(opt) = all_opts.get(index)
            && opt.is_selected().await?
        {
            opt.click().await?;
        }
        Ok(())
    }
}
