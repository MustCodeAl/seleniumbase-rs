//! `sbrs mkdir`: create a folder holding a runnable browser-test suite.
//!
//! The suite is one Cargo test target: a `main.rs` that declares the example
//! tests, which use the rstest `sb` and `sb_cdp` fixtures of
//! [`testing`](crate::testing). Options add a Gherkin scenario that runs on the
//! built-in steps ([`Suite::bdd`]) and a GitHub Actions workflow
//! ([`Suite::gha`]). The golden files under `tests/golden/cli/` pin every
//! generated file, and the `cfg(doctest)` items at the bottom of this file
//! compile the Rust modules against the current API on every
//! `cargo test --features testing` (and `bdd`).

use std::path::PathBuf;

use crate::cli::scaffold::{GeneratedFile, RelativeName, Scaffold, ScaffoldError};

/// Where GitHub looks for workflows, below the folder `sbrs mkdir` runs in.
const WORKFLOW_PATH: &str = ".github/workflows/sbrs-tests.yml";

const MY_FIRST_TEST: &str = "\
//! The first example test. Copy it to start your own.

use rstest::rstest;
use seleniumbase_rs::testing::{sb, Sb};
use seleniumbase_rs::Result;

#[rstest]
#[tokio::test]
async fn my_first_test(sb: Sb) -> Result<()> {
    sb.run(async |sb| {
        sb.open(\"https://example.com\").await?;
        sb.assert_title(\"Example Domain\").await?;
        sb.assert_text(\"Example Domain\", \"h1\").await
    })
    .await
}
";

const PARAMETERIZED_TEST: &str = "\
//! A parameterized test: each `#[case]` is a test of its own, with its own
//! browser and its own folder of evidence when it fails.

use rstest::rstest;
use seleniumbase_rs::testing::{sb, Sb};
use seleniumbase_rs::Result;

#[rstest]
#[case(\"https://example.com\", \"Example Domain\")]
#[case(\"https://example.org\", \"Example Domain\")]
#[tokio::test]
async fn every_page_has_its_heading(
    sb: Sb,
    #[case] page: &str,
    #[case] heading: &str,
) -> Result<()> {
    sb.run(async |sb| {
        sb.open(page).await?;
        sb.assert_text(heading, \"h1\").await
    })
    .await
}
";

const CDP_TEST: &str = "\
//! A pure CDP test: Chrome is driven over the DevTools Protocol, so no
//! chromedriver is needed. `page` is the first tab.

use rstest::rstest;
use seleniumbase_rs::testing::{sb_cdp, SbCdp};
use seleniumbase_rs::Result;

#[rstest]
#[tokio::test]
async fn heading_over_cdp(sb_cdp: SbCdp) -> Result<()> {
    sb_cdp
        .run(async |page| {
            page.goto(\"https://example.com\").await?;
            page.locator(\"h1\")
                .expect()
                .to_contain_text(\"Example Domain\")
                .await
        })
        .await
}
";

/// The Gherkin scenario test; `<SCENARIO>` is the `#[scenario(..)]` attribute.
const BDD_TEST: &str = "\
//! A Gherkin scenario: `features/example.feature` says what happens, and the
//! steps are the library's built-in ones. Needs the feature `bdd` of
//! `seleniumbase-rs`.

use rstest_bdd_macros::scenario;
use seleniumbase_rs::testing::bdd::{sb, SbCase};

<SCENARIO>
#[tokio::test(flavor = \"multi_thread\")]
async fn open_a_page(#[future(awt)] sb: SbCase) {
    let _ = &sb;
}
";

const FEATURE: &str = "\
Feature: Example Domain

  Scenario: Open a page
    Given Open \"https://example.com\"
    Then Assert title \"Example Domain\"
    And Assert text \"Example Domain\" in \"h1\"
";

/// The workflow; `<TARGET>` is the suite's test target.
const WORKFLOW: &str = "\
name: Browser tests

on:
  push:
  pull_request:
  workflow_dispatch:

jobs:
  browser-tests:
    runs-on: ubuntu-latest
    env:
      SB_HEADLESS: \"true\"
    steps:
      - uses: actions/checkout@v6
      - uses: dtolnay/rust-toolchain@stable
      - uses: browser-actions/setup-chrome@v2
        with:
          install-chromedriver: true
      - name: Run the browser tests
        run: cargo test --test <TARGET>
      - name: Upload the evidence of failed tests
        if: ${{ always() }}
        uses: actions/upload-artifact@v6
        with:
          name: sbrs-latest-logs
          path: ./latest_logs/
          if-no-files-found: ignore
";

/// A browser-test suite folder to generate.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::cli::scripts::sb_mkdir::Suite;
///
/// # fn main() -> Result<(), seleniumbase_rs::cli::scaffold::ScaffoldError> {
/// let suite = Suite::new("tests/ui")?;
/// let names: Vec<String> = suite.files().iter().map(|(name, _)| name.to_string()).collect();
/// assert_eq!(names[0], "tests/ui/README.md");
/// assert!(names.contains(&"tests/ui/main.rs".to_owned()));
/// assert!(names.contains(&"tests/ui/cdp_test.rs".to_owned()));
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Suite {
    name: RelativeName,
    basic: bool,
    gha: bool,
    bdd: bool,
}

impl Suite {
    /// A suite in the folder `name`.
    ///
    /// # Errors
    ///
    /// [`ScaffoldError::InvalidName`] when `name` is not a relative name made of
    /// letters, digits, `_`, `-` and `.`.
    pub fn new(name: &str) -> Result<Self, ScaffoldError> {
        Ok(Self {
            name: RelativeName::new(name)?,
            basic: false,
            gha: false,
            bdd: false,
        })
    }

    /// Generates only the scaffolding (`main.rs` and `README.md`) and no
    /// example tests.
    #[must_use]
    pub fn basic(mut self, basic: bool) -> Self {
        self.basic = basic;
        self
    }

    /// Adds a GitHub Actions workflow, `.github/workflows/sbrs-tests.yml`, to
    /// the folder `sbrs mkdir` runs in (which is the repository's root when
    /// GitHub is to read it), not to the suite's folder.
    #[must_use]
    pub fn gha(mut self, gha: bool) -> Self {
        self.gha = gha;
        self
    }

    /// Adds a Gherkin scenario, `features/example.feature` and `bdd_test.rs`,
    /// that runs on the library's built-in steps (feature `bdd`). It is added
    /// whether or not the suite is [`basic`](Self::basic).
    #[must_use]
    pub fn bdd(mut self, bdd: bool) -> Self {
        self.bdd = bdd;
        self
    }

    /// The folder the suite is created in.
    #[must_use]
    pub fn name(&self) -> &RelativeName {
        &self.name
    }

    /// The name Cargo gives the suite's test target: the folder's last component.
    #[must_use]
    pub fn target(&self) -> &str {
        self.name.file_name()
    }

    /// The attribute that binds `bdd_test.rs` to the feature file, whose path is
    /// relative to the crate's root, written as rustfmt would.
    fn scenario_attribute(&self) -> String {
        let path = format!("{}/features/example.feature", self.name);
        let line = format!("#[scenario(path = {path:?}, name = \"Open a page\")]");
        // rustfmt keeps the arguments of an attribute on one line up to 70 columns.
        if line.chars().count() <= "#[scenario()]".len() + 70 {
            line
        } else {
            format!("#[scenario(\n    path = {path:?},\n    name = \"Open a page\"\n)]")
        }
    }

    /// The example tests, by module name and source, in the order of the README.
    fn examples(&self) -> Vec<(&'static str, String)> {
        let mut examples = Vec::new();
        if !self.basic {
            examples.push(("my_first_test", MY_FIRST_TEST.to_owned()));
            examples.push(("parameterized_test", PARAMETERIZED_TEST.to_owned()));
            examples.push(("cdp_test", CDP_TEST.to_owned()));
        }
        if self.bdd {
            examples.push((
                "bdd_test",
                BDD_TEST.replace("<SCENARIO>", &self.scenario_attribute()),
            ));
        }
        examples
    }

    /// The `main.rs` of the test target.
    fn main_rs(&self) -> String {
        let mut modules: Vec<&str> = self.examples().iter().map(|(module, _)| *module).collect();
        modules.sort_unstable();
        let mut main = String::from(
            "//! Browser tests for this folder. See README.md for how Cargo finds them.\n",
        );
        if modules.is_empty() {
            main.push_str(
                "//!\n\
                 //! Add a test by creating `my_test.rs` here with a `#[rstest]` function that\n\
                 //! asks for the `sb` fixture, then declaring it below with `mod my_test;`.\n",
            );
        } else {
            main.push('\n');
            for module in modules {
                main.push_str(&format!("mod {module};\n"));
            }
        }
        main
    }

    /// Every file of the suite, in the order they are created.
    ///
    /// The workflow of [`gha`](Self::gha) is the last, and is named from the
    /// folder `sbrs mkdir` runs in.
    #[must_use]
    pub fn files(&self) -> Vec<GeneratedFile> {
        let mut files = vec![
            (self.name.join("README.md"), self.readme()),
            (self.name.join("main.rs"), self.main_rs()),
        ];
        for (module, source) in self.examples() {
            files.push((self.name.join(&format!("{module}.rs")), source));
        }
        if self.bdd {
            files.push((
                self.name.join("features/example.feature"),
                FEATURE.to_owned(),
            ));
        }
        if self.gha {
            files.push((
                RelativeName::trusted(WORKFLOW_PATH),
                WORKFLOW.replace("<TARGET>", self.target()),
            ));
        }
        files
    }

    fn readme(&self) -> String {
        let target = self.target();
        let path = &self.name;
        let (features, tokio_features, bdd_dependencies) = if self.bdd {
            (
                "\"testing\", \"bdd\"",
                "\"macros\", \"rt-multi-thread\"",
                "rstest-bdd = \"0.6\"\nrstest-bdd-macros = \"0.6\"\n",
            )
        } else {
            ("\"testing\"", "\"macros\", \"rt\"", "")
        };
        let mut rows = String::new();
        for (module, description) in [
            (
                "my_first_test",
                "The `sb` fixture: a simple example test; copy it to start your own.",
            ),
            (
                "parameterized_test",
                "`#[case]`: one test body run for each row of a table, each row its own test.",
            ),
            (
                "cdp_test",
                "The `sb_cdp` fixture: pure CDP, with no chromedriver.",
            ),
        ] {
            if !self.basic {
                rows.push_str(&format!("| `{module}.rs` | {description} |\n"));
            }
        }
        if self.bdd {
            rows.push_str(
                "| `bdd_test.rs` | A Gherkin scenario bound to `features/example.feature`. |\n\
                 | `features/example.feature` | The scenario; its steps are the built-in ones. |\n",
            );
        }
        let workflow = if self.gha {
            format!(
                "\n## GitHub Actions\n\n\
                 `sbrs mkdir --gha` wrote `{WORKFLOW_PATH}` in the folder it ran in, not in\n\
                 this one. GitHub reads workflows from the root of the repository, so move the\n\
                 file there if that folder is not the root. It installs Chrome and chromedriver,\n\
                 runs `cargo test --test {target}` headless, and uploads `latest_logs/`.\n"
            )
        } else {
            String::new()
        };
        format!(
            "\
# {path}

Browser tests generated by `sbrs mkdir`.

## Run them

Cargo runs a folder as one test target when it sits directly inside `tests/`
(`tests/<folder>/main.rs`). If this folder is somewhere else, register it in your
crate's `Cargo.toml`:

```toml
[[test]]
name = \"{target}\"
path = \"{path}/main.rs\"
```

Then run it:

```sh
cargo test --test {target}
```

The tests need Chrome, and a matching chromedriver unless they are pure CDP;
`sbrs doctor` checks both. Your crate also needs the libraries the tests use:

```toml
[dev-dependencies]
seleniumbase-rs = {{ version = \"0.6\", features = [{features}] }}
rstest = \"0.26\"
{bdd_dependencies}tokio = {{ version = \"1\", features = [{tokio_features}] }}
```

## Settings

The `sb` and `sb_cdp` fixtures read `sbase_config.toml` from the directory you run
`cargo test` in, then apply `SB_*` environment variables, which `sbrs test` sets
from its flags (`--reruns`, `--time-limit`, `--env`, ...). For example,
`SB_HEADLESS=true cargo test --test {target}` runs without a window. A failed test
leaves a screenshot, the page source and a report in `latest_logs/`.

## Files

| File | What it is |
| --- | --- |
| `main.rs` | The test target; declares the modules below. |
{rows}{workflow}"
        )
    }

    /// Creates the folder and its files below the scaffold's root and returns the
    /// created paths relative to it.
    ///
    /// Nothing is written if any of the files already exists, unless the
    /// scaffold is replacing files.
    ///
    /// # Errors
    ///
    /// Any [`ScaffoldError`] from [`Scaffold::write_all`].
    pub fn create(&self, scaffold: &Scaffold) -> Result<Vec<PathBuf>, ScaffoldError> {
        scaffold.write_all(&self.files())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(suite: &Suite) -> Vec<String> {
        suite
            .files()
            .iter()
            .map(|(name, _)| name.to_string())
            .collect()
    }

    fn contents(files: &[GeneratedFile], suffix: &str) -> String {
        files
            .iter()
            .find(|(name, _)| name.to_string().ends_with(suffix))
            .unwrap_or_else(|| panic!("no file ending in {suffix}"))
            .1
            .clone()
    }

    #[test]
    fn the_default_suite_has_three_example_tests() {
        assert_eq!(
            names(&Suite::new("ui").unwrap()),
            [
                "ui/README.md",
                "ui/main.rs",
                "ui/my_first_test.rs",
                "ui/parameterized_test.rs",
                "ui/cdp_test.rs"
            ]
        );
    }

    #[test]
    fn basic_leaves_out_the_examples() {
        let suite = Suite::new("ui").unwrap().basic(true);
        assert_eq!(names(&suite), ["ui/README.md", "ui/main.rs"]);
        let files = suite.files();
        let main = contents(&files, "main.rs");
        assert!(!main.contains("mod "), "{main}");
        assert!(main.contains("Add a test by creating"), "{main}");
        assert!(!contents(&files, "README.md").contains("my_first_test"));
    }

    #[test]
    fn bdd_adds_the_scenario_and_its_feature_even_to_a_basic_suite() {
        let suite = Suite::new("tests/ui").unwrap().basic(true).bdd(true);
        assert_eq!(
            names(&suite),
            [
                "tests/ui/README.md",
                "tests/ui/main.rs",
                "tests/ui/bdd_test.rs",
                "tests/ui/features/example.feature"
            ]
        );
        let files = suite.files();
        assert!(
            contents(&files, "bdd_test.rs")
                .contains("path = \"tests/ui/features/example.feature\""),
            "the scenario names its feature from the crate's root"
        );
        assert!(contents(&files, "main.rs").contains("mod bdd_test;"));
        let readme = contents(&files, "README.md");
        assert!(
            readme.contains("features = [\"testing\", \"bdd\"]"),
            "{readme}"
        );
        assert!(readme.contains("rstest-bdd-macros"), "{readme}");
    }

    #[test]
    fn gha_writes_the_workflow_in_the_folder_it_runs_in() {
        let suite = Suite::new("tests/ui-flows").unwrap().gha(true);
        let names = names(&suite);
        assert_eq!(
            names.last().map(String::as_str),
            Some(".github/workflows/sbrs-tests.yml")
        );
        let workflow = contents(&suite.files(), "sbrs-tests.yml");
        assert!(
            workflow.contains("run: cargo test --test ui-flows"),
            "{workflow}"
        );
        assert!(workflow.contains("SB_HEADLESS: \"true\""), "{workflow}");
        assert!(
            workflow.contains("browser-actions/setup-chrome@v2"),
            "{workflow}"
        );
        assert!(
            workflow.contains("dtolnay/rust-toolchain@stable"),
            "{workflow}"
        );
        assert!(workflow.contains("${{ always() }}"), "{workflow}");
        assert!(contents(&suite.files(), "README.md").contains("GitHub Actions"));
        assert!(!contents(&Suite::new("x").unwrap().files(), "README.md").contains("GitHub"));
    }

    #[test]
    fn every_rust_file_parses_and_every_module_is_declared() {
        for (basic, bdd) in [(false, false), (true, false), (false, true), (true, true)] {
            let files = Suite::new("ui").unwrap().basic(basic).bdd(bdd).files();
            let main = contents(&files, "main.rs");
            for (name, source) in &files {
                if name.extension() == Some("rs") {
                    syn::parse_file(source)
                        .unwrap_or_else(|e| panic!("{name} does not parse: {e}\n{source}"));
                    let stem = name.file_stem();
                    if stem != "main" {
                        assert!(
                            main.contains(&format!("mod {stem};")),
                            "{stem} not declared"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_readme_names_the_target_and_the_path() {
        let readme = contents(&Suite::new("tests/ui-flows").unwrap().files(), "README.md");
        assert!(readme.starts_with("# tests/ui-flows\n"), "{readme}");
        assert!(readme.contains("name = \"ui-flows\""), "{readme}");
        assert!(
            readme.contains("path = \"tests/ui-flows/main.rs\""),
            "{readme}"
        );
        assert!(readme.contains("cargo test --test ui-flows"), "{readme}");
        assert!(readme.contains("rstest = \"0.26\""), "{readme}");
    }

    #[test]
    fn hostile_names_never_reach_the_file_system() {
        for bad in ["../x", "/tmp/x", "a/../../x", "x y", "x;rm", "..", ""] {
            assert!(Suite::new(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn creating_twice_fails_without_touching_the_first_copy() {
        let dir = tempfile::tempdir().unwrap();
        let scaffold = Scaffold::new(dir.path());
        let suite = Suite::new("ui").unwrap();
        assert_eq!(suite.create(&scaffold).unwrap().len(), 5);

        std::fs::write(dir.path().join("ui/main.rs"), "// edited").unwrap();
        let error = suite.create(&scaffold).unwrap_err();

        assert!(matches!(error, ScaffoldError::AlreadyExists(_)), "{error}");
        assert_eq!(
            std::fs::read_to_string(dir.path().join("ui/main.rs")).unwrap(),
            "// edited"
        );
    }

    #[test]
    fn an_existing_empty_folder_is_fine() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("ui")).unwrap();
        assert!(Suite::new("ui")
            .unwrap()
            .create(&Scaffold::new(dir.path()))
            .is_ok());
    }

    #[test]
    fn the_workflow_and_the_feature_are_written_in_their_folders() {
        let dir = tempfile::tempdir().unwrap();
        let written = Suite::new("ui")
            .unwrap()
            .bdd(true)
            .gha(true)
            .create(&Scaffold::new(dir.path()))
            .unwrap();
        assert_eq!(written.len(), 8);
        assert!(dir.path().join("ui/features/example.feature").is_file());
        assert!(dir
            .path()
            .join(".github/workflows/sbrs-tests.yml")
            .is_file());
    }
}

// Compile checks. `cargo test --doc --features testing` builds the generated
// modules as a user's test target would, so a template that drifts from the
// API fails here.
#[cfg(all(doctest, feature = "testing"))]
#[doc = concat!(
    "```no_run,test_harness\n",
    "mod my_first_test {\n",
    include_str!("../../../tests/golden/cli/mkdir/my_first_test.rs.golden"),
    "}\n",
    "mod parameterized_test {\n",
    include_str!("../../../tests/golden/cli/mkdir/parameterized_test.rs.golden"),
    "}\n",
    "mod cdp_test {\n",
    include_str!("../../../tests/golden/cli/mkdir/cdp_test.rs.golden"),
    "}\n",
    "fn main() {}\n```"
)]
struct SuiteCompiles;

// The Gherkin scenario needs the feature `bdd`. The golden names its feature
// file from the crate's root, so the feature file itself is stored there too.
#[cfg(all(doctest, feature = "bdd"))]
#[doc = concat!(
    "```no_run,test_harness\n",
    "mod bdd_test {\n",
    include_str!("../../../tests/golden/cli/mkdir_bdd/bdd_test.rs.golden"),
    "}\n",
    "fn main() {}\n```"
)]
struct SuiteScenarioCompiles;
