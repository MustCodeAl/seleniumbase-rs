// Data extraction methods for `BaseCase`.
// Included in `base_case.rs`.

use crate::extract::{extract_json_ld, PageMetadata, TableData};

impl BaseCase {
    /// Extracts structured metadata (SEO, `OpenGraph`, Twitter Cards, canonical URL)
    /// from the current page's HTML `<head>`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError`] if reading the page source fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use seleniumbase_rs::BaseCase;
    ///
    /// # async fn demo(mut sb: BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// sb.open("https://example.com").await?;
    /// let meta = sb.metadata().await?;
    /// println!("Page Title: {:?}", meta.title);
    /// println!("OG Image: {:?}", meta.og_image);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn metadata(&self) -> Result<PageMetadata, SeleniumBaseError> {
        let source = self.page_source().await?;
        Ok(PageMetadata::from_html(&source))
    }

    /// Extracts and deserializes Schema.org JSON-LD structured data from `<script type="application/ld+json">` tags.
    ///
    /// Flattens top-level arrays and Schema.org `@graph` structures.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError`] if reading the page source fails or if JSON deserialization fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use seleniumbase_rs::BaseCase;
    /// use serde::Deserialize;
    ///
    /// #[derive(Debug, Deserialize)]
    /// struct Product {
    ///     name: String,
    /// }
    ///
    /// # async fn demo(mut sb: BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// sb.open("https://shop.example.com/item").await?;
    /// let products: Vec<Product> = sb.json_ld().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn json_ld<T: serde::de::DeserializeOwned>(&self) -> Result<Vec<T>, SeleniumBaseError> {
        let source = self.page_source().await?;
        extract_json_ld(&source)
    }

    /// Extracts an HTML `<table>` matching `selector` into structured headers and rows.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError`] if the element is not found or is not a valid table.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use seleniumbase_rs::BaseCase;
    ///
    /// # async fn demo(mut sb: BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// sb.open("https://example.com/stats").await?;
    /// let table = sb.table("#metrics").await?;
    /// println!("Headers: {:?}", table.headers());
    /// println!("Rows count: {}", table.len());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn table(&mut self, selector: impl Into<Selector>) -> Result<TableData, SeleniumBaseError> {
        let el = self.find_element(selector).await?;
        let html = el.html().await?;
        TableData::from_html(&html)
    }

    /// Converts the current page HTML into clean, LLM-ready Markdown.
    ///
    /// Automatically strips navigation, footers, scripts, and non-content elements.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError`] if reading the page source fails.
    pub async fn markdown(&self) -> Result<String, SeleniumBaseError> {
        let source = self.page_source().await?;
        Ok(crate::extract::html_to_markdown(&source))
    }

    /// Automatically fills in form inputs matching the fields of a serializable struct.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError`] if form serialization or element interaction fails.
    pub async fn fill_form<T: serde::Serialize>(&mut self, form_data: &T) -> Result<(), SeleniumBaseError> {
        let data = crate::extract::FormData::from_serializable(form_data)?;
        for (field, value) in data.iter() {
            let sel = format!("input[name='{field}'], input[id='{field}'], textarea[name='{field}'], select[name='{field}']");
            if let Ok(el) = self.find_element(&sel).await {
                let _ = el.send_keys(value).await;
            }
        }
        Ok(())
    }
}
