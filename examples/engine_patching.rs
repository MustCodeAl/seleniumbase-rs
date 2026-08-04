use std::fs;

use seleniumbase_rs::{engine_spoofing_args, ChromedriverPatcher, EnginePatch, Fingerprint};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use a temporary file so the example does not modify a real driver.
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("chromedriver");
    let marker = b"window.cdc_abcdef1234567890abcdef_Array = window.Array;";
    fs::write(&path, marker)?;

    let patcher = ChromedriverPatcher::new(&path);
    println!("needs patch before: {}", patcher.needs_patch()?);
    patcher.patch(EnginePatch::balanced())?;
    println!("needs patch after:  {}", patcher.needs_patch()?);
    println!("backup created:     {}", patcher.backup_path().exists());

    let args = engine_spoofing_args();
    println!("engine spoofing args count: {}", args.len());
    println!("first arg: {}", args.first().unwrap());

    let fp = Fingerprint::windows_desktop();
    let script = seleniumbase_rs::stealth::evasions::bootstrap_script(&fp);
    println!("bootstrap script length: {} bytes", script.len());

    Ok(())
}
