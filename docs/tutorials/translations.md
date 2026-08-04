# Test Translations Guide

SeleniumBase for Rust includes a small translator utility that maps simple
action names into multiple languages. This is useful for localized test reports
or for teams that prefer non-English step names.

## What you will learn

- Which languages are supported.
- How to translate action names.
- How to use translated actions in tests.

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

Translations are stored in the `Translator` module. Add entries to the language
map to support additional languages, then rebuild.

## When to use

- Writing tests for localized products.
- Generating readable reports in different languages.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Translation returns the original word | Language not supported | Use one of the supported language codes. |
| `perform_named_action` fails | Action name not mapped | Check that the translated string exists in the action registry. |
