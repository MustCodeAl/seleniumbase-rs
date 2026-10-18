---
title: "Test Translations Guide"
---

SeleniumBase for Rust includes a small translator utility that maps simple
action names into multiple languages. This is useful for localized test reports
or for teams that prefer non-English step names.

<Info>

**What you will learn**

- Which languages are supported.
- How to translate action names.
- How to use translated actions in tests.

</Info>

<Visibility for="agents">

Summary for AI exports: `Translator::new("zh").translate("click")` returns the action name in another language (English, Chinese, Spanish, German, French and Portuguese are supported), and `sb.translate_action("click", "zh")` with `sb.perform_named_action(&action, "#button")` uses it in a test. A word whose language is not supported comes back unchanged.

</Visibility>

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

<Tip>

Translations are stored in the `Translator` module. Add entries to the language
map to support additional languages, then rebuild.

</Tip>

## When to use

- Writing tests for localized products.
- Generating readable reports in different languages.

## Troubleshooting

<AccordionGroup>

<Accordion title="Translation returns the original word" icon="language">

**Likely cause.** Language not supported.

**Fix.** Use one of the supported language codes.

</Accordion>

<Accordion title="perform_named_action fails" icon="triangle-exclamation">

**Likely cause.** Action name not mapped.

**Fix.** Check that the translated string exists in the action registry.

</Accordion>

</AccordionGroup>
