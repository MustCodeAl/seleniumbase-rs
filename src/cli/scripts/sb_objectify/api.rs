//! The four commands: the functions on text and the functions on files.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::edit::{apply, line_end, line_ending, line_start, text_end, Edit};
use super::lexer::{lex, Kind, Lexed};
use super::model::{module_name, PageObjects};
use super::scan::scan;
use super::{MODULE_LINE, PAGE_OBJECTS_FILE};
use crate::error::SeleniumBaseError;

/// What `inject-objects -c` puts before the selectors it appends to a line.
const COMMENT_START: &str = " // ";

/// What [`extract_text`] made of a source file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Extraction {
    /// The whole new text of `page_objects.rs`.
    pub page_objects: String,
    /// How many selector literals the source holds.
    pub found: usize,
    /// How many page objects (constants) were new.
    pub added: usize,
}

/// What [`inject_text`] and [`revert_text`] made of a source file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rewrite {
    /// The whole new text of the test file.
    pub source: String,
    /// How many selectors were replaced by page objects, or page objects by selectors.
    pub changed: usize,
}

/// What `extract-objects` did to the page objects file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtractReport {
    /// The page objects file next to the test file.
    pub objects_file: PathBuf,
    /// How many selector literals the test file holds.
    pub found: usize,
    /// How many page objects were new, and written to the file.
    pub added: usize,
}

/// What `inject-objects` or `revert-objects` did to the test file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RewriteReport {
    /// The test file.
    pub test_file: PathBuf,
    /// How many selectors were replaced by page objects, or page objects by selectors.
    pub changed: usize,
}

/// The page objects file that belongs to `test_file`: `page_objects.rs` in the same folder.
#[must_use]
pub fn objects_path(test_file: &Path) -> PathBuf {
    test_file.with_file_name(PAGE_OBJECTS_FILE)
}

/// The page objects `source` needs, merged into the text of an existing page objects file.
///
/// The selectors are found as [`extract_objects`] describes. A selector that the page already
/// has is not added again; the others get the name the module docs describe, after the ones the
/// file has.
///
/// # Errors
///
/// [`SeleniumBaseError::InvalidConfig`] when `existing` is not a file this module wrote.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::cli::scripts::sb_objectify::extract_text;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let source = r##"
/// async fn login(sb: &mut BaseCase) {
///     sb.open("https://example.com/login").await?;
///     sb.type_text("#user", "ada").await?;
/// }
/// "##;
///
/// let extraction = extract_text(source, None)?;
///
/// assert_eq!(extraction.found, 1);
/// assert!(extraction.page_objects.contains(
///     "pub mod example_com_login {\n    pub const USER: &str = \"#user\";\n}"
/// ));
/// # Ok(())
/// # }
/// ```
pub fn extract_text(source: &str, existing: Option<&str>) -> Result<Extraction, SeleniumBaseError> {
    let mut objects = match existing {
        Some(text) => read_objects(text)?,
        None => PageObjects::default(),
    };
    let lexed = lex(source);
    let found = scan(source, &lexed);
    let mut added = 0;
    for hit in &found {
        if objects.add(hit.url.as_deref(), &hit.literal) {
            added += 1;
        }
    }
    Ok(Extraction {
        page_objects: objects.render(),
        found: found.len(),
        added,
    })
}

/// Replaces the selector literals of `source` by the page objects of `page_objects`, and
/// declares the module that holds them.
///
/// A selector that has no page object stays as it is. With `comments`, every line that changed
/// ends with `// ` and the selectors it had; a line that ends inside a string or a block comment
/// gets none. When nothing changes, `source` is returned as it was.
///
/// # Errors
///
/// [`SeleniumBaseError::InvalidConfig`] when `page_objects` is not a file this module wrote.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::cli::scripts::sb_objectify::{extract_text, inject_text};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let source = "fn t() {\n    sb.click(\"#go\");\n}\n";
/// let objects = extract_text(source, None)?.page_objects;
///
/// let rewrite = inject_text(source, &objects, false)?;
///
/// assert_eq!(
///     rewrite.source,
///     "mod page_objects;\nfn t() {\n    sb.click(page_objects::page::GO);\n}\n"
/// );
/// # Ok(())
/// # }
/// ```
pub fn inject_text(
    source: &str,
    page_objects: &str,
    comments: bool,
) -> Result<Rewrite, SeleniumBaseError> {
    let objects = read_objects(page_objects)?;
    let lexed = lex(source);
    let found = scan(source, &lexed);
    let mut edits = Vec::new();
    let mut by_line: BTreeMap<usize, Vec<&str>> = BTreeMap::new();
    for hit in &found {
        let module = module_name(hit.url.as_deref());
        let Some(constant) = objects.constant(&module, &hit.literal) else {
            continue;
        };
        edits.push(Edit {
            start: hit.start,
            end: hit.end,
            text: format!(
                "{}page_objects::{module}::{}",
                "super::".repeat(hit.mod_depth),
                constant.name
            ),
        });
        by_line
            .entry(line_end(source, hit.end))
            .or_default()
            .push(&hit.literal);
    }
    let changed = edits.len();
    if changed == 0 {
        return Ok(Rewrite {
            source: source.to_owned(),
            changed,
        });
    }
    if comments {
        for (end, literals) in &by_line {
            if !lexed.spans_line_break(*end) {
                let at = text_end(source, *end);
                edits.push(Edit {
                    start: at,
                    end: at,
                    text: format!("{COMMENT_START}{}", literals.join(", ")),
                });
            }
        }
    }
    edits.extend(declare_module(source, &lexed));
    Ok(Rewrite {
        source: apply(source, edits),
        changed,
    })
}

/// Replaces the page objects of `source` by the selector literals they stand for.
///
/// Only a path that names a constant of `page_objects` is replaced, as `inject_text` wrote it:
/// `page_objects::module::NAME`, with a `super::` for every inline module the call is in. With
/// `comments`, the `// selector` comment that `inject_text` appended to a line goes too; without
/// it the comments stay. The `mod page_objects;` line goes when the file does not use
/// `page_objects` any more. When nothing changes, `source` is returned as it was.
///
/// # Errors
///
/// [`SeleniumBaseError::InvalidConfig`] when `page_objects` is not a file this module wrote.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::cli::scripts::sb_objectify::{extract_text, inject_text, revert_text};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let source = "fn t() {\n    sb.click(\"#go\");\n}\n";
/// let objects = extract_text(source, None)?.page_objects;
/// let injected = inject_text(source, &objects, true)?;
///
/// let reverted = revert_text(&injected.source, &objects, true)?;
///
/// assert_eq!(reverted.source, source);
/// # Ok(())
/// # }
/// ```
pub fn revert_text(
    source: &str,
    page_objects: &str,
    comments: bool,
) -> Result<Rewrite, SeleniumBaseError> {
    let objects = read_objects(page_objects)?;
    let lexed = lex(source);
    let mut edits = Vec::new();
    let mut restored = BTreeSet::new();
    // The literals restored on each line, and where the last of them ends.
    let mut by_line: BTreeMap<usize, (Vec<&str>, usize)> = BTreeMap::new();
    for index in 0..lexed.tokens.len() {
        let Some(reference) = reference(source, &lexed, index) else {
            continue;
        };
        let Some(literal) = objects.literal(reference.module, reference.name) else {
            continue;
        };
        let end = lexed.tokens[reference.last].end;
        edits.push(Edit {
            start: lexed.tokens[reference.first].start,
            end,
            text: literal.to_owned(),
        });
        restored.insert(index);
        let line = by_line
            .entry(line_end(source, end))
            .or_insert_with(|| (Vec::new(), end));
        line.0.push(literal);
        line.1 = end;
    }
    let changed = edits.len();
    if changed == 0 {
        return Ok(Rewrite {
            source: source.to_owned(),
            changed,
        });
    }
    if comments {
        for (end, (literals, last_restored)) in &by_line {
            let suffix = format!("{COMMENT_START}{}", literals.join(", "));
            let at = text_end(source, *end);
            let Some(start) = at.checked_sub(suffix.len()) else {
                continue;
            };
            if !lexed.spans_line_break(*end)
                && start >= *last_restored
                && source.get(start..at) == Some(suffix.as_str())
            {
                edits.push(Edit {
                    start,
                    end: at,
                    text: String::new(),
                });
            }
        }
    }
    let still_used = (0..lexed.tokens.len()).any(|index| {
        lexed.is_ident(source, index, "page_objects")
            && !restored.contains(&index)
            && !(index > 0 && lexed.is_ident(source, index - 1, "mod"))
    });
    if !still_used {
        edits.extend(module_line(source, &lexed));
    }
    Ok(Rewrite {
        source: apply(source, edits),
        changed,
    })
}

/// A path `[super::]*page_objects::module::NAME` in the source.
struct Reference<'a> {
    /// The index of the first token of the path.
    first: usize,
    /// The index of the token with the name of the constant.
    last: usize,
    module: &'a str,
    name: &'a str,
}

/// The reference that starts at the `page_objects` identifier at `index`, if there is one.
fn reference<'a>(source: &'a str, lexed: &Lexed, index: usize) -> Option<Reference<'a>> {
    if !lexed.is_ident(source, index, "page_objects") {
        return None;
    }
    let colons = [index + 1, index + 2, index + 4, index + 5];
    let words = [index + 3, index + 6];
    if !colons.iter().all(|at| lexed.is_punct(*at, b':'))
        || !words
            .iter()
            .all(|at| lexed.tokens.get(*at).is_some_and(|t| t.kind == Kind::Ident))
    {
        return None;
    }
    let mut first = index;
    while first >= 3
        && lexed.is_ident(source, first - 3, "super")
        && lexed.is_punct(first - 2, b':')
        && lexed.is_punct(first - 1, b':')
    {
        first -= 3;
    }
    if first > 0 && lexed.is_punct(first - 1, b':') {
        // The tail of a longer path, such as `crate::page_objects::...`.
        return None;
    }
    Some(Reference {
        first,
        last: index + 6,
        module: lexed.text(source, index + 3),
        name: lexed.text(source, index + 6),
    })
}

/// The edit that declares `mod page_objects;`, unless the file has the module already: a new
/// line above the first item (and above its docs), after the inner attributes.
fn declare_module(source: &str, lexed: &Lexed) -> Option<Edit> {
    let mut depth = 0_usize;
    for (index, token) in lexed.tokens.iter().enumerate() {
        match token.kind {
            Kind::Punct(b'{') => depth += 1,
            Kind::Punct(b'}') => depth = depth.saturating_sub(1),
            Kind::Ident
                if depth == 0
                    && lexed.is_ident(source, index, "mod")
                    && lexed.is_ident(source, index + 1, "page_objects") =>
            {
                return None;
            }
            _ => {}
        }
    }
    let mut first = 0;
    while lexed.is_punct(first, b'#')
        && lexed.is_punct(first + 1, b'!')
        && lexed.is_punct(first + 2, b'[')
    {
        match lexed.partner[first + 2] {
            Some(close) => first = close + 1,
            None => break,
        }
    }
    let token = lexed.tokens.get(first)?;
    let mut at = line_start(source, token.start);
    while at > 0 {
        let previous = line_start(source, at - 1);
        if source
            .get(previous..at)
            .is_some_and(|line| line.trim_start().starts_with("///"))
        {
            at = previous;
        } else {
            break;
        }
    }
    Some(Edit {
        start: at,
        end: at,
        text: format!("{MODULE_LINE}{}", line_ending(source)),
    })
}

/// The edit that removes the line `mod page_objects;`, if the file has exactly that line.
fn module_line(source: &str, lexed: &Lexed) -> Option<Edit> {
    let mut depth = 0_usize;
    for (index, token) in lexed.tokens.iter().enumerate() {
        match token.kind {
            Kind::Punct(b'{') => depth += 1,
            Kind::Punct(b'}') => depth = depth.saturating_sub(1),
            Kind::Ident
                if depth == 0
                    && lexed.is_ident(source, index, "mod")
                    && lexed.is_ident(source, index + 1, "page_objects")
                    && lexed.is_punct(index + 2, b';') =>
            {
                let start = line_start(source, token.start);
                let end = line_end(source, token.start);
                if source.get(start..text_end(source, end)) != Some(MODULE_LINE) {
                    return None;
                }
                return Some(Edit {
                    start,
                    end: (end + 1).min(source.len()),
                    text: String::new(),
                });
            }
            _ => {}
        }
    }
    None
}

/// Reads the text of a page objects file.
fn read_objects(text: &str) -> Result<PageObjects, SeleniumBaseError> {
    PageObjects::parse(text).map_err(|message| {
        SeleniumBaseError::invalid_config(format!(
            "{PAGE_OBJECTS_FILE} cannot be read: {message}; move it aside or fix it"
        ))
    })
}

/// The error for a file that cannot be read or written.
fn io_error(error: &std::io::Error, action: &str, path: &Path) -> SeleniumBaseError {
    std::io::Error::new(
        error.kind(),
        format!("failed to {action} {}: {error}", path.display()),
    )
    .into()
}

/// Refuses a file that cannot be a test file for these commands.
fn check_test_file(path: &Path) -> Result<(), SeleniumBaseError> {
    if path.extension().is_none_or(|extension| extension != "rs") {
        return Err(SeleniumBaseError::invalid_config(format!(
            "{} is not a Rust file: the name must end in .rs",
            path.display()
        )));
    }
    if path
        .file_name()
        .is_some_and(|name| name == PAGE_OBJECTS_FILE)
    {
        return Err(SeleniumBaseError::invalid_config(format!(
            "{} is the page objects file, not a test file",
            path.display()
        )));
    }
    Ok(())
}

/// Reads a file that has to be there.
fn read(path: &Path) -> Result<String, SeleniumBaseError> {
    fs::read_to_string(path).map_err(|error| io_error(&error, "read", path))
}

/// Reads the page objects file of `test_file`, which has to be there.
fn read_objects_file(test_file: &Path) -> Result<String, SeleniumBaseError> {
    let path = objects_path(test_file);
    match fs::read_to_string(&path) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(SeleniumBaseError::invalid_config(format!(
                "{} does not exist; run `sbrs extract-objects {}` first",
                path.display(),
                test_file.display()
            )))
        }
        Err(error) => Err(io_error(&error, "read", &path)),
    }
}

/// `sbrs extract-objects`: writes the selectors of `test_file` to `page_objects.rs` next to it.
///
/// The file is read as [`extract_text`] describes and written only when a page object is new.
/// `test_file` is not changed.
///
/// # Errors
///
/// [`SeleniumBaseError::InvalidConfig`] when `test_file` is not a `.rs` file, is the page
/// objects file, or the page objects file is not one this module wrote;
/// [`SeleniumBaseError::Io`] when a file cannot be read or written.
pub fn extract_objects(test_file: &Path) -> Result<ExtractReport, SeleniumBaseError> {
    check_test_file(test_file)?;
    let source = read(test_file)?;
    let objects_file = objects_path(test_file);
    let existing = match fs::read_to_string(&objects_file) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(io_error(&error, "read", &objects_file)),
    };
    let extraction = extract_text(&source, existing.as_deref())?;
    if extraction.added > 0 {
        fs::write(&objects_file, &extraction.page_objects)
            .map_err(|error| io_error(&error, "write", &objects_file))?;
    }
    Ok(ExtractReport {
        objects_file,
        found: extraction.found,
        added: extraction.added,
    })
}

/// `sbrs inject-objects`: replaces the selectors of `test_file` by the page objects of the
/// `page_objects.rs` next to it, as [`inject_text`] does.
///
/// The test file is written only when something changes.
///
/// # Errors
///
/// As [`extract_objects`]; also [`SeleniumBaseError::InvalidConfig`] when there is no
/// `page_objects.rs` yet.
pub fn inject_objects(
    test_file: &Path,
    comments: bool,
) -> Result<RewriteReport, SeleniumBaseError> {
    check_test_file(test_file)?;
    let source = read(test_file)?;
    let objects = read_objects_file(test_file)?;
    let rewrite = inject_text(&source, &objects, comments)?;
    if rewrite.changed > 0 {
        fs::write(test_file, &rewrite.source)
            .map_err(|error| io_error(&error, "write", test_file))?;
    }
    Ok(RewriteReport {
        test_file: test_file.to_owned(),
        changed: rewrite.changed,
    })
}

/// `sbrs objectify`: [`extract_objects`], then [`inject_objects`] (which is skipped when the
/// test file has no selector).
///
/// # Errors
///
/// As [`extract_objects`] and [`inject_objects`].
pub fn objectify(
    test_file: &Path,
    comments: bool,
) -> Result<(ExtractReport, RewriteReport), SeleniumBaseError> {
    let extracted = extract_objects(test_file)?;
    if extracted.found == 0 {
        // No page objects file was made, so there is nothing to inject.
        let untouched = RewriteReport {
            test_file: test_file.to_owned(),
            changed: 0,
        };
        return Ok((extracted, untouched));
    }
    let injected = inject_objects(test_file, comments)?;
    Ok((extracted, injected))
}

/// `sbrs revert-objects`: replaces the page objects of `test_file` by the selectors they stand
/// for, as [`revert_text`] does.
///
/// The test file is written only when something changes; `page_objects.rs` stays.
///
/// # Errors
///
/// As [`inject_objects`].
pub fn revert_objects(
    test_file: &Path,
    comments: bool,
) -> Result<RewriteReport, SeleniumBaseError> {
    check_test_file(test_file)?;
    let source = read(test_file)?;
    let objects = read_objects_file(test_file)?;
    let rewrite = revert_text(&source, &objects, comments)?;
    if rewrite.changed > 0 {
        fs::write(test_file, &rewrite.source)
            .map_err(|error| io_error(&error, "write", test_file))?;
    }
    Ok(RewriteReport {
        test_file: test_file.to_owned(),
        changed: rewrite.changed,
    })
}
