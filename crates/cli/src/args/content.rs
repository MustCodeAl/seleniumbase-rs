//! The arguments of `mkpres`, `mkchart`, `print`, `translate` and `convert`, and what each
//! command does with them.
//!
//! (Plain comments on the structs, not docs: a doc comment on a struct that derives `Args`
//! would be help text.)

use std::error::Error;
use std::path::{Path, PathBuf};

use seleniumbase_rs::cli::scaffold::ScaffoldError;
use seleniumbase_rs::cli::scripts::sb_mkchart::ChartFile;
use seleniumbase_rs::cli::scripts::sb_mkpres::PresentationFile;
use seleniumbase_rs::cli::scripts::{sb_convert, sb_print, sb_translate};

use super::language::LanguageFlag;

// The arguments of `sbrs mkpres`.
#[derive(Debug, usage::Args)]
#[usage(args_override_self = false)]
pub(crate) struct MkpresArgs {
    /// File to create, for example `tests/deck.rs`.
    #[usage(value_hint = usage::ValueHint::FilePath, extensions("rs"))]
    pub(crate) file: String,
    /// Replace the file if it already exists.
    #[usage(long, bool_value)]
    pub(crate) force: bool,
    #[usage(arg_group)]
    pub(crate) language: Option<LanguageFlag>,
}

impl MkpresArgs {
    /// The test file the arguments describe.
    pub(crate) fn presentation_file(&self) -> Result<PresentationFile, ScaffoldError> {
        let file = PresentationFile::new(&self.file)?;
        Ok(match self.language {
            Some(language) => file.language(language.language()),
            None => file,
        })
    }
}

// The arguments of `sbrs mkchart`.
#[derive(Debug, usage::Args)]
#[usage(args_override_self = false)]
pub(crate) struct MkchartArgs {
    /// File to create, for example `tests/chart.rs`.
    #[usage(value_hint = usage::ValueHint::FilePath, extensions("rs"))]
    pub(crate) file: String,
    /// Replace the file if it already exists.
    #[usage(long, bool_value)]
    pub(crate) force: bool,
    #[usage(arg_group)]
    pub(crate) language: Option<LanguageFlag>,
}

impl MkchartArgs {
    /// The test file the arguments describe.
    pub(crate) fn chart_file(&self) -> Result<ChartFile, ScaffoldError> {
        let file = ChartFile::new(&self.file)?;
        Ok(match self.language {
            Some(language) => file.language(language.language()),
            None => file,
        })
    }
}

// The arguments of `sbrs print`.
#[derive(Debug, usage::Args)]
#[usage(args_override_self = false)]
pub(crate) struct PrintArgs {
    /// File to print.
    #[usage(value_hint = usage::ValueHint::FilePath)]
    pub(crate) file: String,
    /// Put the line number in front of every line.
    #[usage(short = 'n', long = "line-numbers", bool_value)]
    pub(crate) numbers: bool,
}

impl PrintArgs {
    /// Prints the file to standard output.
    ///
    /// A failure to read it carries the file's name and has the status of a local file error.
    pub(crate) fn run(&self) -> Result<(), Box<dyn Error>> {
        let text = sb_print::print_file(&self.file).map_err(|error| {
            std::io::Error::new(
                error.kind(),
                format!("failed to read {}: {error}", self.file),
            )
        })?;
        if self.numbers {
            print!("{}", sb_print::number_lines(&text));
        } else {
            print!("{text}");
        }
        Ok(())
    }
}

// The arguments of `sbrs translate`.
#[derive(Debug, usage::Args)]
#[usage(args_override_self = false)]
pub(crate) struct TranslateArgs {
    /// The scenario file.
    #[usage(value_hint = usage::ValueHint::FilePath, extensions("json"))]
    pub(crate) file: PathBuf,
    #[usage(arg_group)]
    pub(crate) language: LanguageFlag,
    /// Print the translation (the default when no other action is given). With -o or -c it
    /// is printed as well as written.
    #[usage(short, long, bool_value)]
    pub(crate) print: bool,
    /// Replace the file with the translation.
    #[usage(short, long, bool_value, conflicts("--copy"))]
    pub(crate) overwrite: bool,
    /// Write the translation to a new file next to it, such as `login_es.json`.
    #[usage(short, long, bool_value)]
    pub(crate) copy: bool,
    /// Put the line number in front of every printed line.
    #[usage(short = 'n', long = "line-numbers", bool_value)]
    pub(crate) numbers: bool,
}

impl TranslateArgs {
    /// What the arguments ask `translate` to write, if anything.
    pub(crate) const fn action(&self) -> sb_translate::TranslateAction {
        if self.overwrite {
            sb_translate::TranslateAction::Overwrite
        } else if self.copy {
            sb_translate::TranslateAction::Copy
        } else {
            sb_translate::TranslateAction::Print
        }
    }

    /// Whether the translation is printed: asked for with `-p`, or the default when nothing is
    /// written.
    pub(crate) const fn prints(&self) -> bool {
        self.print || matches!(self.action(), sb_translate::TranslateAction::Print)
    }

    /// Translates the scenario and says what happened: the scenario for a print, the file
    /// written, and a note on the actions the dictionary does not know.
    pub(crate) fn run(&self) -> Result<(), Box<dyn Error>> {
        let command = sb_translate::TranslateCommand {
            file: self.file.clone(),
            language: self.language.language(),
            action: self.action(),
        };
        let report = sb_translate::run(&command)?;
        if self.prints() {
            if self.numbers {
                print!("{}", sb_print::number_lines(&report.text));
            } else {
                print!("{}", report.text);
            }
        }
        if let Some(path) = &report.written {
            println!(
                "Wrote {} ({} steps in {})",
                path.display(),
                report.translated,
                command.language
            );
        }
        if !report.unknown.is_empty() {
            eprintln!(
                "sbrs: left {} unknown action(s) as they were: {}",
                report.unknown.len(),
                report.unknown.join(", ")
            );
        }
        Ok(())
    }
}

// The arguments of `sbrs convert`.
#[derive(Debug, usage::Args)]
#[usage(args_override_self = false)]
pub(crate) struct ConvertArgs {
    /// The exported Selenium WebDriver test (a Selenium IDE or Katalon Recorder export).
    #[usage(value_hint = usage::ValueHint::FilePath, extensions("py"))]
    pub(crate) file: String,
    /// Replace the converted file if it already exists.
    #[usage(long, bool_value)]
    pub(crate) force: bool,
}

impl ConvertArgs {
    /// Converts the file, prints where the Rust went and what could not be converted.
    ///
    /// As `import-python`, a statement the importer could only leave as a `TODO` is a warning
    /// and a statement it could not read at all an error: the file is still written, and the
    /// command then fails (status 1).
    pub(crate) fn run(&self) -> Result<(), Box<dyn Error>> {
        let report = sb_convert::convert(Path::new(&self.file), self.force)?;
        println!("Created {}", report.output.display());
        for diagnostic in &report.diagnostics {
            eprintln!(
                "{:?} at line {}: {}",
                diagnostic.severity, diagnostic.line, diagnostic.message
            );
        }
        if report.complete {
            Ok(())
        } else {
            Err(format!(
                "the conversion reported errors; review the TODOs in {}",
                report.output.display()
            )
            .into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn translate(overwrite: bool, copy: bool, print: bool) -> TranslateArgs {
        TranslateArgs {
            file: PathBuf::from("login.json"),
            language: LanguageFlag::Es,
            print,
            overwrite,
            copy,
            numbers: false,
        }
    }

    #[test]
    fn print_is_the_default_action_and_pairs_with_one_other() {
        let default = translate(false, false, false);
        assert_eq!(default.action(), sb_translate::TranslateAction::Print);
        assert!(default.prints());

        let overwrite = translate(true, false, false);
        assert_eq!(overwrite.action(), sb_translate::TranslateAction::Overwrite);
        assert!(!overwrite.prints(), "-o alone writes and prints nothing");

        let copy_and_print = translate(false, true, true);
        assert_eq!(copy_and_print.action(), sb_translate::TranslateAction::Copy);
        assert!(copy_and_print.prints(), "-p -c prints and writes");
    }

    #[test]
    fn mkpres_and_mkchart_take_the_language_of_the_flag() {
        let pres = MkpresArgs {
            file: "deck".to_owned(),
            force: false,
            language: Some(LanguageFlag::Zh),
        };
        assert!(pres.presentation_file().unwrap().render().contains("你好"));
        let chart = MkchartArgs {
            file: "chart".to_owned(),
            force: false,
            language: Some(LanguageFlag::Ja),
        };
        assert!(chart.chart_file().unwrap().render().contains("物体 A"));
        let plain = MkpresArgs {
            file: "deck".to_owned(),
            force: false,
            language: None,
        };
        assert!(plain
            .presentation_file()
            .unwrap()
            .render()
            .contains("Hello"));
        assert!(MkpresArgs {
            file: "../deck".to_owned(),
            ..plain
        }
        .presentation_file()
        .is_err());
    }
}
