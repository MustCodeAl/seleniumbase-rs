//! Type-safe data extraction from HTML and web pages.
//!
//! Provides utilities for extracting structured data without boilerplate:
//! - [`PageMetadata`]: OpenGraph, Twitter Cards, SEO meta tags, and canonical URLs.
//! - [`extract_json_ld`]: Schema.org JSON-LD extraction and deserialization into typed structs.
//! - [`TableData`]: HTML `<table>` parsing into columns, rows, and typed record deserialization.

pub mod arena;
pub mod json_ld;
pub mod metadata;
pub mod table;

#[doc(inline)]
pub use arena::{PageArena, TableDataArena};
#[doc(inline)]
pub use json_ld::{extract_json_ld, extract_json_ld_raw};
#[doc(inline)]
pub use metadata::PageMetadata;
#[doc(inline)]
pub use table::TableData;
