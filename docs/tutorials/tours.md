# Interactive Tours Guide

Create guided product tours directly from your tests. Tours are rendered as HTML
overlays that walk the user through elements. The exported files are standalone
and can be shared or embedded in test reports.

## What you will learn

- How to create and export a tour.
- Which tour themes are available.
- How to play a tour and common use cases.

## Basic tour

```rust
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

Exported tours are standalone HTML files. Open them in a browser to play the
tour, or serve them from your test report.

## Use cases

- Onboarding flows.
- Interactive documentation.
- Demo and presentation walkthroughs.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Tour element not highlighted | Selector is hidden or inside Shadow DOM | Ensure the element is visible or use a shadow-aware selector. |
| Export file is empty | No steps added | Call `add_tour_step` before `export_tour`. |
| Theme styling missing | Offline environment | Use a theme that bundles CSS or serve the file with internet access. |
