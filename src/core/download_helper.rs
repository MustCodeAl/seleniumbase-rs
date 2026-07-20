use std::fs;
use std::io::Write;
use std::path::Path;

/// Download a file from `url` and save it to `dest`.
pub async fn download_file<P: AsRef<Path>>(
    client: &reqwest::Client,
    url: &str,
    dest: P,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let response = client.get(url).send().await?;
    let bytes = response.bytes().await?;
    let mut file = fs::File::create(dest)?;
    file.write_all(&bytes)?;
    Ok(())
}

/// Download a file synchronously using a blocking client.
pub fn download_file_blocking<P: AsRef<Path>>(
    url: &str,
    dest: P,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;
    let response = client.get(url).send()?;
    let bytes = response.bytes()?;
    let mut file = fs::File::create(dest)?;
    file.write_all(&bytes)?;
    Ok(())
}
