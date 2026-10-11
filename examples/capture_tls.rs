//! Measures the TLS `ClientHello` of the local Chrome and writes it into `src/wire/measured`.
//!
//! The browser connects to a loopback listener that records each hello and hangs up, so nothing
//! is typed by hand: the profile, the JA3, JA4 and peetprint strings and the raw bytes all come
//! from what the browser sent.
//!
//! ```text
//! cargo run --features scrape --example capture_tls
//! cargo run --features scrape --example capture_tls -- --help
//! ```

use std::path::Path;
use std::process::ExitCode;

use seleniumbase_rs::scrape::runtime::capture::args::{ToolArgs, USAGE};
use seleniumbase_rs::scrape::runtime::capture::record::write_fixtures;
use seleniumbase_rs::scrape::runtime::capture::{measure_tls, CaptureError, EntryId, MeasuredFile};
use seleniumbase_rs::wire::BrowserKind;

/// The name and version this tool writes into the provenance of the file.
const TOOL: &str = concat!("capture_tls ", env!("CARGO_PKG_VERSION"));

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("capture_tls: {error}");
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
        println!("capture_tls: measure the TLS ClientHello of the local Chrome\n\n{USAGE}");
        return Ok(());
    }
    let (version, tls) = measure_tls(&args.chrome).await?;
    let id = EntryId {
        kind: BrowserKind::Chrome,
        major: version.major,
        os: version.os()?,
    };
    let mut file = MeasuredFile::open(&args.out_dir, id)?;
    file.set_identity(id, &version);
    file.set_tls(&tls)?;
    file.stamp(TOOL, &version, chrono::Utc::now());
    file.save()?;
    let fixtures = write_fixtures(&args.fixtures_dir, id, &tls)?;

    println!("browser      {} on {:?}", version.product, id.os);
    println!(
        "hellos       {} ordinary, websocket: {}",
        tls.samples.len(),
        tls.websocket_sample.is_some()
    );
    println!("shuffles     {}", tls.spec.permute_extensions);
    if let Some(sample) = tls.samples.first() {
        println!("ja4          {}", sample.ja4);
    }
    println!("known gaps   {:?}", tls.known_gaps);
    match tls
        .spec
        .without_known_gaps(&tls.known_gaps)
        .check_reproducible()
    {
        Ok(()) => println!("reproducible yes, apart from the known gaps"),
        Err(error) => println!("reproducible no: {error}"),
    }
    println!("wrote        {}", file.path().display());
    for path in fixtures {
        println!("wrote        {}", path.display());
    }
    Ok(())
}
