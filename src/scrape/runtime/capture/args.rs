//! The command-line arguments the capture tools share.

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::chrome::{ChromeOptions, DEFAULT_PAGE_PROBES};
use super::CaptureError;

/// Where the measured files go, relative to the crate root.
pub const DEFAULT_OUT_DIR: &str = "src/wire/measured";
/// Where the raw byte fixtures go, relative to the crate root.
pub const DEFAULT_FIXTURES_DIR: &str = "tests/fixtures/measured";
/// The most TLS probe connections `--probes` accepts.
pub const MAX_PROBES: u64 = 64;
/// The longest `--timeout` accepted, in seconds.
pub const MAX_TIMEOUT_SECS: u64 = 600;

/// What every capture tool prints for `--help`.
pub const USAGE: &str = "\
Options:
  --out DIR        where the measured JSON goes (default: src/wire/measured)
  --fixtures DIR   where the raw byte fixtures go (default: tests/fixtures/measured)
  --chrome PATH    the browser binary (default: the system Chrome)
  --lang TAG       the browser's --lang, which sets Accept-Language (default: en-US)
  --probes N       how many ordinary HTTPS connections the TLS probe makes (default: 4)
  --timeout SECS   how long to wait for the browser (default: 30)
  --headless       run the browser headless (its identity is then not measured)
  --no-sandbox     pass --no-sandbox to the browser
  --snippet        print a console snippet for browsers this tool cannot drive, and exit
  --help           print this text";

/// The parsed command line of a capture tool.
#[derive(Debug, Clone)]
pub struct ToolArgs {
    /// Where the measured JSON goes.
    pub out_dir: PathBuf,
    /// Where the raw byte fixtures go.
    pub fixtures_dir: PathBuf,
    /// How to start the browser.
    pub chrome: ChromeOptions,
    /// Whether to print the console snippet instead of measuring.
    pub snippet: bool,
    /// Whether to print the usage instead of measuring.
    pub help: bool,
}

impl ToolArgs {
    /// Parses `args` (without the program name). Relative defaults are resolved against `root`.
    ///
    /// # Errors
    ///
    /// Returns [`CaptureError::Usage`] for an unknown argument, a flag without its value or a
    /// value that is out of range.
    pub fn parse<I>(args: I, root: &Path) -> Result<Self, CaptureError>
    where
        I: IntoIterator<Item = String>,
    {
        let mut parsed = Self {
            out_dir: root.join(DEFAULT_OUT_DIR),
            fixtures_dir: root.join(DEFAULT_FIXTURES_DIR),
            chrome: ChromeOptions::default(),
            snippet: false,
            help: false,
        };
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--out" => parsed.out_dir = PathBuf::from(value(&mut args, &arg)?),
                "--fixtures" => parsed.fixtures_dir = PathBuf::from(value(&mut args, &arg)?),
                "--chrome" => {
                    parsed.chrome.executable = Some(PathBuf::from(value(&mut args, &arg)?));
                }
                "--lang" => parsed.chrome.lang = value(&mut args, &arg)?,
                "--probes" => {
                    let count = number(&value(&mut args, &arg)?, &arg, 1, MAX_PROBES)?;
                    parsed.chrome.page_probes =
                        usize::try_from(count).unwrap_or(DEFAULT_PAGE_PROBES);
                }
                "--timeout" => {
                    let secs = number(&value(&mut args, &arg)?, &arg, 1, MAX_TIMEOUT_SECS)?;
                    parsed.chrome.timeout = Duration::from_secs(secs);
                }
                "--headless" => parsed.chrome.headless = true,
                "--no-sandbox" => parsed.chrome.no_sandbox = true,
                "--snippet" => parsed.snippet = true,
                "--help" | "-h" => parsed.help = true,
                _ => return Err(CaptureError::Usage(format!("unknown argument `{arg}`"))),
            }
        }
        Ok(parsed)
    }
}

/// The value after a flag.
fn value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, CaptureError> {
    args.next()
        .ok_or_else(|| CaptureError::Usage(format!("`{flag}` needs a value")))
}

/// Parses a whole number in `min..=max`.
fn number(text: &str, flag: &str, min: u64, max: u64) -> Result<u64, CaptureError> {
    text.parse::<u64>()
        .ok()
        .filter(|parsed| (min..=max).contains(parsed))
        .ok_or_else(|| {
            CaptureError::Usage(format!("`{flag}` takes a whole number from {min} to {max}"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<ToolArgs, CaptureError> {
        ToolArgs::parse(args.iter().map(|arg| (*arg).to_owned()), Path::new("/repo"))
    }

    #[test]
    fn the_defaults_point_into_the_crate() {
        let args = parse(&[]).expect("no arguments is fine");

        assert_eq!(args.out_dir, Path::new("/repo/src/wire/measured"));
        assert_eq!(
            args.fixtures_dir,
            Path::new("/repo/tests/fixtures/measured")
        );
        assert!(!args.snippet && !args.help && !args.chrome.headless);
    }

    #[test]
    fn every_flag_is_read() {
        let args = parse(&[
            "--out",
            "o",
            "--fixtures",
            "f",
            "--chrome",
            "/bin/chrome",
            "--lang",
            "de-DE",
            "--probes",
            "7",
            "--timeout",
            "12",
            "--headless",
            "--no-sandbox",
            "--snippet",
            "--help",
        ])
        .expect("valid");

        assert_eq!(args.out_dir, Path::new("o"));
        assert_eq!(args.fixtures_dir, Path::new("f"));
        assert_eq!(
            args.chrome.executable.as_deref(),
            Some(Path::new("/bin/chrome"))
        );
        assert_eq!(args.chrome.lang, "de-DE");
        assert_eq!(args.chrome.page_probes, 7);
        assert_eq!(args.chrome.timeout, Duration::from_secs(12));
        assert!(args.chrome.headless && args.chrome.no_sandbox && args.snippet && args.help);
    }

    #[test]
    fn bad_arguments_are_usage_errors() {
        for bad in [
            &["--nope"][..],
            &["--out"],
            &["--probes", "0"],
            &["--probes", "65"],
            &["--probes", "x"],
            &["--timeout", "601"],
            &["--timeout", "-1"],
        ] {
            assert!(matches!(parse(bad), Err(CaptureError::Usage(_))), "{bad:?}");
        }
    }
}
