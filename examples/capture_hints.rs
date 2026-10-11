//! Measures the client hints, `User-Agent` and `Accept-Language` of the local Chrome and writes them
//! into `src/wire/measured`.
//!
//! A loopback page asks the browser for its high-entropy hints and reports what
//! `navigator.userAgentData` says, and the server records the `Sec-CH-UA` request headers of the
//! same browser. The two must agree, or nothing is written.
//!
//! For a browser this tool cannot drive, `--snippet` prints a script to paste into its developer
//! console; it prints the same JSON the page reports.
//!
//! ```text
//! cargo run --features scrape --example capture_hints
//! cargo run --features scrape --example capture_hints -- --snippet
//! ```

use std::path::Path;
use std::process::ExitCode;

use seleniumbase_rs::scrape::runtime::capture::args::{ToolArgs, USAGE};
use seleniumbase_rs::scrape::runtime::capture::hints::console_snippet;
use seleniumbase_rs::scrape::runtime::capture::{
    measure_hints, CaptureError, EntryId, MeasuredFile,
};
use seleniumbase_rs::wire::BrowserKind;

/// The name and version this tool writes into the provenance of the file.
const TOOL: &str = concat!("capture_hints ", env!("CARGO_PKG_VERSION"));

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("capture_hints: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), CaptureError> {
    let args = ToolArgs::parse(
        std::env::args().skip(1),
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )?;
    if args.help {
        println!("capture_hints: measure the client hints of the local Chrome\n\n{USAGE}");
        return Ok(());
    }
    if args.snippet {
        println!("{}", console_snippet());
        return Ok(());
    }
    let (version, hints) = measure_hints(&args.chrome).await?;
    let id = EntryId {
        kind: BrowserKind::Chrome,
        major: version.major,
        os: version.os()?,
    };
    let mut file = MeasuredFile::open(&args.out_dir, id)?;
    file.set_identity(id, &version);
    file.set_hints(&hints)?;
    file.stamp(TOOL, &version, chrono::Utc::now());
    file.save()?;

    println!("browser      {} on {:?}", version.product, id.os);
    println!(
        "platform     {} {}",
        hints.client_hints.platform, hints.client_hints.platform_version
    );
    println!(
        "arch         {} {}-bit",
        hints.client_hints.architecture, hints.client_hints.bitness
    );
    println!("brands       {}", hints.client_hints.brands.len());
    println!("languages    {}", hints.accept_language);
    println!("wrote        {}", file.path().display());
    Ok(())
}
