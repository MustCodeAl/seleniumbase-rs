---
description: "Strict Rust API guidelines, parity mapping rules, and prohibition of get_ getter prefixes"
globs: ["**/*.rs", "parity/*", "docs/*"]
always_on: true
---

# Strict Rust API Guidelines & Python Parity Rules

1. **No `get_` prefixes on getters (Rule R3 / C-GETTER / Decision D2):**
   - In accordance with Rust API Guidelines C-GETTER and Decision D2, getters on `BaseCase`, `Element`, `Page`, or any other type MUST drop `get_`.
   - Examples: use `text(selector)`, `title()`, `current_url()`, `page_source()`, `attribute(selector, attr)`, `property(selector, prop)`, `cookies()`, `cookie(name)`, `element(selector)`, `value(selector)`, `user_agent()`, `window_size()`.
   - NEVER add `get_text`, `get_title`, `get_current_url`, `get_cookies`, or any redundant `get_*` alias methods to `BaseCase` or the CDP engine.

2. **Always consult parity sources first:**
   - Before adding any method or thinking a Python feature is missing, check `docs/parity.md` (generated from `parity/api.toml`) or https://mustcodeal-seleniumbase-rs.docs7.io/parity and `docs/design/api_guidelines_audit.md`.
   - Python `get_*` methods are already tracked and mapped to their idiomatic Rust equivalents.
   - Use the `parity_lookup` or `find_api` tools in the `dev` MCP server.

3. **Standard conversions, constructors, and traits:**
   - Conversions follow C-CONV: `as_` for cheap borrow, `to_` for costly conversion, `into_` for consuming conversions.
   - Value types must implement standard traits: `FromStr`, `Display`, `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash` where applicable (C-COMMON-TRAITS, C-CONV-TRAITS).
   - Constructors follow C-CTOR: `new()`, or `try_new()` when fallible.
