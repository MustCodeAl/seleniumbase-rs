//! `sbrs pdf-text` and `sbrs assert-pdf-text`, run as the real binary.
//!
//! Reading the text of a PDF needs the `pdf` feature. A build without it must
//! say so and exit with status 2 before it starts a browser; the commands stay
//! in `--help` and in the shell completions so that the command line does not
//! change with the features. The status and hint tests run only when `pdf` is
//! off, which is the default build.

use std::path::Path;
use std::process::{Command, Output};

const SBASE: &str = env!("CARGO_BIN_EXE_sbrs");

/// Runs `sbrs` in `dir`, an empty folder, so no `sbase_config.toml` of the
/// host is picked up.
fn sbrs(dir: &Path, args: &[&str]) -> Output {
    Command::new(SBASE)
        .args(args)
        .current_dir(dir)
        .env_remove("NO_COLOR")
        .env_remove("RUST_LOG")
        .env("SB_LOG_LEVEL", "error")
        .output()
        .expect("sbrs runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn the_help_lists_both_pdf_text_commands_whatever_the_features() {
    let dir = tempfile::tempdir().expect("temp dir");

    let list = sbrs(dir.path(), &["--help"]);

    assert!(list.status.success(), "{}", stderr(&list));
    assert!(stdout(&list).contains("pdf-text"), "{}", stdout(&list));
    assert!(
        stdout(&list).contains("assert-pdf-text"),
        "{}",
        stdout(&list)
    );
}

#[cfg(not(feature = "pdf"))]
#[test]
fn without_the_pdf_feature_both_commands_exit_with_status_2_and_a_hint() {
    let dir = tempfile::tempdir().expect("temp dir");

    for args in [
        vec!["pdf-text", "report.pdf"],
        vec!["assert-pdf-text", "report.pdf", "--text", "Quarterly"],
    ] {
        let output = sbrs(dir.path(), &args);

        assert_eq!(
            output.status.code(),
            Some(2),
            "{args:?}: {}",
            stderr(&output)
        );
        let hint = stderr(&output);
        assert!(
            hint.contains("built without PDF text extraction"),
            "{args:?}: {hint}"
        );
        assert!(
            hint.contains(
                "cargo install --git https://github.com/MustCodeAl/seleniumbase-rs \
                 seleniumbase-rs-cli --features pdf"
            ),
            "the hint says how to get the feature, from the package that builds sbrs: {hint}"
        );
        assert!(stdout(&output).is_empty(), "{args:?}: {}", stdout(&output));
    }
}
