---
title: "Interactive Tours Guide"
sidebarTitle: "Tours"
---

Create guided product tours directly from your tests. Tours are rendered as HTML
overlays that walk the user through elements. The exported files are standalone
and can be shared or embedded in test reports.

<Info>

**What you will learn**

- How to create and export a tour.
- Which tour themes are available.
- How to play a tour and common use cases.

</Info>

## Basic tour

```rust src/main.rs
use seleniumbase_rs::{BaseCase, BrowserConfig, TourTheme};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig::default()).await?;
    sb.open("https://seleniumbase.io/demo_page").await?;

    sb.create_tour_with_theme("demo_tour", TourTheme::IntroJs).await?;
    sb.add_tour_step("Welcome to the demo page.", Some("body")).await?;
    sb.add_tour_step("Type something here.", Some("#myInput")).await?;
    sb.add_tour_step("Then click this button.", Some("#myButton")).await?;
    sb.export_tour("demo_tour.html").await?;

    sb.quit().await?;
    Ok(())
}
```

## Tour themes

The crate supports several JavaScript tour libraries:

- `TourTheme::Shepherd`
- `TourTheme::IntroJs`
- `TourTheme::DriverJs`
- `TourTheme::Bootstrap`
- `TourTheme::Hopscotch`

Use `create_tour_with_theme(name, theme)` to set the theme explicitly, or use a
convenience constructor such as `create_introjs_tour(name)`.

## Play a tour

<Info>

Exported tours are standalone HTML files. Open them in a browser to play the
tour, or serve them from your test report.

</Info>

## Use cases

- Onboarding flows.
- Interactive documentation.
- Demo and presentation walkthroughs.

## Troubleshooting

<AccordionGroup>

<Accordion title="Tour element not highlighted" icon="highlighter">

**Likely cause.** Selector is hidden or inside Shadow DOM.

**Fix.** Ensure the element is visible or use a shadow-aware selector.

</Accordion>

<Accordion title="Export file is empty" icon="file">

**Likely cause.** No steps added.

**Fix.** Call `add_tour_step` before `export_tour`.

</Accordion>

<Accordion title="Theme styling missing" icon="palette">

**Likely cause.** Offline environment.

**Fix.** Use a theme that bundles CSS or serve the file with internet access.

</Accordion>

</AccordionGroup>
