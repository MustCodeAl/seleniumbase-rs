//! Whether the machine that runs a real-Chrome test is one the project measured.
//!
//! The measured TLS, HTTP/2 and header tables hold one entry, Chrome 155 on macOS, and there is
//! no fallback to another system's values: a Chrome 155 handshake under a Linux user agent is a
//! contradiction a detector can see. A test that compares a real Chrome with that entry, or builds
//! a session from it, cannot run on any other system. [`measured_or_skip`] says so, and the test
//! returns instead of failing for a persona that does not exist.
//!
//! A macOS machine whose Chrome has another major version is not skipped. That mismatch is the
//! alarm that the capture tools must be run again, and the test has to fail on it.
//!
//! Rust has no run-time skip, so a skipped test passes. Its note goes to standard error, which the
//! test harness shows for a passing test only with `-- --nocapture`.

use seleniumbase_rs::stealth::fingerprint::OsType;
use seleniumbase_rs::wire::emulation;
use seleniumbase_rs::wire::BrowserKind;

/// The operating system this machine runs, as the persona of a real browser on it.
fn host_os() -> OsType {
    if cfg!(target_os = "macos") {
        OsType::Macos
    } else if cfg!(target_os = "windows") {
        OsType::Windows
    } else {
        OsType::Linux
    }
}

/// Whether `test` can run on this machine. When it cannot, prints why and returns `false`; the
/// test then returns.
pub fn measured_or_skip(test: &str) -> bool {
    match emulation::get(BrowserKind::Chrome, host_os()) {
        Ok(_) => true,
        Err(error) => {
            eprintln!(
                "skipped {test}: {error}.\n\
                 It compares a real Chrome with the measured one, and only Chrome 155 on macOS is \
                 measured today. See \"Re-measuring when Chrome updates\" in \
                 docs/tutorials/stealth_scraping.md."
            );
            false
        }
    }
}
