# Interactive Tours Guide

Create guided product tours directly from your tests. Tours are rendered as HTML overlays that walk the user through elements.

## Basic tour

```rust
use seleniumbase_rs::{BaseCase, BrowserConfig, TourTheme};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig::default()).await?;
    sb.open("https://seleniumbase.io/demo_page").await?;

    sb.create_tour(theme=TourTheme::IntroJs)?;
    sb.add_tour_step("Welcome", "body", Some("Welcome to the demo page."))?;
    sb.add_tour_step("Input", "#myInput", Some("Type something here."))?;
    sb.add_tour_step("Button", "#myButton", Some("Then click this button."))?;
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

Pick the theme that matches your application's style.

## Play a tour

Exported tours are standalone HTML files. Open them in a browser to play the tour, or serve them from your test report.

## Use cases

- Onboarding flows.
- Interactive documentation.
- Demo and presentation walkthroughs.
