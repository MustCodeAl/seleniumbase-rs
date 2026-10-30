//! `#[async_trait]` for a test plugin, imported from `seleniumbase_rs` alone.
//!
//! The manifest of this crate does not list `async-trait`: the attribute is re-exported by
//! `seleniumbase_rs::plugins::observer`, next to the traits it is written for.

use seleniumbase_rs::plugins::observer::{async_trait, PageEvidence};
use seleniumbase_rs::{tokio, Result};

/// A page that has nothing in it, to implement the trait on.
struct EmptyPage;

#[async_trait]
impl PageEvidence for EmptyPage {
    async fn screenshot_png(&self) -> Result<Vec<u8>> {
        Ok(Vec::new())
    }

    async fn page_source(&self) -> Result<String> {
        Ok("<html></html>".to_owned())
    }
}

#[tokio::test]
async fn a_page_type_of_ours_implements_the_evidence_trait() {
    let page = EmptyPage;
    assert_eq!(page.page_source().await.unwrap(), "<html></html>");
    assert!(page.screenshot_png().await.unwrap().is_empty());
}
