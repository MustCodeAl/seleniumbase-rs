//! The arguments of `extract-objects`, `inject-objects`, `objectify` and `revert-objects`, and
//! what each command says when it is done.
//!
//! (Plain comments on the structs, not docs: a doc comment on a struct that derives `Args`
//! would be help text.)

use std::error::Error;
use std::path::Path;

use seleniumbase_rs::cli::scripts::sb_objectify::{self, ExtractReport, RewriteReport};

// The arguments of `sbrs extract-objects`.
#[derive(Debug, usage::Args)]
#[usage(args_override_self = false)]
pub(crate) struct ExtractObjectsArgs {
    /// The Rust test file whose selectors become page objects.
    #[usage(value_hint = usage::ValueHint::FilePath, extensions("rs"))]
    pub(crate) file: String,
}

impl ExtractObjectsArgs {
    /// Writes the page objects of the file and says how many there were.
    pub(crate) fn run(&self) -> Result<(), Box<dyn Error>> {
        let report = sb_objectify::extract_objects(Path::new(&self.file))?;
        println!("{}", extract_message(&self.file, &report));
        Ok(())
    }
}

// The arguments of `sbrs inject-objects` and `sbrs objectify`.
#[derive(Debug, usage::Args)]
#[usage(args_override_self = false)]
pub(crate) struct InjectObjectsArgs {
    /// The Rust test file whose selectors are replaced by page objects.
    #[usage(value_hint = usage::ValueHint::FilePath, extensions("rs"))]
    pub(crate) file: String,
    /// Add the selector as a comment, such as `// "#user"`, at the end of each line it replaced.
    #[usage(short, long, bool_value)]
    pub(crate) comments: bool,
}

impl InjectObjectsArgs {
    /// Replaces the selectors of the file by the page objects and says how many.
    pub(crate) fn inject(&self) -> Result<(), Box<dyn Error>> {
        let report = sb_objectify::inject_objects(Path::new(&self.file), self.comments)?;
        println!("{}", inject_message(&self.file, &report));
        Ok(())
    }

    /// Extracts the page objects of the file, then injects them, and says what each did.
    pub(crate) fn objectify(&self) -> Result<(), Box<dyn Error>> {
        let (extracted, injected) = sb_objectify::objectify(Path::new(&self.file), self.comments)?;
        println!("{}", extract_message(&self.file, &extracted));
        if extracted.found > 0 {
            println!("{}", inject_message(&self.file, &injected));
        }
        Ok(())
    }
}

// The arguments of `sbrs revert-objects`.
#[derive(Debug, usage::Args)]
#[usage(args_override_self = false)]
pub(crate) struct RevertObjectsArgs {
    /// The Rust test file whose page objects are replaced by their selectors.
    #[usage(value_hint = usage::ValueHint::FilePath, extensions("rs"))]
    pub(crate) file: String,
    /// Also take off the comments that `inject-objects -c` appended to the lines it changed.
    #[usage(short, long, bool_value)]
    pub(crate) comments: bool,
}

impl RevertObjectsArgs {
    /// Puts the selectors back and says how many.
    pub(crate) fn run(&self) -> Result<(), Box<dyn Error>> {
        let report = sb_objectify::revert_objects(Path::new(&self.file), self.comments)?;
        println!("{}", revert_message(&self.file, &report));
        Ok(())
    }
}

/// `count` and `word`, the word made plural when the count is not one.
fn counted(count: usize, word: &str) -> String {
    if count == 1 {
        format!("1 {word}")
    } else {
        format!("{count} {word}s")
    }
}

/// What `extract-objects` says.
fn extract_message(file: &str, report: &ExtractReport) -> String {
    let objects = report.objects_file.display();
    if report.found == 0 {
        format!("No selectors found in {file}.")
    } else if report.added == 0 {
        format!("{objects} already has the selectors of {file}.")
    } else {
        format!(
            "Wrote {} to {objects}.",
            counted(report.added, "page object")
        )
    }
}

/// What `inject-objects` says.
fn inject_message(file: &str, report: &RewriteReport) -> String {
    if report.changed == 0 {
        format!("No selector of {file} has a page object.")
    } else {
        format!(
            "Replaced {} in {file} with page objects.",
            counted(report.changed, "selector")
        )
    }
}

/// What `revert-objects` says.
fn revert_message(file: &str, report: &RewriteReport) -> String {
    if report.changed == 0 {
        format!("{file} uses no page object.")
    } else {
        format!(
            "Restored {} in {file}.",
            counted(report.changed, "selector")
        )
    }
}
