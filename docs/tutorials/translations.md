# Test Translations Guide

SeleniumBase for Rust includes a small translator utility that maps simple action names into multiple languages.

## Supported languages

- English
- Chinese (中文)
- Spanish (Español)
- German (Deutsch)
- French (Français)
- Portuguese (Português)

## Translate an action

```rust
use seleniumbase_rs::utils::translate::Translator;

let t = Translator::new("zh");
assert_eq!(t.translate("click"), "点击");
```

## Use in tests

```rust
let action = sb.translate_action("click", "zh");
sb.perform_named_action(&action, "#button").await?;
```

## Add a new language

Translations are stored in the `Translator` module. Add entries to the language map to support additional languages.

## When to use

- Writing tests for localized products.
- Generating readable reports in different languages.
