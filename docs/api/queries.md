---
title: "Queries"
sidebarTitle: "Queries"
description: "Read text, attributes, properties, counts and page state."
---

<Note>

Methods are listed as `name(arguments)`, called on a `BaseCase` as `sb.name(...).await?`. The argument names, the selector syntax and the return types are explained in [Reading the API reference](/api/api_reference#reading-this-page).

</Note>

## Queries

| Method | Description |
|--------|-------------|
| `is_element_present(selector)` | Returns true if in DOM. |
| `is_element_visible(selector)` | Returns true if visible. |
| `is_element_enabled(selector)` | Returns true if enabled. |
| `is_selected(selector)` | Returns true if the element is selected. |
| `is_text_visible(text, selector)` | Returns true if text visible. |
| `text(selector)` | Returns element text. |
| `attribute(selector, attr)` | Returns what Selenium's `attribute` returns: the DOM property of that name when the element has one (what was typed into an `<input>`, not the `value=""` its markup holds; the chosen `value` of a `<select>`; the resolved `href` or `src` of a link or an image), and otherwise the attribute. `checked`, `selected` and the other boolean attributes are `"true"` or `None` by what the element is now. `None` when neither is set. |
| `select_options(selector)` | Returns the options of a `<select>` as text. `.attribute("value")` or `.attribute("index")` reads those of each option instead; `.timeout(..)` sets the wait. |
| `property(selector, prop)` | Returns property value. |
| `find_element(selector)` | Returns an [`Element`](/api/crate-root): the element the browser found, waited for up to 10 seconds. Every selector, in this table and elsewhere, may cross shadow roots (`host::shadow inner`): see the [Shadow DOM guide](/guides/write-tests/selectors#shadow-dom). |
| `find_elements(selector)` | Returns every match as an `Element`, possibly none. |
| `unique_links()` | Returns unique `href` values from the page. |
| `all_links()` | Returns all `href` values from the page. |
| `link_status_code(url)` | Returns the HTTP status code for `url`. |
| `metadata()` | Extracts structured metadata (`PageMetadata`: SEO title, description, OpenGraph, Twitter Cards, canonical URL) from the current page's HTML `<head>`. |
| `json_ld::<T>()` | Extracts and deserializes Schema.org JSON-LD structured data (`Vec<T>`) from `<script type="application/ld+json">` tags, flattening top-level arrays and `@graph` structures. |
| `table(selector)` | Extracts an HTML `<table>` matching `selector` into structured `TableData` with headers and rows. |
| `markdown()` | Converts the current page HTML into clean, LLM-ready Markdown, stripping navigation, footers, scripts, and non-content elements. |
