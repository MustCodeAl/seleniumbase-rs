---
title: "Queries"
sidebarTitle: "Queries"
description: "Read text, attributes, properties, counts and page state."
---

<Note>

Methods are listed as `name(arguments)`, called on a `BaseCase` as `sb.name(...).await?`. The argument names, the selector syntax and the return types are explained in [Reading the API reference](/tutorials/api_reference#reading-this-page).

</Note>

## Queries

| Method | Description |
|--------|-------------|
| `is_element_present(selector)` | Returns true if in DOM. |
| `is_element_visible(selector)` | Returns true if visible. |
| `is_element_enabled(selector)` | Returns true if enabled. |
| `is_selected(selector)` | Returns true if the element is selected. |
| `is_text_visible(text, selector)` | Returns true if text visible. |
| `get_text(selector)` | Returns element text. |
| `get_attribute(selector, attr)` | Returns the attribute value; when the element has no such attribute, its DOM property of that name, as Selenium does (the chosen `value` of a `<select>`, what was typed into an `<input>`). `None` when neither is set. |
| `get_select_options(selector)` | Returns the options of a `<select>` as text. `.attribute("value")` or `.attribute("index")` reads those of each option instead; `.timeout(..)` sets the wait. |
| `get_property(selector, prop)` | Returns property value. |
| `get_shadow_root(selector)` | Returns the shadow root. |
| `find_element(selector)` | Returns a `WebElement` handle. |
| `find_elements(selector)` | Returns all matching `WebElement` handles. |
| `get_unique_links()` | Returns unique `href` values from the page. |
| `get_all_links()` | Returns all `href` values from the page. |
| `get_link_status_code(url)` | Returns the HTTP status code for `url`. |
