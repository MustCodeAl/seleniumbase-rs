//! Example: uploading test artifacts to S3 (`--features s3`).
//! Azure and GCP uploaders moved to `seleniumbase-rs-plugins` in v0.10.0.

use seleniumbase_rs::tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Run with `--features s3` to upload artifacts to S3.");
    println!("Azure and GCP uploaders moved to seleniumbase-rs-plugins in v0.10.0.");
    Ok(())
}
