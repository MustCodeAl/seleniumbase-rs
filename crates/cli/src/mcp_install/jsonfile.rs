//! Editing the JSON config files of the clients: one key of the top-level object changes and
//! the rest of the file keeps its keys, its order, its indent and its final newline.
//!
//! A file with comments (JSONC) or any other text `serde_json` cannot read is not rewritten:
//! a rewrite would lose the comments. The caller is told what to add by hand instead.

use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde_json::{Map, Value};

use super::entry::{decide, Decision, Entry, Existing, LaunchSpec};
use super::fsx;
use super::jsonedit;
use super::report::{EntryReport, Preview, Problem, Status};

/// Where a client keeps its servers in the file and how one entry looks.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    /// The top-level key that holds the servers by name.
    pub key: &'static str,
    /// The shape of an entry.
    pub kind: LayoutKind,
}

/// The shapes of an entry.
#[derive(Clone, Copy, Debug)]
pub enum LayoutKind {
    /// `{ "command": ..., "args": [...], "env": {...} }`, with `"type": "stdio"` when `typed`.
    Plain {
        /// Whether the entry carries `"type": "stdio"`.
        typed: bool,
    },
    /// opencode: `{ "type": "local", "command": [program, args...], "environment": {...} }`.
    Opencode,
}

impl Layout {
    /// The entry that starts `spec`.
    pub fn entry(&self, spec: &LaunchSpec) -> Value {
        let mut object = Map::new();
        match self.kind {
            LayoutKind::Plain { typed } => {
                if typed {
                    object.insert("type".to_owned(), "stdio".into());
                }
                object.insert("command".to_owned(), spec.command.clone().into());
                object.insert("args".to_owned(), strings_value(&spec.args));
                if !spec.env.is_empty() {
                    object.insert("env".to_owned(), map_value(&spec.env));
                }
            }
            LayoutKind::Opencode => {
                object.insert("type".to_owned(), "local".into());
                let mut command = vec![spec.command.clone()];
                command.extend(spec.args.iter().cloned());
                object.insert("command".to_owned(), strings_value(&command));
                if !spec.env.is_empty() {
                    object.insert("environment".to_owned(), map_value(&spec.env));
                }
                object.insert("enabled".to_owned(), true.into());
            }
        }
        Value::Object(object)
    }

    /// The process `entry` starts, or `None` when it is not a local stdio server in this
    /// layout, or is switched off.
    pub fn parse(&self, entry: &Value) -> Option<LaunchSpec> {
        let object = entry.as_object()?;
        match self.kind {
            LayoutKind::Plain { .. } => {
                if object
                    .get("type")
                    .is_some_and(|kind| kind.as_str() != Some("stdio"))
                {
                    return None;
                }
                Some(LaunchSpec {
                    command: object.get("command")?.as_str()?.to_owned(),
                    args: optional(object.get("args"), strings)?,
                    env: optional(object.get("env"), string_map)?,
                })
            }
            LayoutKind::Opencode => {
                if object
                    .get("type")
                    .is_some_and(|kind| kind.as_str() != Some("local"))
                {
                    return None;
                }
                if object
                    .get("enabled")
                    .is_some_and(|on| on.as_bool() == Some(false))
                {
                    return None;
                }
                let mut command = strings(object.get("command")?)?;
                if command.is_empty() {
                    return None;
                }
                let program = command.remove(0);
                Some(LaunchSpec {
                    command: program,
                    args: command,
                    env: optional(object.get("environment"), string_map)?,
                })
            }
        }
    }
}

fn strings_value(items: &[String]) -> Value {
    Value::Array(items.iter().cloned().map(Value::String).collect())
}

fn map_value(map: &BTreeMap<String, String>) -> Value {
    Value::Object(
        map.iter()
            .map(|(key, value)| (key.clone(), Value::String(value.clone())))
            .collect(),
    )
}

fn strings(value: &Value) -> Option<Vec<String>> {
    value
        .as_array()?
        .iter()
        .map(|item| item.as_str().map(str::to_owned))
        .collect()
}

fn string_map(value: &Value) -> Option<BTreeMap<String, String>> {
    value
        .as_object()?
        .iter()
        .map(|(key, item)| item.as_str().map(|text| (key.clone(), text.to_owned())))
        .collect()
}

/// `read(value)` for a key that is there, the default for one that is not, and `None` for a
/// key whose value `read` cannot make sense of.
fn optional<T: Default>(value: Option<&Value>, read: fn(&Value) -> Option<T>) -> Option<T> {
    match value {
        None => Some(T::default()),
        Some(value) => read(value),
    }
}

/// Why a file could not be loaded.
#[derive(Debug)]
pub enum LoadError {
    /// The file could not be read.
    Io(String),
    /// The file is there but is not plain JSON this program can rewrite; the text says why.
    Unreadable(String),
}

/// A config file: its parsed content, and its text for the edits.
#[derive(Debug)]
pub struct Doc {
    root: Map<String, Value>,
    /// The text without its byte order mark; empty for a file that is not there.
    body: String,
    /// One level of the file's indent.
    unit: String,
    bom: bool,
}

impl Doc {
    fn empty() -> Self {
        Self {
            root: Map::new(),
            body: String::new(),
            unit: "  ".to_owned(),
            bom: false,
        }
    }

    /// Reads `path`. A file that does not exist, or holds only white space, is an empty object.
    ///
    /// # Errors
    ///
    /// [`LoadError::Io`] when the file cannot be read; [`LoadError::Unreadable`] when it has
    /// comments, is not valid JSON or does not hold an object.
    pub fn load(path: &Path) -> Result<Self, LoadError> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Self::empty()),
            Err(e) => {
                return Err(LoadError::Io(format!(
                    "cannot read {}: {e}",
                    path.display()
                )))
            }
        };
        let text = String::from_utf8(bytes)
            .map_err(|_| LoadError::Unreadable("is not valid UTF-8".to_owned()))?;
        let (bom, body) = match text.strip_prefix('\u{feff}') {
            Some(rest) => (true, rest),
            None => (false, text.as_str()),
        };
        if body.trim().is_empty() {
            return Ok(Self {
                bom,
                ..Self::empty()
            });
        }
        if fsx::has_comments(body) {
            return Err(LoadError::Unreadable("has comments (JSONC)".to_owned()));
        }
        let value: Value = serde_json::from_str(body).map_err(|e| {
            LoadError::Unreadable(format!(
                "is not valid JSON ({e}); a trailing comma does that"
            ))
        })?;
        let Value::Object(root) = value else {
            return Err(LoadError::Unreadable(
                "does not hold a JSON object".to_owned(),
            ));
        };
        Ok(Self {
            root,
            body: body.to_owned(),
            unit: fsx::detect_indent(body),
            bom,
        })
    }

    /// The entries under the layout's key.
    ///
    /// # Errors
    ///
    /// [`LoadError::Unreadable`] when the key holds something other than an object.
    pub fn existing(&self, layout: &Layout) -> Result<Existing, LoadError> {
        match self.root.get(layout.key) {
            None => Ok(Existing::new()),
            Some(Value::Object(servers)) => Ok(servers
                .iter()
                .map(|(name, entry)| (name.clone(), layout.parse(entry)))
                .collect()),
            Some(_) => Err(LoadError::Unreadable(format!(
                "has `{}` set to something other than an object",
                layout.key
            ))),
        }
    }

    /// The file's text with the entries `write` added or replaced and the entries called
    /// `remove` taken out. Only those members of the text change; every other byte stays.
    ///
    /// `None` when the edit cannot be made, or when the result does not hold exactly the
    /// content it should (every other member as it was, the written entries, the removed ones
    /// gone); the file is then left alone.
    pub fn updated(&self, layout: &Layout, write: &[&Entry], remove: &[&str]) -> Option<String> {
        let body = if self.body.trim().is_empty() {
            let servers: Map<String, Value> = write
                .iter()
                .map(|entry| (entry.name.clone(), layout.entry(&entry.spec)))
                .collect();
            let mut root = Map::new();
            root.insert(layout.key.to_owned(), Value::Object(servers));
            let mut text = jsonedit::pretty(&Value::Object(root), &self.unit);
            text.push('\n');
            text
        } else {
            let mut body = self.body.clone();
            for entry in write {
                let value = layout.entry(&entry.spec);
                body = jsonedit::upsert(&body, layout.key, &entry.name, &value, &self.unit)?;
            }
            for name in remove {
                body = jsonedit::remove(&body, layout.key, name)?;
            }
            body
        };
        if !self.holds_exactly(&body, layout, write, remove) {
            return None;
        }
        Some(if self.bom {
            format!("\u{feff}{body}")
        } else {
            body
        })
    }

    /// Whether `body` is this document with the entries written and removed, and nothing else
    /// changed.
    fn holds_exactly(
        &self,
        body: &str,
        layout: &Layout,
        write: &[&Entry],
        remove: &[&str],
    ) -> bool {
        let Ok(Value::Object(mut after)) = serde_json::from_str::<Value>(body) else {
            return false;
        };
        let mut before = self.root.clone();
        let mut expected = match before.remove(layout.key) {
            Some(Value::Object(servers)) => servers,
            None => Map::new(),
            Some(_) => return false,
        };
        let Some(Value::Object(found)) = after.remove(layout.key) else {
            return false;
        };
        for entry in write {
            expected.insert(entry.name.clone(), layout.entry(&entry.spec));
        }
        for name in remove {
            expected.remove(*name);
        }
        before == after && expected == found
    }
}

/// The entries `layout` holds in the file at `path`, for a client whose file is only read.
///
/// # Errors
///
/// A message saying why the file could not be read.
pub fn read_existing(path: &Path, layout: &Layout) -> Result<Existing, String> {
    let describe = |error: LoadError| match error {
        LoadError::Io(message) => message,
        LoadError::Unreadable(why) => format!("{} {why}", path.display()),
    };
    Doc::load(path)
        .and_then(|doc| doc.existing(layout))
        .map_err(describe)
}

/// One change to the servers of a file.
#[derive(Clone, Copy, Debug)]
pub enum Op<'a> {
    /// Add the entry, or replace a different one of that name when `force` is set.
    Upsert {
        /// The entry.
        entry: &'a Entry,
        /// Whether a different entry of that name may be replaced.
        force: bool,
    },
    /// Remove the entry called `name`.
    Remove {
        /// The entry's name.
        name: &'a str,
    },
}

/// What [`apply`] did.
#[derive(Debug)]
pub struct Applied {
    /// One report for each operation, in order.
    pub reports: Vec<EntryReport>,
    /// What a dry run shows.
    pub preview: Vec<Preview>,
}

/// Applies `ops` to the file at `path` in one read and one write, or shows them when
/// `dry_run` is set. A file that needs no change is neither backed up nor written.
///
/// # Errors
///
/// [`Problem::Refused`] when the file has comments or is not JSON (the text to add by hand
/// comes with it); [`Problem::Failed`] when the file cannot be read or written.
pub fn apply(
    path: &Path,
    layout: &Layout,
    ops: &[Op<'_>],
    dry_run: bool,
) -> Result<Applied, Problem> {
    let doc = Doc::load(path).map_err(|e| problem(e, path, layout, ops))?;
    let existing = doc
        .existing(layout)
        .map_err(|e| problem(e, path, layout, ops))?;

    let mut reports = Vec::new();
    let mut write: Vec<&Entry> = Vec::new();
    let mut remove: Vec<&str> = Vec::new();
    for op in ops {
        match *op {
            Op::Upsert { entry, force } => {
                let status = match decide(existing.get(&entry.name), &entry.spec, force) {
                    Decision::Add => Status::Added,
                    Decision::Replace => Status::Replaced,
                    Decision::Unchanged => Status::Unchanged,
                    Decision::Conflict => Status::Conflict,
                };
                if matches!(status, Status::Added | Status::Replaced) {
                    write.push(entry);
                }
                reports.push(EntryReport {
                    name: entry.name.clone(),
                    status,
                });
            }
            Op::Remove { name } => {
                let status = if existing.contains_key(name) {
                    remove.push(name);
                    Status::Removed
                } else {
                    Status::Absent
                };
                reports.push(EntryReport {
                    name: name.to_owned(),
                    status,
                });
            }
        }
    }
    if write.is_empty() && remove.is_empty() {
        return Ok(Applied {
            reports,
            preview: Vec::new(),
        });
    }
    if dry_run {
        let mut preview = Vec::new();
        if !write.is_empty() {
            preview.push(Preview::Snippet {
                heading: format!("would write to {}", path.display()),
                text: snippet(layout, &write),
            });
        }
        if !remove.is_empty() {
            preview.push(Preview::Snippet {
                heading: format!("would remove from {}", path.display()),
                text: remove
                    .iter()
                    .map(|name| format!("{}: {name}", layout.key))
                    .collect::<Vec<_>>()
                    .join("\n"),
            });
        }
        return Ok(Applied { reports, preview });
    }
    let text = doc.updated(layout, &write, &remove).ok_or_else(|| {
        Problem::Failed(format!(
            "could not edit {} with certainty, so it was left as it was",
            path.display()
        ))
    })?;
    fsx::replace(path, text.as_bytes())
        .map_err(|e| Problem::Failed(format!("cannot write {}: {e}", path.display())))?;
    Ok(Applied {
        reports,
        preview: Vec::new(),
    })
}

/// The JSON to merge into a file by hand: the layout's key holding `entries`.
pub fn snippet(layout: &Layout, entries: &[&Entry]) -> String {
    let servers: Map<String, Value> = entries
        .iter()
        .map(|entry| (entry.name.clone(), layout.entry(&entry.spec)))
        .collect();
    let mut root = Map::new();
    root.insert(layout.key.to_owned(), Value::Object(servers));
    jsonedit::pretty(&Value::Object(root), "  ")
}

fn problem(error: LoadError, path: &Path, layout: &Layout, ops: &[Op<'_>]) -> Problem {
    match error {
        LoadError::Io(message) => Problem::Failed(message),
        LoadError::Unreadable(why) => {
            let adds: Vec<&Entry> = ops
                .iter()
                .filter_map(|op| match *op {
                    Op::Upsert { entry, .. } => Some(entry),
                    Op::Remove { .. } => None,
                })
                .collect();
            let removes: Vec<&str> = ops
                .iter()
                .filter_map(|op| match *op {
                    Op::Remove { name } => Some(name),
                    Op::Upsert { .. } => None,
                })
                .collect();
            let by_hand = if adds.is_empty() {
                format!(
                    "remove {} from `{}` in {}",
                    removes.join(", "),
                    layout.key,
                    path.display()
                )
            } else {
                format!(
                    "add this to {} (merge it into what is there):\n{}",
                    path.display(),
                    snippet(layout, &adds)
                )
            };
            Problem::Refused {
                reason: format!("{} {why}, so sbrs-mcp does not rewrite it", path.display()),
                by_hand: Some(by_hand),
            }
        }
    }
}
