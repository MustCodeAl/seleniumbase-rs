//! `python_to_rust` and `scaffold`: Rust test code for a model to write.
//!
//! Both only compute text. `python_to_rust` is the importer of
//! `sbrs import-python`; `scaffold` renders the files `sbrs mkfile` and
//! `sbrs mkdir` create, from the same templates and through the same name
//! checks, and leaves writing them to the caller.

use std::fmt::Write as _;

use serde_json::json;

use super::NoBrowser;
use crate::cli::scaffold::ScaffoldError;
use crate::cli::scripts::sb_mkdir::Suite;
use crate::cli::scripts::sb_mkfile::TestFile;
use crate::error::SeleniumBaseError;
use crate::mcp::devtools::is_test_name;
use crate::mcp::support::{json_output, nonempty};
use crate::mcp::{Args, Ctx, Effect, Output, Prop, Schema, ToolDef, ToolError};
use crate::utilities::python_importer::{
    import_python, ImportOptions, ImportSeverity, PythonSource,
};

/// The most Python source one call converts.
const MAX_CODE_BYTES: usize = 256 * 1024;

/// The tools of this file.
pub(super) fn tools() -> Vec<ToolDef<NoBrowser>> {
    vec![
        ToolDef::new(
            "python_to_rust",
            "Python To Rust",
            "Convert a Python SeleniumBase or Selenium test to a Rust test for this library, as \
             `sbrs import-python` does. The Python is read, never run. Statements it cannot \
             translate stay as `// TODO` comments in the Rust, and each one is also a \
             diagnostic with its line number; review them before using the test. The result \
             says which API family it detected.",
            Effect::Inspect,
            Schema::new()
                .required(
                    "code",
                    Prop::string("The Python source of the test, at most 256 KiB"),
                )
                .optional(
                    "source",
                    Prop::choice(
                        &["auto", "seleniumbase", "selenium"],
                        "Which Python API the code uses; auto looks at its imports and calls",
                    )
                    .default("auto"),
                )
                .optional(
                    "test_name",
                    Prop::string("Name of the Rust test function").default("imported_python_test"),
                ),
            python_to_rust,
        ),
        ToolDef::new(
            "scaffold",
            "Scaffold",
            "Show the files `sbrs mkfile` (kind test: one browser-test file) or `sbrs mkdir` \
             (kind suite: a folder with main.rs, helpers.rs, a README and example tests) would \
             create, as text with each file's path and content. Writes nothing: create the \
             files yourself. The names follow the same rules as the commands.",
            Effect::Inspect,
            Schema::new()
                .required(
                    "kind",
                    Prop::choice(
                        &["test", "suite"],
                        "A single test file, or a folder holding a test suite",
                    ),
                )
                .required(
                    "name",
                    Prop::string(
                        "Relative name: `login` or `tests/login.rs` for a test, `tests/ui` for \
                         a suite; letters, digits, _, - and . only",
                    ),
                )
                .optional(
                    "url",
                    Prop::string("kind test: the page the test opens (an http, https or file URL)"),
                )
                .optional(
                    "basic",
                    Prop::boolean(
                        "Leave out the example assertions (test) or the example tests (suite)",
                    )
                    .default(false),
                ),
            scaffold,
        ),
    ]
}

async fn python_to_rust(_ctx: Ctx<NoBrowser>, args: Args) -> Result<Output, ToolError> {
    let code = args.str("code")?;
    if code.len() > MAX_CODE_BYTES {
        return Err(ToolError::invalid(
            "code",
            format!("larger than {MAX_CODE_BYTES} bytes; convert it in parts"),
        ));
    }
    let source = match args.choice("source", &["auto", "seleniumbase", "selenium"], "auto")? {
        "seleniumbase" => PythonSource::SeleniumBase,
        "selenium" => PythonSource::Selenium,
        _ => PythonSource::Auto,
    };
    let test_name = args.str_or("test_name", "imported_python_test")?;
    if !is_test_name(test_name) {
        return Err(ToolError::invalid(
            "test_name",
            "use letters, digits and _ only, starting with a letter, at most 64 characters",
        ));
    }
    let options = ImportOptions {
        source,
        test_name: test_name.to_owned(),
    };
    let code = code.to_owned();
    // The importer is plain synchronous code; a panic in it must not take the
    // server down, so it runs where a panic comes back as a `JoinError`.
    let result = tokio::task::spawn_blocking(move || import_python(&code, &options))
        .await
        .map_err(|error| {
            ToolError::Failed(SeleniumBaseError::python_migration(format!(
                "the importer stopped on this input: {error}"
            )))
        })?;
    let diagnostics: Vec<_> = result
        .diagnostics
        .iter()
        .map(|diagnostic| {
            json!({
                "line": diagnostic.line,
                "severity": match diagnostic.severity {
                    ImportSeverity::Error => "error",
                    ImportSeverity::Warning => "warning",
                },
                "message": diagnostic.message,
            })
        })
        .collect();
    json_output(json!({
        "detected_source": match result.detected_source {
            PythonSource::SeleniumBase => "seleniumbase",
            PythonSource::Selenium => "selenium",
            PythonSource::Auto => "auto",
        },
        "complete": result.is_complete(),
        "diagnostics": diagnostics,
        "rust": result.rust,
        "note": "A static conversion: the Python was read, not run. Every `// TODO` comment is a \
                 statement to port by hand; parity_lookup and find_api find the Rust methods.",
    }))
}

async fn scaffold(_ctx: Ctx<NoBrowser>, args: Args) -> Result<Output, ToolError> {
    let kind = args.choice("kind", &["test", "suite"], "test")?;
    let name = args.str("name")?;
    let url = nonempty(&args, "url")?;
    let basic = args.bool_or("basic", false)?;
    let bad = |error: ScaffoldError| {
        let argument = match error {
            ScaffoldError::InvalidValue { .. } => "url",
            _ => "name",
        };
        ToolError::invalid(argument, error.to_string())
    };

    let (files, next) = if kind == "suite" {
        if url.is_some() {
            return Err(ToolError::invalid(
                "url",
                "only a test (kind test) takes a URL",
            ));
        }
        let suite = Suite::new(name).map_err(bad)?.basic(basic);
        let next = format!(
            "Run it with `cargo test --test {}` (a folder directly inside tests/ is one test \
             target; the README says how to register it elsewhere). It needs Chrome and a \
             matching chromedriver: `sbrs doctor` checks both.",
            suite.target()
        );
        (suite.files(), next)
    } else {
        let mut file = TestFile::new(name).map_err(bad)?.basic(basic);
        if let Some(url) = url {
            file = file.url(url).map_err(bad)?;
        }
        let stem = file
            .name()
            .to_string()
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .trim_end_matches(".rs")
            .to_owned();
        let next = format!(
            "Run it with `cargo test --test {stem}` when the file is in your crate's tests/ \
             folder. It needs Chrome and a matching chromedriver: `sbrs doctor` checks both."
        );
        (vec![(file.name().clone(), file.render())], next)
    };

    let mut text = format!(
        "Scaffold for kind {kind}, name {name}. Nothing was written; create these {} file(s) \
         yourself.\n",
        files.len()
    );
    for (path, content) in &files {
        let _ = write!(text, "\n=== {path} ===\n{content}");
        if !content.ends_with('\n') {
            text.push('\n');
        }
    }
    let _ = write!(text, "\n{next}\n");
    Ok(text.into())
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;
    use crate::mcp::Settings;

    fn host() -> crate::mcp::Host<NoBrowser> {
        super::super::host(Settings::new("out"))
    }

    async fn call(tool: &str, arguments: Value) -> Result<Output, ToolError> {
        host()
            .call(tool, arguments.as_object().cloned().unwrap())
            .await
    }

    #[tokio::test]
    async fn a_seleniumbase_test_becomes_a_rust_test_and_says_what_it_detected() {
        let python = "from seleniumbase import BaseCase\n\
                      class T(BaseCase):\n    def test_x(self):\n        \
                      self.open('https://example.com')\n        \
                      self.assert_text('Example Domain', 'h1')\n";

        let answer = call(
            "python_to_rust",
            json!({ "code": python, "test_name": "example" }),
        )
        .await
        .unwrap();

        let answer = answer.as_json().unwrap();
        assert_eq!(answer["detected_source"], "seleniumbase");
        assert_eq!(answer["complete"], true);
        let rust = answer["rust"].as_str().unwrap();
        assert!(rust.contains("async fn example"), "{rust}");
        assert!(rust.contains("https://example.com"), "{rust}");
        assert_eq!(answer["diagnostics"], json!([]));
    }

    #[tokio::test]
    async fn what_the_importer_cannot_read_comes_back_as_diagnostics_with_lines() {
        let python = "from seleniumbase import BaseCase\n\
                      class T(BaseCase):\n    def test_x(self):\n        \
                      self.open('https://example.com')\n        \
                      self.some_helper_nobody_has()\n";

        let answer = call("python_to_rust", json!({ "code": python }))
            .await
            .unwrap();

        let answer = answer.as_json().unwrap();
        let diagnostics = answer["diagnostics"].as_array().unwrap();
        assert!(!diagnostics.is_empty(), "{answer}");
        assert!(diagnostics[0]["line"].as_u64().unwrap() >= 1);
        assert!(["warning", "error"].contains(&diagnostics[0]["severity"].as_str().unwrap()));
        assert!(answer["rust"].as_str().unwrap().contains("TODO"));
    }

    #[tokio::test]
    async fn the_selenium_family_can_be_chosen() {
        let python = "from selenium import webdriver\ndriver = webdriver.Chrome()\n\
                      driver.get('https://example.com')\n";

        let answer = call(
            "python_to_rust",
            json!({ "code": python, "source": "selenium" }),
        )
        .await
        .unwrap();

        assert_eq!(answer.as_json().unwrap()["detected_source"], "selenium");
    }

    #[tokio::test]
    async fn bad_arguments_of_python_to_rust_name_the_argument() {
        for (arguments, name) in [
            (
                json!({ "code": "x", "test_name": "not a name" }),
                "test_name",
            ),
            (json!({ "code": "x", "test_name": "1abc" }), "test_name"),
            (json!({ "code": "x", "source": "ruby" }), "source"),
            (json!({ "code": "x".repeat(MAX_CODE_BYTES + 1) }), "code"),
            (json!({}), "code"),
        ] {
            let error = call("python_to_rust", arguments).await.unwrap_err();
            assert!(
                matches!(&error, ToolError::InvalidArgument { name: given, .. } if given == name),
                "{name}: {error}"
            );
        }
    }

    #[tokio::test]
    async fn a_test_scaffold_is_what_sbrs_mkfile_writes() {
        let answer = call("scaffold", json!({ "kind": "test", "name": "tests/login" }))
            .await
            .unwrap();

        let text = answer.as_text().unwrap();
        let expected = TestFile::new("tests/login").unwrap().render();
        assert!(text.contains("=== tests/login.rs ==="), "{text}");
        assert!(text.contains(&expected), "{text}");
        assert!(text.contains("Nothing was written"), "{text}");
        assert!(text.contains("cargo test --test login"), "{text}");
    }

    #[tokio::test]
    async fn a_suite_scaffold_is_what_sbrs_mkdir_writes_and_basic_leaves_out_the_examples() {
        let full = call("scaffold", json!({ "kind": "suite", "name": "tests/ui" }))
            .await
            .unwrap();
        let basic = call(
            "scaffold",
            json!({ "kind": "suite", "name": "tests/ui", "basic": true }),
        )
        .await
        .unwrap();

        let full = full.as_text().unwrap();
        for (path, content) in Suite::new("tests/ui").unwrap().files() {
            assert!(full.contains(&format!("=== {path} ===")), "{path}");
            assert!(full.contains(&content), "{path}");
        }
        assert!(full.contains("my_first_test.rs"), "{full}");
        assert!(!basic
            .as_text()
            .unwrap()
            .contains("=== tests/ui/my_first_test.rs"));
        assert!(basic
            .as_text()
            .unwrap()
            .contains("=== tests/ui/main.rs ==="));
    }

    #[tokio::test]
    async fn a_test_scaffold_can_open_a_given_page() {
        let answer = call(
            "scaffold",
            json!({ "kind": "test", "name": "shop", "url": "https://shop.test/cart" }),
        )
        .await
        .unwrap();

        assert!(answer.as_text().unwrap().contains("https://shop.test/cart"));
    }

    #[tokio::test]
    async fn names_that_could_leave_the_folder_are_refused_and_nothing_is_written() {
        let dir = tempfile::tempdir().unwrap();
        let host = super::super::host(Settings::new(dir.path()));
        for name in [
            "../escape",
            "/etc/passwd",
            "a b",
            "-flag",
            ".hidden",
            "a\\b",
            "",
        ] {
            let error = host
                .call(
                    "scaffold",
                    json!({ "kind": "test", "name": name })
                        .as_object()
                        .cloned()
                        .unwrap(),
                )
                .await
                .unwrap_err();
            assert!(
                matches!(&error, ToolError::InvalidArgument { name: given, .. } if given == "name"),
                "{name:?}: {error}"
            );
        }
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn a_url_must_be_a_web_or_file_address_and_belongs_to_a_test() {
        let bad_url = call(
            "scaffold",
            json!({ "kind": "test", "name": "a", "url": "javascript:alert(1)" }),
        )
        .await
        .unwrap_err();
        assert!(
            matches!(&bad_url, ToolError::InvalidArgument { name, .. } if name == "url"),
            "{bad_url}"
        );
        let suite_url = call(
            "scaffold",
            json!({ "kind": "suite", "name": "a", "url": "https://example.com" }),
        )
        .await
        .unwrap_err();
        assert!(
            matches!(&suite_url, ToolError::InvalidArgument { name, .. } if name == "url"),
            "{suite_url}"
        );
    }
}
