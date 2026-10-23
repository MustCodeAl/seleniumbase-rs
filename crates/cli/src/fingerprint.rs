//! `sbrs fingerprint`: capture the fingerprint of a real browser, write a preset or a random
//! identity, check a file and compare two.
//!
//! This file declares the arguments and starts the browser for `capture`; what the other
//! commands print is built by `seleniumbase_rs::cli::fingerprint_cmd`, which has no parser and
//! is tested without a binary. Every file read or written here is the one file format, the v2
//! `Fingerprint` JSON.

use std::error::Error;
use std::io::Write;
use std::path::PathBuf;

use seleniumbase_rs::browser::config::BrowserConfig;
use seleniumbase_rs::cli::fingerprint_cmd::{self as text, ExportSource};
use seleniumbase_rs::{BaseCase, OsType};
use serde_json::json;

// The `fingerprint` command: it holds only its own subcommands. (Plain comments, not docs: a
// doc comment on this struct would be help text.)
#[derive(Debug, usage::Args)]
pub struct FingerprintArgs {
    #[usage(subcommand)]
    pub command: FingerprintCommand,
}

#[derive(Debug, usage::Subcommands)]
pub enum FingerprintCommand {
    /// Read the fingerprint of a real browser and write it as a fingerprint file.
    ///
    /// Starts a browser as `sbrs open` does (the global --browser, --headless, --proxy and the
    /// rest apply), opens URL (a blank page when there is none), reads what a page can see
    /// of the browser, and prints the fingerprint as JSON, or writes it to FILE. A summary
    /// goes to standard error. What no setting holds (window sizes, the automation markers,
    /// the canvas hash) stays out of the fingerprint; --readings writes it to its own file.
    Capture(CaptureArgs),
    /// Write a preset or a random identity as a fingerprint file.
    ///
    /// Give --preset NAME (windows, edge, macos, linux, android, ios) or --random. A random
    /// identity follows --seed, so the same seed gives the same file, and --os, the system it
    /// claims (default: this machine's). The file goes to FILE, else to standard output.
    Export(ExportArgs),
    /// Check that a fingerprint file loads and is coherent.
    ///
    /// Prints how many settings the file gives, each error and warning, and the verdict. Exits
    /// with status 2 when there is an error or the file does not load.
    Check(CheckArgs),
    /// Compare two fingerprint files.
    ///
    /// EXPECTED is what is expected and FOUND what is found: a setting that EXPECTED leaves
    /// natural is never compared. Prints each setting that differs. Exits with status 1 when
    /// any differs, 0 when none does, and 2 when a file does not load.
    ///
    /// To check that a browser presents a file: `sbrs --fingerprint f.json fingerprint
    /// capture -o seen.json`, then `sbrs fingerprint diff f.json seen.json`.
    Diff(DiffArgs),
    /// List the presets and what each claims.
    Presets,
}

// The options of `fingerprint capture`. (Plain comments, not docs: a doc comment on this
// struct would be help text.)
#[derive(Debug, usage::Args)]
#[usage(args_override_self = false)]
pub struct CaptureArgs {
    /// The page to open first (default: a blank page).
    #[usage(value_name = "URL")]
    url: Option<String>,
    /// Write the fingerprint to FILE instead of printing it.
    #[usage(short = 'o', long, value_name = "FILE", value_hint = usage::ValueHint::AnyPath)]
    output: Option<PathBuf>,
    /// Also write the readings that no setting holds, and where the capture was taken, to FILE.
    #[usage(long, value_name = "FILE", value_hint = usage::ValueHint::AnyPath)]
    readings: Option<PathBuf>,
}

/// The systems a random identity can claim.
#[derive(Clone, Copy, Debug, usage::ValueEnum)]
enum OsArg {
    Windows,
    Macos,
    Linux,
    Android,
    Ios,
}

impl From<OsArg> for OsType {
    fn from(value: OsArg) -> Self {
        match value {
            OsArg::Windows => Self::Windows,
            OsArg::Macos => Self::Macos,
            OsArg::Linux => Self::Linux,
            OsArg::Android => Self::Android,
            OsArg::Ios => Self::Ios,
        }
    }
}

// The options of `fingerprint export`. (Plain comments, not docs: a doc comment on this
// struct would be help text.)
#[derive(Debug, usage::Args)]
#[usage(args_override_self = false)]
pub struct ExportArgs {
    /// The preset to write: windows, edge, macos, linux, android or ios (safari is ios).
    #[usage(long, value_name = "NAME")]
    preset: Option<String>,
    /// Write a random identity instead of a preset.
    #[usage(long, bool_value)]
    random: bool,
    /// With --random, the seed of the draw: the same seed gives the same identity.
    #[usage(long, value_name = "N")]
    seed: Option<u64>,
    /// With --random, the system the identity claims (default: this machine's).
    #[usage(long, value_enum, value_name = "OS")]
    os: Option<OsArg>,
    /// Write the fingerprint to FILE instead of printing it.
    #[usage(short = 'o', long, value_name = "FILE", value_hint = usage::ValueHint::AnyPath)]
    output: Option<PathBuf>,
}

// The argument of `fingerprint check`. (Plain comments, not docs: a doc comment on this
// struct would be help text.)
#[derive(Debug, usage::Args)]
pub struct CheckArgs {
    /// The fingerprint file to check.
    #[usage(value_name = "FILE", value_hint = usage::ValueHint::AnyPath)]
    file: PathBuf,
}

// The arguments of `fingerprint diff`. (Plain comments, not docs: a doc comment on this
// struct would be help text.)
#[derive(Debug, usage::Args)]
pub struct DiffArgs {
    /// The file with what is expected.
    #[usage(value_name = "EXPECTED", value_hint = usage::ValueHint::AnyPath)]
    expected: PathBuf,
    /// The file with what is found.
    #[usage(value_name = "FOUND", value_hint = usage::ValueHint::AnyPath)]
    found: PathBuf,
}

/// Runs the subcommands that start no browser and returns the exit status; `None` for
/// `capture`, which does.
///
/// # Errors
///
/// A file that cannot be written. A file that cannot be read, and a wrong combination of
/// options, are answered with status 2 and a message instead.
pub fn run_offline(command: &FingerprintCommand) -> Result<Option<i32>, Box<dyn Error>> {
    let status = match command {
        FingerprintCommand::Capture(_) => return Ok(None),
        FingerprintCommand::Presets => {
            print!("{}", text::presets_table());
            0
        }
        FingerprintCommand::Export(args) => export(args)?,
        FingerprintCommand::Check(args) => match text::check(&args.file) {
            Ok(check) => {
                print!("{}", check.text);
                if check.is_coherent {
                    0
                } else {
                    2
                }
            }
            Err(error) => failed(&error),
        },
        FingerprintCommand::Diff(args) => match text::diff(&args.expected, &args.found) {
            Ok(difference) => {
                print!("{}", difference.text);
                i32::from(difference.differs)
            }
            Err(error) => failed(&error),
        },
    };
    std::io::stdout().flush()?;
    Ok(Some(status))
}

/// Says why a command could not run and gives the status for it.
fn failed(error: &dyn std::fmt::Display) -> i32 {
    eprintln!("sbrs: {error}");
    2
}

/// `fingerprint export`: the identity, to the file or to standard output.
fn export(args: &ExportArgs) -> Result<i32, Box<dyn Error>> {
    let source = match (&args.preset, args.random) {
        (Some(_), true) => return Ok(misuse("give --preset or --random, not both")),
        (None, false) => return Ok(misuse("give --preset NAME or --random")),
        (Some(name), false) if args.seed.is_some() || args.os.is_some() => {
            let _ = name;
            return Ok(misuse("--seed and --os apply to --random only"));
        }
        (Some(name), false) => ExportSource::Preset(name.clone()),
        (None, true) => ExportSource::Random {
            os: args.os.map(OsType::from),
            seed: args.seed,
        },
    };
    let fingerprint = match text::export(&source) {
        Ok(fingerprint) => fingerprint,
        Err(error) => return Ok(failed(&error)),
    };
    match &args.output {
        Some(path) => {
            fingerprint.save(path)?;
            eprintln!("Wrote {}", path.display());
        }
        None => println!("{}", fingerprint.to_json_pretty()?),
    }
    Ok(0)
}

/// A wrong combination of options, said the way the parser says its own mistakes.
fn misuse(message: &str) -> i32 {
    eprintln!("error: {message}");
    2
}

/// `fingerprint capture`: reads the fingerprint of the browser `config` starts.
///
/// The browser is closed whether or not the capture worked.
///
/// # Errors
///
/// The browser does not start, the page does not open, the capture fails, or a file cannot
/// be written.
pub async fn capture(args: CaptureArgs, config: BrowserConfig) -> Result<(), Box<dyn Error>> {
    let mut sb = BaseCase::new(config).await?;
    let taken = async {
        if let Some(url) = args.url.as_deref() {
            sb.open(url).await?;
        }
        sb.capture_fingerprint().await
    }
    .await;
    sb.quit().await?;
    let captured = taken?;

    eprint!("{}", text::capture_summary(&captured));
    match &args.output {
        Some(path) => {
            captured.fingerprint.save(path)?;
            eprintln!("Wrote {}", path.display());
        }
        None => println!("{}", captured.fingerprint.to_json_pretty()?),
    }
    if let Some(path) = &args.readings {
        let readings = json!({ "probe": captured.probe, "unmapped": captured.unmapped });
        std::fs::write(path, serde_json::to_string_pretty(&readings)?)?;
        eprintln!("Wrote {}", path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn export_args(preset: Option<&str>, random: bool) -> ExportArgs {
        ExportArgs {
            preset: preset.map(str::to_owned),
            random,
            seed: None,
            os: None,
            output: None,
        }
    }

    #[test]
    fn export_wants_exactly_one_source() {
        assert_eq!(export(&export_args(None, false)).unwrap(), 2);
        assert_eq!(export(&export_args(Some("windows"), true)).unwrap(), 2);
    }

    #[test]
    fn seed_and_os_belong_to_random() {
        let mut args = export_args(Some("windows"), false);
        args.seed = Some(7);
        assert_eq!(export(&args).unwrap(), 2);
        let mut args = export_args(Some("windows"), false);
        args.os = Some(OsArg::Macos);
        assert_eq!(export(&args).unwrap(), 2);
    }

    #[test]
    fn an_unknown_preset_is_status_2_not_a_panic() {
        assert_eq!(export(&export_args(Some("beos"), false)).unwrap(), 2);
    }

    #[test]
    fn every_system_maps_to_its_own() {
        assert_eq!(OsType::from(OsArg::Windows), OsType::Windows);
        assert_eq!(OsType::from(OsArg::Macos), OsType::Macos);
        assert_eq!(OsType::from(OsArg::Linux), OsType::Linux);
        assert_eq!(OsType::from(OsArg::Android), OsType::Android);
        assert_eq!(OsType::from(OsArg::Ios), OsType::Ios);
    }
}
