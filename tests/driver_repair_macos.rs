//! `launch_chromedriver` starts a driver that macOS would kill.
//!
//! Needs a chromedriver on `PATH`, so it is ignored by default:
//!
//! ```text
//! cargo test --test driver_repair_macos -- --ignored
//! ```
//!
//! It copies the installed driver into a temporary folder's
//! `downloaded_drivers/`, where the launcher looks first, changes one byte the
//! signature covers so the system kills it, and checks that the launcher signs
//! it again and starts it. It runs from that temporary folder, so it never
//! touches a `downloaded_drivers/` of yours.

#![cfg(target_os = "macos")]

use std::fs;
use std::process::Command;

use seleniumbase_rs::browser::driver_access::killed_by_the_system;
use seleniumbase_rs::browser::launcher::launch_chromedriver;

#[tokio::test]
#[ignore = "needs chromedriver on PATH"]
async fn a_driver_macos_kills_is_signed_again_and_started() {
    let installed = which::which("chromedriver")
        .expect("this test needs chromedriver on PATH (it is ignored by default)");

    // The launcher looks in ./downloaded_drivers. This is the only test in this
    // file, so changing the working directory affects nothing else.
    let work = tempfile::tempdir().unwrap();
    std::env::set_current_dir(work.path()).unwrap();
    let folder = work.path().join("downloaded_drivers");
    fs::create_dir_all(&folder).unwrap();
    let staged = folder.join("chromedriver");

    fs::copy(&installed, &staged).unwrap();
    let mut bytes = fs::read(&staged).unwrap();
    assert_eq!(
        bytes[..4],
        [0xcf, 0xfa, 0xed, 0xfe],
        "expected a thin 64-bit Mach-O chromedriver"
    );
    // The `reserved` field of the header is covered by the signature but read
    // by nothing.
    bytes[28] ^= 0xff;
    fs::write(&staged, bytes).unwrap();

    let direct = Command::new(&staged).arg("--version").output().unwrap();
    assert!(
        killed_by_the_system(direct.status),
        "the altered driver was not killed ({:?}), so there is nothing to repair",
        direct.status
    );

    let mut driver = launch_chromedriver()
        .await
        .expect("the launcher repairs the driver and starts it");
    assert!(
        driver.url.starts_with("http://127.0.0.1:"),
        "{}",
        driver.url
    );
    driver.kill();

    // The file was repaired in place: it now runs by itself.
    let after = Command::new(&staged).arg("--version").output().unwrap();
    assert!(after.status.success(), "{:?}", after.status);
}
