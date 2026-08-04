use seleniumbase_rs::{BaseCase, BrowserConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig::default()).await?;
    sb.open("https://seleniumbase.io/demo_page").await?;

    sb.create_tour("Demo Tour").await?;
    sb.add_tour_step("Welcome to the demo page.", Some("body"))
        .await?;
    sb.add_tour_step("Type something here.", Some("#myInput"))
        .await?;
    sb.add_tour_step("Then click this button.", Some("#myButton"))
        .await?;
    sb.export_tour("demo_tour.html").await?;

    println!("Tour exported to demo_tour.html");
    sb.quit().await?;
    Ok(())
}
