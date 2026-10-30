//! The contract with the Homebrew tap (`homebrew-tap`, `Formula/sbrs.rb`).
//!
//! The formula builds `crates/cli` from the release tarball, writes the shell completions, the
//! Fig and usage specs and the Warp workflows into the keg, and its `test do` runs the installed
//! `sbrs`. Nothing of that is seen from this repository until a release is published, so this
//! file repeats what the formula relies on, against the real binary:
//!
//! * `test do`: `sbrs --version` names the version of the crate, `sbrs completions usage` lists
//!   `mkfile`, `sbrs plugin list` lists `shield`, `sbrs completions fig` writes a `Fig.Spec`.
//! * `install`: `generate_completions_from_executable(bin/"sbrs", "completions", shells: [:bash,
//!   :zsh, :fish, :pwsh])` runs `sbrs completions SHELL` for each of those four names, and
//!   `Dir[".warp/workflows/*.yaml"]` finds the workflows in the source tree.
//!
//! Homebrew passes `powershell` for `:pwsh`, and so does the release workflow
//! (`.github/workflows/release.yml`); `sbrs completions pwsh` is refused, so nothing may pass it.
//!
//! `scripts/check-consumers.sh homebrew` (`just contracts`) reads the formula in the tap's
//! checkout and fails when it relies on a string that is not in this file. The test of
//! `sbrs completions` for every shell is also in `cli_e2e.rs`; this one is for the names the
//! formula passes.

#[path = "support/sbrs.rs"]
mod sbrs;

use std::path::{Path, PathBuf};

use sbrs::{run, stderr, stdout};

/// The shells of `shells: [:bash, :zsh, :fish, :pwsh]` in the formula, and the argument Homebrew
/// passes for each after `completions`: the name of the shell, except that `:pwsh` is passed as
/// `powershell` (documented in Homebrew's `generate_completions_from_executable`). `sbrs`
/// refuses `pwsh` itself, which is fine only while Homebrew never sends it.
const FORMULA_SHELLS: [(&str, &str); 4] = [
    ("bash", "bash"),
    ("zsh", "zsh"),
    ("fish", "fish"),
    ("pwsh", "powershell"),
];

fn here() -> tempfile::TempDir {
    tempfile::tempdir().expect("a temporary folder")
}

/// The root of the repository: the formula builds from the tarball of it.
fn source_tree() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `assert_match version.to_s, shell_output("#{bin}/sbrs --version")`.
#[test]
fn the_version_names_the_version_of_the_crate() {
    let dir = here();
    let output = run(dir.path(), &[], &["--version"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        stdout(&output).contains(env!("CARGO_PKG_VERSION")),
        "{}",
        stdout(&output)
    );
}

/// `assert_match "mkfile", shell_output("#{bin}/sbrs completions usage")`, and the usage spec the
/// formula installs into `pkgshare/"usage"`.
#[test]
fn the_usage_spec_lists_mkfile() {
    let dir = here();
    let output = run(dir.path(), &[], &["completions", "usage"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("mkfile"), "{}", stdout(&output));
}

/// `assert_match "Fig.Spec", shell_output("#{bin}/sbrs completions fig")`, and the spec the
/// formula installs into `pkgshare/"fig"`.
#[test]
fn the_fig_spec_is_a_fig_spec() {
    let dir = here();
    let output = run(dir.path(), &[], &["completions", "fig"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("Fig.Spec"), "{}", stdout(&output));
}

/// `assert_match "shield", shell_output("#{bin}/sbrs plugin list")`: the formula installs the
/// default features of the command line, which include the built-in plugins.
#[cfg(feature = "plugins-builtin")]
#[test]
fn the_plugin_list_has_shield() {
    let dir = here();
    let output = run(dir.path(), &[], &["plugin", "list"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("shield"), "{}", stdout(&output));
}

/// `generate_completions_from_executable(bin/"sbrs", "completions", shells: [:bash, :zsh, :fish,
/// :pwsh])`: Homebrew passes the name of each shell as the argument after `completions`, and
/// stops the install when `sbrs` refuses one.
#[test]
fn every_shell_the_formula_passes_is_accepted() {
    let dir = here();
    for (symbol, shell) in FORMULA_SHELLS {
        let output = run(dir.path(), &[], &["completions", shell]);
        assert!(
            output.status.success(),
            "`sbrs completions {shell}` (the formula's :{symbol}) was refused: {}",
            stderr(&output)
        );
        assert!(
            stdout(&output).contains("sbrs"),
            "`sbrs completions {shell}` wrote no script"
        );
    }
}

/// `(pkgshare/"warp-workflows").install Dir[".warp/workflows/*.yaml"]`: an empty list installs
/// nothing and says nothing.
#[test]
fn the_warp_workflows_are_in_the_source_tree() {
    let folder = source_tree().join(".warp/workflows");
    let workflows: Vec<_> = std::fs::read_dir(&folder)
        .unwrap_or_else(|error| panic!("{}: {error}", folder.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "yaml")
        })
        .collect();
    assert!(
        !workflows.is_empty(),
        "no .warp/workflows/*.yaml in the source tree"
    );
}
