//! `sbase mkdir`: create a folder holding a runnable browser-test suite.
//!
//! The suite is one Cargo test target: a `main.rs` that declares a `helpers`
//! module and the example tests, which use
//! [`run_browser_test`](crate::run_browser_test). The golden files under
//! `tests/golden/cli/` pin every generated file, and the `cfg(doctest)` item at
//! the bottom of this file compiles the Rust modules against the current API on
//! every `cargo test`.

use std::path::PathBuf;

use crate::cli::scaffold::{GeneratedFile, RelativeName, Scaffold, ScaffoldError};

const MAIN_HEADER: &str = "\
//! Browser tests for this folder. See README.md for how Cargo finds them.
";

const MAIN_DEFAULT: &str = "\
//! Browser tests for this folder. See README.md for how Cargo finds them.

mod helpers;
mod my_first_test;
mod parameterized_test;
";

const HELPERS: &str = "\
//! Shared setup for the tests in this folder.

use seleniumbase_rs::config::settings::Settings;
use seleniumbase_rs::{BrowserConfig, Result};

/// The browser configuration every test uses.
///
/// It reads `sbase_config.toml` from the directory `cargo test` runs in, then
/// lets `SB_*` environment variables (for example `SB_HEADLESS=true`) override it.
pub fn config() -> Result<BrowserConfig> {
    Ok(Settings::load(None::<&str>)?.to_browser_config())
}
";

const MY_FIRST_TEST: &str = "\
//! The first example test. Copy it to start your own.

use seleniumbase_rs::{run_browser_test, Result};

use crate::helpers;

#[tokio::test]
async fn my_first_test() -> Result<()> {
    run_browser_test(helpers::config()?, |sb| {
        Box::pin(async move {
            sb.open(\"https://example.com\").await?;
            sb.assert_title(\"Example Domain\").await?;
            sb.assert_text(\"h1\", \"Example Domain\").await
        })
    })
    .await
}
";

const PARAMETERIZED_TEST: &str = "\
//! A parameterized test: one browser session for each row of the table below.
//!
//! Rust has no built-in parameterized tests; a loop over a table does the job.

use seleniumbase_rs::{run_browser_test, Result};

use crate::helpers;

/// `(page, text expected in its heading)`.
const PAGES: [(&str, &str); 2] = [
    (\"https://example.com\", \"Example Domain\"),
    (\"https://example.org\", \"Example Domain\"),
];

#[tokio::test]
async fn every_page_has_its_heading() -> Result<()> {
    for (page, heading) in PAGES {
        run_browser_test(helpers::config()?, move |sb| {
            Box::pin(async move {
                sb.open(page).await?;
                sb.assert_text(\"h1\", heading).await
            })
        })
        .await?;
    }
    Ok(())
}
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
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Suite {
    name: RelativeName,
    basic: bool,
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
        })
    }

    /// Generates only the scaffolding (`main.rs`, `helpers.rs`, `README.md`) and
    /// no example tests.
    #[must_use]
    pub fn basic(mut self, basic: bool) -> Self {
        self.basic = basic;
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

    /// Every file of the suite, in the order they are created.
    #[must_use]
    pub fn files(&self) -> Vec<GeneratedFile> {
        let main = if self.basic {
            format!(
                "{MAIN_HEADER}//!\n\
                 //! Add a test by creating `my_test.rs` here with a `#[tokio::test]` function,\n\
                 //! then declaring it below with `mod my_test;`.\n\
                 \n\
                 // Remove this attribute once a test uses `helpers::config`.\n\
                 #[allow(dead_code)]\n\
                 mod helpers;\n"
            )
        } else {
            MAIN_DEFAULT.to_owned()
        };
        let mut files = vec![
            (self.name.join("README.md"), self.readme()),
            (self.name.join("main.rs"), main),
            (self.name.join("helpers.rs"), HELPERS.to_owned()),
        ];
        if !self.basic {
            files.push((self.name.join("my_first_test.rs"), MY_FIRST_TEST.to_owned()));
            files.push((
                self.name.join("parameterized_test.rs"),
                PARAMETERIZED_TEST.to_owned(),
            ));
        }
        files
    }

    fn readme(&self) -> String {
        let target = self.target();
        let path = &self.name;
        let examples = if self.basic {
            String::new()
        } else {
            "| `my_first_test.rs` | A simple example test; copy it to start your own. |\n\
             | `parameterized_test.rs` | One test body run for each row of a table. |\n"
                .to_owned()
        };
        format!(
            "\
# {path}

Browser tests generated by `sbase mkdir`.

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

The tests need Chrome and a matching chromedriver; `sbase doctor` checks both.
Your crate also needs the libraries the tests use:

```toml
[dev-dependencies]
seleniumbase-rs = \"0.1\"
tokio = {{ version = \"1\", features = [\"macros\", \"rt-multi-thread\"] }}
```

## Settings

`helpers::config()` reads `sbase_config.toml` from the directory you run
`cargo test` in, then applies `SB_*` environment variables. For example,
`SB_HEADLESS=true cargo test --test {target}` runs without a window.

## Files

| File | What it is |
| --- | --- |
| `main.rs` | The test target; declares the modules below. |
| `helpers.rs` | Setup shared by the tests. |
{examples}"
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

    fn contents(files: &[GeneratedFile], suffix: &str) -> String {
        files
            .iter()
            .find(|(name, _)| name.to_string().ends_with(suffix))
            .unwrap_or_else(|| panic!("no file ending in {suffix}"))
            .1
            .clone()
    }

    #[test]
    fn the_default_suite_has_two_example_tests() {
        let files = Suite::new("ui").unwrap().files();
        let names: Vec<String> = files.iter().map(|(name, _)| name.to_string()).collect();
        assert_eq!(
            names,
            [
                "ui/README.md",
                "ui/main.rs",
                "ui/helpers.rs",
                "ui/my_first_test.rs",
                "ui/parameterized_test.rs"
            ]
        );
    }

    #[test]
    fn basic_leaves_out_the_examples_and_keeps_helpers_quiet() {
        let files = Suite::new("ui").unwrap().basic(true).files();
        let names: Vec<String> = files.iter().map(|(name, _)| name.to_string()).collect();
        assert_eq!(names, ["ui/README.md", "ui/main.rs", "ui/helpers.rs"]);
        let main = contents(&files, "main.rs");
        assert!(main.contains("#[allow(dead_code)]\nmod helpers;"), "{main}");
        assert!(!main.contains("my_first_test"), "{main}");
        assert!(!contents(&files, "README.md").contains("my_first_test"));
    }

    #[test]
    fn every_rust_file_parses_and_every_module_is_declared() {
        for basic in [false, true] {
            let files = Suite::new("ui").unwrap().basic(basic).files();
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
}

// Compile check. `cargo test --doc` builds the generated modules as a user's test
// target would, so a template that drifts from the API fails here.
#[cfg(doctest)]
#[doc = concat!(
    "```no_run,test_harness\n",
    "mod helpers {\n",
    include_str!("../../../tests/golden/cli/mkdir/helpers.rs.golden"),
    "}\n",
    "mod my_first_test {\n",
    include_str!("../../../tests/golden/cli/mkdir/my_first_test.rs.golden"),
    "}\n",
    "mod parameterized_test {\n",
    include_str!("../../../tests/golden/cli/mkdir/parameterized_test.rs.golden"),
    "}\n",
    "fn main() {}\n```"
)]
struct SuiteCompiles;
