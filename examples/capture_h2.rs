//! Measures the HTTP/2 connection start and the request headers of the local Chrome and adds them
//! to its file in `src/wire/measured`.
//!
//! The tool serves a small HTTPS site with an ephemeral CA, starts Chrome so that it trusts only
//! that CA, and reads the first request on each of five connections (a navigation, a `fetch`, an
//! image, a script and a stylesheet), once over HTTP/2 and once over HTTP/1.1. Run `capture_tls`
//! and `capture_hints` first: this tool extends their file and refuses a Chrome of another version.
//!
//! ```text
//! cargo run --features impersonate --example capture_h2
//! cargo run --features impersonate --example capture_h2 -- --help
//! ```

use std::path::Path;
use std::process::ExitCode;

use seleniumbase_rs::impersonate::capture_h2::{
    measure_and_write, parse_args, CaptureH2Error, Report, USAGE_PASSES,
};
use seleniumbase_rs::scrape::runtime::capture::args::USAGE;
use seleniumbase_rs::wire::h2_spec::H2Spec;

/// The name and version this tool writes into the provenance of the file.
const TOOL: &str = concat!("capture_h2 ", env!("CARGO_PKG_VERSION"));

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("capture_h2: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), CaptureH2Error> {
    let (args, passes) = parse_args(
        std::env::args().skip(1),
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )?;
    if args.help {
        println!(
            "capture_h2: measure the HTTP/2 start and the request headers of the local Chrome\n\n{USAGE}\n{USAGE_PASSES}"
        );
        return Ok(());
    }
    let report = measure_and_write(&args, passes, TOOL, chrono::Utc::now()).await?;
    print_report(&report);
    Ok(())
}

fn print_report(report: &Report) {
    println!(
        "browser      {} on {:?}",
        report.version.product, report.id.os
    );
    println!(
        "probes       h2: {}, h1: {}",
        report.h2.as_ref().map_or(0, Vec::len),
        report.h1.as_ref().map_or(0, Vec::len)
    );
    match &report.derived {
        Some(derived) => {
            println!("akamai       {}", derived.http2.akamai());
            println!("headers      {:?}", derived.http2.headers_priority);
            println!(
                "lists        document {} headers, fetch {} headers, priority for {:?}",
                derived.headers.document.len(),
                derived.headers.fetch.len(),
                derived
                    .headers
                    .priority_by_dest
                    .iter()
                    .map(|(dest, _)| dest.as_str())
                    .collect::<Vec<_>>()
            );
            print_reproducible(&derived.http2);
            if derived.differences.is_empty() {
                println!("h1 vs h2     the header lists agree");
            }
            for difference in &derived.differences {
                println!("h1 vs h2     {difference}");
            }
        }
        None => println!("specs        not derived: both passes are needed"),
    }
    println!("wrote        {}", report.path.display());
    for path in &report.fixtures {
        println!("wrote        {}", path.display());
    }
}

fn print_reproducible(spec: &H2Spec) {
    match spec.check_reproducible() {
        Ok(()) => println!("reproducible yes"),
        Err(error) => println!("reproducible no: {error}"),
    }
}
