//! Safe creation of the files and directories the `sbase mk*` commands generate.
//!
//! A name typed on the command line must never be able to reach outside the
//! directory the command runs in, and a generated file must never silently
//! replace one that already exists. [`RelativeName`] is the validated name and
//! [`Scaffold`] is the writer that enforces both rules:
//!
//! - names are relative, use `/` between components, and each component is made
//!   of ASCII letters, digits, `_`, `-` and `.` only (so there is no `..`, no
//!   drive letter, no backslash, no shell metacharacter and no Windows device
//!   name);
//! - a link inside the target directory that points outside it is refused;
//! - an existing file is an error unless the caller asked to replace it, and a
//!   file is only ever created with `create_new`, so a link planted at the
//!   final component is never followed;
//! - [`Scaffold::write_all`] checks every file before it writes any, so a
//!   conflict leaves the directory untouched.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::cli::scaffold::{RelativeName, Scaffold};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let dir = tempfile::tempdir()?;
//! let scaffold = Scaffold::new(dir.path());
//!
//! let name = RelativeName::new("tests/login.rs")?;
//! scaffold.write_all(&[(name, "// a test\n".to_owned())])?;
//! assert!(dir.path().join("tests").join("login.rs").is_file());
//!
//! // A name that climbs out of the directory is rejected before anything is written.
//! assert!(RelativeName::new("../outside.rs").is_err());
//! assert!(RelativeName::new("/etc/passwd").is_err());
//! # Ok(())
//! # }
//! ```

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use thiserror::Error;

/// The longest a single path component may be, in bytes.
const MAX_COMPONENT_LEN: usize = 128;

/// Why a scaffolding command failed.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ScaffoldError {
    /// The name given on the command line is not an allowed relative name.
    #[error("invalid name {name:?}: {reason}")]
    InvalidName {
        /// The name as it was typed.
        name: String,
        /// What is wrong with it.
        reason: String,
    },
    /// A value other than a name (a URL, a title) is not acceptable.
    #[error("invalid {what} {value:?}: {reason}")]
    InvalidValue {
        /// What kind of value it is, for example `URL`.
        what: &'static str,
        /// The value as it was typed.
        value: String,
        /// What is wrong with it.
        reason: String,
    },
    /// One or more files that would be created already exist.
    #[error("already exists: {}; pass --force to replace", paths(.0))]
    AlreadyExists(Vec<PathBuf>),
    /// The path exists but is a link or a directory, so it will not be replaced.
    #[error("refusing to replace {}: it is not a regular file", .0.display())]
    NotARegularFile(PathBuf),
    /// A directory on the way points (through a link) outside the target directory.
    #[error("refusing to write {}: it resolves outside the target directory", .0.display())]
    EscapesRoot(PathBuf),
    /// Reading or writing the file system failed.
    #[error("{}: {source}", .path.display())]
    Io {
        /// The path being worked on.
        path: PathBuf,
        /// The underlying error.
        source: io::Error,
    },
}

fn paths(list: &[PathBuf]) -> String {
    list.iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

impl ScaffoldError {
    pub(crate) fn invalid_name(name: &str, reason: impl Into<String>) -> Self {
        Self::InvalidName {
            name: name.to_owned(),
            reason: reason.into(),
        }
    }

    pub(crate) fn io(path: &Path, source: io::Error) -> Self {
        Self::Io {
            path: path.to_path_buf(),
            source,
        }
    }
}

/// A validated path that is relative and cannot leave the directory it is joined to.
///
/// See the [module documentation](self) for the rules. The components are kept
/// in a [`PathBuf`], but [`Display`](fmt::Display) always prints them with `/`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelativeName {
    path: PathBuf,
}

impl RelativeName {
    /// Validates `raw` as a relative name.
    ///
    /// A leading `./` and trailing `/` are accepted and dropped.
    ///
    /// # Errors
    ///
    /// Returns [`ScaffoldError::InvalidName`] for an empty name, an absolute path,
    /// a `..` component, an empty component (`a//b`), a character outside
    /// `A-Za-z0-9_-.`, a component that starts with `-` or `.` or ends with `.`,
    /// a component longer than 128 bytes, or a Windows device name such as `NUL`.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::cli::scaffold::RelativeName;
    ///
    /// assert_eq!(RelativeName::new("./tests/login.rs").unwrap().to_string(), "tests/login.rs");
    /// for bad in ["", "..", "a/../b", "/abs", "C:\\x", "a\\b", "a b", "-flag", ".hidden", "con.rs"] {
    ///     assert!(RelativeName::new(bad).is_err(), "{bad}");
    /// }
    /// ```
    pub fn new(raw: &str) -> Result<Self, ScaffoldError> {
        if raw.is_empty() {
            return Err(ScaffoldError::invalid_name(raw, "the name is empty"));
        }
        if raw.starts_with('/') {
            return Err(ScaffoldError::invalid_name(
                raw,
                "absolute paths are not allowed; give a name relative to the current directory",
            ));
        }
        let trimmed = raw.trim_end_matches('/');
        let mut path = PathBuf::new();
        for component in trimmed.split('/') {
            match component {
                "." => continue,
                ".." => {
                    return Err(ScaffoldError::invalid_name(
                        raw,
                        "'..' would leave the current directory",
                    ))
                }
                other => {
                    check_component(raw, other)?;
                    path.push(other);
                }
            }
        }
        if path.as_os_str().is_empty() {
            return Err(ScaffoldError::invalid_name(raw, "the name is empty"));
        }
        Ok(Self { path })
    }

    /// The validated path, relative to the directory it will be joined to.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.path
    }

    /// The last component, for example `login.rs`.
    #[must_use]
    pub fn file_name(&self) -> &str {
        self.path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
    }

    /// The last component without its extension, for example `login`.
    #[must_use]
    pub fn file_stem(&self) -> &str {
        self.path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
    }

    /// The extension of the last component, if it has one.
    #[must_use]
    pub fn extension(&self) -> Option<&str> {
        self.path.extension().and_then(|ext| ext.to_str())
    }

    /// This name with `extension` appended when it has none.
    ///
    /// # Errors
    ///
    /// Returns [`ScaffoldError::InvalidName`] when the name already has a
    /// different extension, so `report.txt` cannot become an HTML file by accident.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::cli::scaffold::RelativeName;
    ///
    /// let name = RelativeName::new("MyTest").unwrap().with_extension_or_default("rs").unwrap();
    /// assert_eq!(name.to_string(), "MyTest.rs");
    /// assert!(RelativeName::new("MyTest.py").unwrap().with_extension_or_default("rs").is_err());
    /// ```
    pub fn with_extension_or_default(mut self, extension: &str) -> Result<Self, ScaffoldError> {
        match self.extension() {
            Some(existing) if existing.eq_ignore_ascii_case(extension) => Ok(self),
            Some(existing) => Err(ScaffoldError::invalid_name(
                &self.to_string(),
                format!("expected a .{extension} file, not .{existing}"),
            )),
            None => {
                let name = format!("{}.{extension}", self.file_name());
                self.path.set_file_name(name);
                Ok(self)
            }
        }
    }

    /// This name with `child` below it.
    ///
    /// `child` is a trusted constant of the caller, never user input.
    #[must_use]
    pub(crate) fn join(&self, child: &str) -> Self {
        debug_assert!(
            Self::new(child).is_ok(),
            "{child:?} is not a valid relative name"
        );
        Self {
            path: self.path.join(child),
        }
    }
}

impl fmt::Display for RelativeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = self.path.iter();
        if let Some(first) = parts.next() {
            write!(f, "{}", first.to_string_lossy())?;
        }
        for part in parts {
            write!(f, "/{}", part.to_string_lossy())?;
        }
        Ok(())
    }
}

fn check_component(raw: &str, component: &str) -> Result<(), ScaffoldError> {
    if component.is_empty() {
        return Err(ScaffoldError::invalid_name(
            raw,
            "it has an empty path component ('//')",
        ));
    }
    if component.len() > MAX_COMPONENT_LEN {
        return Err(ScaffoldError::invalid_name(
            raw,
            format!("a path component is longer than {MAX_COMPONENT_LEN} bytes"),
        ));
    }
    if let Some(bad) = component
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')))
    {
        return Err(ScaffoldError::invalid_name(
            raw,
            format!(
                "{bad:?} is not allowed; use letters, digits, '_', '-' and '.' with '/' between folders"
            ),
        ));
    }
    if component.starts_with('-') {
        return Err(ScaffoldError::invalid_name(
            raw,
            "a name must not start with '-' (tools would read it as an option)",
        ));
    }
    if component.starts_with('.') {
        return Err(ScaffoldError::invalid_name(
            raw,
            "a name must not start with '.'",
        ));
    }
    if component.ends_with('.') {
        return Err(ScaffoldError::invalid_name(
            raw,
            "a name must not end with '.' (Windows drops the dot)",
        ));
    }
    if is_windows_device_name(component) {
        return Err(ScaffoldError::invalid_name(
            raw,
            "this is a reserved device name on Windows",
        ));
    }
    Ok(())
}

/// `CON`, `PRN`, `AUX`, `NUL`, `COM1`-`COM9` and `LPT1`-`LPT9`, with any extension.
fn is_windows_device_name(component: &str) -> bool {
    let stem = component.split('.').next().unwrap_or_default();
    let stem = stem.to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|digit| matches!(digit.as_bytes(), [b'1'..=b'9']))
}

/// A file to create: where, and what it contains.
pub type GeneratedFile = (RelativeName, String);

/// Writes generated files below a root directory without leaving it or
/// overwriting anything by accident. See the [module documentation](self).
#[derive(Clone, Debug)]
pub struct Scaffold {
    root: PathBuf,
    replace: bool,
}

impl Scaffold {
    /// A writer that creates files below `root`, which must already exist.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            replace: false,
        }
    }

    /// Whether an existing regular file may be replaced. The default is `false`.
    #[must_use]
    pub fn replacing(mut self, replace: bool) -> Self {
        self.replace = replace;
        self
    }

    /// The directory files are created in.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Creates every file (and the folders above it), or none of them.
    ///
    /// All files are checked first: the first conflict aborts before anything is
    /// written. An I/O failure while writing can still leave the files written
    /// before it. Returns the created paths relative to the root.
    ///
    /// # Errors
    ///
    /// - [`ScaffoldError::AlreadyExists`] if a file exists and replacing is off;
    /// - [`ScaffoldError::NotARegularFile`] if a link or folder is in the way;
    /// - [`ScaffoldError::EscapesRoot`] if a folder on the way is a link that
    ///   leads outside the root;
    /// - [`ScaffoldError::Io`] for any other file-system failure.
    pub fn write_all(&self, files: &[GeneratedFile]) -> Result<Vec<PathBuf>, ScaffoldError> {
        let root =
            fs::canonicalize(&self.root).map_err(|source| ScaffoldError::io(&self.root, source))?;

        let mut conflicts = Vec::new();
        for (name, _) in files {
            let target = root.join(name.as_path());
            check_parent(&root, &target)?;
            match fs::symlink_metadata(&target) {
                Ok(meta) if self.replace && meta.is_file() => {}
                Ok(meta) if self.replace && !meta.is_file() => {
                    return Err(ScaffoldError::NotARegularFile(name.as_path().to_path_buf()))
                }
                Ok(_) => conflicts.push(name.as_path().to_path_buf()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(source) => return Err(ScaffoldError::io(&target, source)),
            }
        }
        if !conflicts.is_empty() {
            return Err(ScaffoldError::AlreadyExists(conflicts));
        }

        let mut created = Vec::with_capacity(files.len());
        for (name, contents) in files {
            let target = root.join(name.as_path());
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|source| ScaffoldError::io(parent, source))?;
            }
            // The folders exist now, so a link among them can be followed and judged.
            check_parent(&root, &target)?;
            if self.replace && fs::symlink_metadata(&target).is_ok_and(|meta| meta.is_file()) {
                fs::remove_file(&target).map_err(|source| ScaffoldError::io(&target, source))?;
            }
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
                .map_err(|source| match source.kind() {
                    io::ErrorKind::AlreadyExists => {
                        ScaffoldError::AlreadyExists(vec![name.as_path().to_path_buf()])
                    }
                    _ => ScaffoldError::io(&target, source),
                })?;
            file.write_all(contents.as_bytes())
                .map_err(|source| ScaffoldError::io(&target, source))?;
            created.push(name.as_path().to_path_buf());
        }
        Ok(created)
    }
}

/// Fails if the closest existing folder above `target` resolves outside `root`.
fn check_parent(root: &Path, target: &Path) -> Result<(), ScaffoldError> {
    let mut probe = target.parent();
    while let Some(dir) = probe {
        match fs::symlink_metadata(dir) {
            Ok(_) => {
                let resolved =
                    fs::canonicalize(dir).map_err(|source| ScaffoldError::io(dir, source))?;
                return if resolved.starts_with(root) {
                    Ok(())
                } else {
                    Err(ScaffoldError::EscapesRoot(target.to_path_buf()))
                };
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => probe = dir.parent(),
            Err(source) => return Err(ScaffoldError::io(dir, source)),
        }
    }
    Ok(())
}

/// Turns a file stem into a valid Rust function name.
///
/// `MyTest` becomes `my_test`, `login-flow` becomes `login_flow`, and a stem that
/// is empty, starts with a digit or is a keyword gets a `test_` prefix.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::cli::scaffold::rust_fn_name;
///
/// assert_eq!(rust_fn_name("MyTest"), "my_test");
/// assert_eq!(rust_fn_name("login-flow.v2"), "login_flow_v2");
/// assert_eq!(rust_fn_name("1st"), "test_1st");
/// assert_eq!(rust_fn_name("match"), "test_match");
/// assert_eq!(rust_fn_name(""), "my_first_test");
/// ```
#[must_use]
pub fn rust_fn_name(stem: &str) -> String {
    let mut out = String::with_capacity(stem.len() + 4);
    let mut previous_lower = false;
    for c in stem.chars() {
        if c.is_ascii_alphanumeric() {
            if c.is_ascii_uppercase() && previous_lower {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
            previous_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
        } else if !out.ends_with('_') && !out.is_empty() {
            out.push('_');
            previous_lower = false;
        }
    }
    let name = out.trim_matches('_').to_owned();
    if name.is_empty() {
        return "my_first_test".to_owned();
    }
    if name.starts_with(|c: char| c.is_ascii_digit()) || is_rust_keyword(&name) {
        return format!("test_{name}");
    }
    name
}

fn is_rust_keyword(word: &str) -> bool {
    matches!(
        word,
        "as" | "break"
            | "const"
            | "continue"
            | "crate"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "async"
            | "await"
            | "dyn"
            | "abstract"
            | "become"
            | "box"
            | "do"
            | "final"
            | "macro"
            | "override"
            | "priv"
            | "typeof"
            | "unsized"
            | "virtual"
            | "yield"
            | "try"
            | "gen"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(raw: &str) -> RelativeName {
        RelativeName::new(raw).unwrap_or_else(|e| panic!("{raw:?} should be valid: {e}"))
    }

    #[test]
    fn accepts_plain_relative_names() {
        for good in [
            "a",
            "a.rs",
            "tests/login.rs",
            "my_tests/sub-dir/x.v2.rs",
            "A1",
        ] {
            assert_eq!(name(good).to_string(), good);
        }
    }

    #[test]
    fn drops_dot_components_and_trailing_slashes() {
        assert_eq!(name("./a/./b/").to_string(), "a/b");
    }

    #[test]
    fn rejects_every_way_out_of_the_directory() {
        for bad in [
            "..",
            "../x",
            "a/../../x",
            "a/..",
            "/abs",
            "//server/share",
            "C:\\x",
            "C:/x",
            "a\\b",
            "\\\\server\\share",
            "~/x",
            "a:b",
        ] {
            assert!(RelativeName::new(bad).is_err(), "{bad:?} must be rejected");
        }
    }

    #[test]
    fn rejects_awkward_names() {
        for bad in [
            "", ".", "./", "a//b", "a b", "a\nb", "a\0b", "-rf", "a/-x", ".hidden", "a/.env",
            "name.", "a$b", "a;b", "a`b", "a*b", "é.rs", "con", "NUL.txt", "com1.rs", "lpt9",
        ] {
            assert!(RelativeName::new(bad).is_err(), "{bad:?} must be rejected");
        }
        assert!(RelativeName::new(&"x".repeat(MAX_COMPONENT_LEN + 1)).is_err());
        assert!(RelativeName::new(&"x".repeat(MAX_COMPONENT_LEN)).is_ok());
        // `com0` and `com10` are not device names.
        assert!(RelativeName::new("com0").is_ok());
        assert!(RelativeName::new("com10").is_ok());
    }

    #[test]
    fn error_messages_name_the_problem() {
        let message = RelativeName::new("../x").unwrap_err().to_string();
        assert!(message.contains("'..'"), "{message}");
        let message = RelativeName::new("/x").unwrap_err().to_string();
        assert!(message.contains("absolute"), "{message}");
        let message = RelativeName::new("a b").unwrap_err().to_string();
        assert!(message.contains("' '"), "{message}");
    }

    #[test]
    fn extension_is_added_or_checked() {
        assert_eq!(
            name("t")
                .with_extension_or_default("rs")
                .unwrap()
                .to_string(),
            "t.rs"
        );
        assert_eq!(
            name("d/t")
                .with_extension_or_default("rs")
                .unwrap()
                .to_string(),
            "d/t.rs"
        );
        assert_eq!(
            name("t.RS")
                .with_extension_or_default("rs")
                .unwrap()
                .to_string(),
            "t.RS"
        );
        assert!(name("t.py").with_extension_or_default("rs").is_err());
    }

    #[test]
    fn stems_become_function_names() {
        assert_eq!(rust_fn_name("MyFirstTest"), "my_first_test");
        assert_eq!(rust_fn_name("HTTPServer"), "httpserver");
        assert_eq!(rust_fn_name("a--b__c"), "a_b_c");
        assert_eq!(rust_fn_name("--"), "my_first_test");
        assert_eq!(rust_fn_name("9"), "test_9");
        assert_eq!(rust_fn_name("self"), "test_self");
        assert_eq!(rust_fn_name("fn"), "test_fn");
    }

    #[test]
    fn writes_nested_files_and_reports_them_relative() {
        let dir = tempfile::tempdir().unwrap();
        let created = Scaffold::new(dir.path())
            .write_all(&[
                (name("a/b/one.txt"), "1".to_owned()),
                (name("two.txt"), "2".to_owned()),
            ])
            .unwrap();
        assert_eq!(
            created,
            [PathBuf::from("a/b/one.txt"), PathBuf::from("two.txt")]
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("a/b/one.txt")).unwrap(),
            "1"
        );
        assert_eq!(fs::read_to_string(dir.path().join("two.txt")).unwrap(), "2");
    }

    #[test]
    fn an_existing_file_is_kept_and_nothing_else_is_written() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("keep.txt"), "mine").unwrap();

        let error = Scaffold::new(dir.path())
            .write_all(&[
                (name("new.txt"), "n".to_owned()),
                (name("keep.txt"), "overwritten".to_owned()),
            ])
            .unwrap_err();

        assert!(
            matches!(&error, ScaffoldError::AlreadyExists(p) if p == &[PathBuf::from("keep.txt")])
        );
        assert!(error.to_string().contains("--force"), "{error}");
        assert_eq!(
            fs::read_to_string(dir.path().join("keep.txt")).unwrap(),
            "mine"
        );
        assert!(!dir.path().join("new.txt").exists(), "all-or-nothing");
    }

    #[test]
    fn replacing_overwrites_a_regular_file_only_when_asked() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("f.txt"), "old content that is longer").unwrap();

        Scaffold::new(dir.path())
            .replacing(true)
            .write_all(&[(name("f.txt"), "new".to_owned())])
            .unwrap();

        assert_eq!(fs::read_to_string(dir.path().join("f.txt")).unwrap(), "new");
    }

    #[test]
    fn a_directory_is_never_replaced() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("d")).unwrap();

        let error = Scaffold::new(dir.path())
            .replacing(true)
            .write_all(&[(name("d"), "x".to_owned())])
            .unwrap_err();

        assert!(
            matches!(error, ScaffoldError::NotARegularFile(_)),
            "{error}"
        );
    }

    #[test]
    fn a_missing_root_is_an_error_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let error = Scaffold::new(dir.path().join("missing"))
            .write_all(&[(name("f"), String::new())])
            .unwrap_err();
        assert!(matches!(error, ScaffoldError::Io { .. }), "{error}");
    }

    #[cfg(unix)]
    mod links {
        use super::*;
        use std::os::unix::fs::symlink;

        #[test]
        fn a_folder_link_pointing_outside_is_refused() {
            let outside = tempfile::tempdir().unwrap();
            let dir = tempfile::tempdir().unwrap();
            symlink(outside.path(), dir.path().join("escape")).unwrap();

            let error = Scaffold::new(dir.path())
                .write_all(&[(name("escape/payload.txt"), "x".to_owned())])
                .unwrap_err();

            assert!(matches!(error, ScaffoldError::EscapesRoot(_)), "{error}");
            assert!(!outside.path().join("payload.txt").exists());
        }

        #[test]
        fn a_folder_link_pointing_inside_is_fine() {
            let dir = tempfile::tempdir().unwrap();
            fs::create_dir(dir.path().join("real")).unwrap();
            symlink(dir.path().join("real"), dir.path().join("alias")).unwrap();

            Scaffold::new(dir.path())
                .write_all(&[(name("alias/f.txt"), "x".to_owned())])
                .unwrap();

            assert!(dir.path().join("real/f.txt").is_file());
        }

        #[test]
        fn a_planted_file_link_is_not_followed_even_when_replacing() {
            let outside = tempfile::tempdir().unwrap();
            let victim = outside.path().join("victim.txt");
            fs::write(&victim, "precious").unwrap();
            let dir = tempfile::tempdir().unwrap();
            symlink(&victim, dir.path().join("f.txt")).unwrap();

            let refused = Scaffold::new(dir.path())
                .write_all(&[(name("f.txt"), "x".to_owned())])
                .unwrap_err();
            assert!(
                matches!(refused, ScaffoldError::AlreadyExists(_)),
                "{refused}"
            );

            let refused = Scaffold::new(dir.path())
                .replacing(true)
                .write_all(&[(name("f.txt"), "x".to_owned())])
                .unwrap_err();
            assert!(
                matches!(refused, ScaffoldError::NotARegularFile(_)),
                "{refused}"
            );

            assert_eq!(fs::read_to_string(&victim).unwrap(), "precious");
        }

        #[test]
        fn a_dangling_file_link_is_refused_too() {
            let outside = tempfile::tempdir().unwrap();
            let dir = tempfile::tempdir().unwrap();
            let target = outside.path().join("not-yet.txt");
            symlink(&target, dir.path().join("f.txt")).unwrap();

            let error = Scaffold::new(dir.path())
                .write_all(&[(name("f.txt"), "x".to_owned())])
                .unwrap_err();

            assert!(matches!(error, ScaffoldError::AlreadyExists(_)), "{error}");
            assert!(!target.exists());
        }
    }
}
