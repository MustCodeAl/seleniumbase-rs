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
}
