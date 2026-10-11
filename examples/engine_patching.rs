//! Patch the automation markers out of a chromedriver binary, and list the
//! Chromium flags and page script that hide the rest.
//!
//! Chromedriver leaks identifying strings into the pages it drives (the `cdc_`
//! variables, `__webdriver` globals, and so on). `ChromedriverPatcher` rewrites
//! them in the binary so the browser never injects them.
//!
//! The example never touches a real driver. It writes a small file that
//! contains one marker to a temporary folder, patches that, and shows the
//! bytes before and after, the `.orig` backup, and a restore. It then prints
//! the engine-level spoofing flags and the size of the JavaScript bootstrap a
//! fingerprint produces.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example engine_patching
//! ```
//!
//! It needs no browser and no network. To patch a real chromedriver, use
//! `sbase patch-chromedriver --path /path/to/chromedriver`, which keeps the
//! same `.orig` backup.

use std::fs;

use seleniumbase_rs::stealth::evasions::bootstrap_script;
use seleniumbase_rs::{
    engine_spoofing_args, ChromedriverPatcher, EnginePatch, Fingerprint, Result, SeleniumBaseError,
};

fn main() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("chromedriver");
    let marker = "window.cdc_abcdef1234567890abcdef_Array = window.Array;";
    fs::write(&path, marker)?;

    let patcher = ChromedriverPatcher::new(&path);
    println!("needs patch before: {}", patcher.needs_patch()?);

    patcher.patch(EnginePatch::balanced())?;
    let patched = fs::read_to_string(&path)?;
    println!("needs patch after:  {}", patcher.needs_patch()?);
    // The patch overwrites the marker with blanks, so print the text quoted.
    println!("before: {marker:?}");
    println!("after:  {patched:?}");
    if patched.contains("cdc_abcdef") {
        return Err(SeleniumBaseError::AssertionFailed(
            "the cdc_ marker survived patching".to_owned(),
        ));
    }

    // The first patch keeps the original beside the binary, so it can be undone.
    println!(
        "backup {}: {}",
        patcher.backup_path().display(),
        patcher.backup_path().exists()
    );
    patcher.restore()?;
    println!("restored, needs patch again: {}", patcher.needs_patch()?);

    // Flags that stop Chromium announcing itself, whatever driver starts it.
    let args = engine_spoofing_args();
    println!("engine spoofing flags: {}", args.len());
    for arg in args.iter().take(3) {
        println!("  {arg}");
    }

    // The page-side half: a script built from a fingerprint.
    let script = bootstrap_script(&Fingerprint::windows_desktop());
    println!(
        "bootstrap script for a Windows persona: {} bytes",
        script.len()
    );
    Ok(())
}
