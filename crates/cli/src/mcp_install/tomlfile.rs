//! Editing Codex's `config.toml` with `toml_edit`, which keeps its comments, its order and
//! every table that is not `mcp_servers.NAME`.
//!
//! The format is the one in Codex's documentation
//! (<https://developers.openai.com/codex/mcp>): a table `[mcp_servers.NAME]` with `command`
//! and `args`, and `[mcp_servers.NAME.env]` for the environment.

use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use toml_edit::{value, Array, DocumentMut, Item, Table};

use super::entry::{decide, Decision, Entry, Existing, LaunchSpec};
use super::fsx;
use super::jsonfile::Applied;
use super::report::{EntryReport, Preview, Problem, Status};

const KEY: &str = "mcp_servers";

/// Reads `path`: an empty document when the file does not exist.
fn load(path: &Path) -> Result<DocumentMut, Problem> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(DocumentMut::new()),
        Err(e) => {
            return Err(Problem::Failed(format!(
                "cannot read {}: {e}",
                path.display()
            )))
        }
    };
    text.parse::<DocumentMut>().map_err(|e| Problem::Refused {
        reason: format!(
            "{} is not valid TOML ({}), so sbrs-mcp does not rewrite it",
            path.display(),
            e.message()
        ),
        by_hand: None,
    })
}

fn spec_of(item: &Item) -> Option<LaunchSpec> {
    let table = item.as_table_like()?;
    if table.get("url").is_some() {
        return None;
    }
    let command = table.get("command")?.as_str()?.to_owned();
    let args = match table.get("args") {
        None => Vec::new(),
        Some(args) => args
            .as_array()?
            .iter()
            .map(|arg| arg.as_str().map(str::to_owned))
            .collect::<Option<Vec<_>>>()?,
    };
    let env = match table.get("env") {
        None => BTreeMap::new(),
        Some(env) => env
            .as_table_like()?
            .iter()
            .map(|(key, item)| item.as_str().map(|text| (key.to_owned(), text.to_owned())))
            .collect::<Option<BTreeMap<_, _>>>()?,
    };
    Some(LaunchSpec { command, args, env })
}

fn existing_of(doc: &DocumentMut) -> Result<Existing, String> {
    let Some(servers) = doc.get(KEY) else {
        return Ok(Existing::new());
    };
    let table = servers
        .as_table_like()
        .ok_or_else(|| format!("`{KEY}` is not a table"))?;
    Ok(table
        .iter()
        .map(|(name, item)| (name.to_owned(), spec_of(item)))
        .collect())
}

/// The entries in the file at `path`, for deciding what to change.
///
/// # Errors
///
/// A message saying why the file could not be read.
pub fn read_existing(path: &Path) -> Result<Existing, String> {
    let doc = load(path).map_err(|problem| match problem {
        Problem::Failed(message) => message,
        Problem::Refused { reason, .. } => reason,
    })?;
    existing_of(&doc).map_err(|why| format!("{}: {why}", path.display()))
}

fn item_of(spec: &LaunchSpec) -> Item {
    let mut table = Table::new();
    table.insert("command", value(spec.command.as_str()));
    let mut args = Array::new();
    for arg in &spec.args {
        args.push(arg.as_str());
    }
    table.insert("args", value(args));
    if !spec.env.is_empty() {
        let mut env = Table::new();
        for (key, text) in &spec.env {
            env.insert(key, value(text.as_str()));
        }
        table.insert("env", Item::Table(env));
    }
    Item::Table(table)
}

/// The TOML that registers `entries`, for a dry run and for a file to edit by hand.
pub fn snippet(entries: &[&Entry]) -> String {
    let mut doc = DocumentMut::new();
    let mut servers = Table::new();
    servers.set_implicit(true);
    for entry in entries {
        servers.insert(&entry.name, item_of(&entry.spec));
    }
    doc.insert(KEY, Item::Table(servers));
    doc.to_string()
}

/// One change to the servers of the file.
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

/// Applies `ops` to the file at `path` in one read and one write, or shows them when
/// `dry_run` is set. A file that needs no change is neither backed up nor written.
///
/// # Errors
///
/// [`Problem::Refused`] when the file is not TOML; [`Problem::Failed`] when it cannot be read
/// or written.
pub fn apply(path: &Path, ops: &[Op<'_>], dry_run: bool) -> Result<Applied, Problem> {
    let mut doc = load(path)?;
    let existing = existing_of(&doc).map_err(|why| {
        Problem::refused(format!(
            "{}: {why}, so sbrs-mcp does not rewrite it",
            path.display()
        ))
    })?;
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
                text: snippet(&write),
            });
        }
        if !remove.is_empty() {
            preview.push(Preview::Snippet {
                heading: format!("would remove from {}", path.display()),
                text: remove
                    .iter()
                    .map(|name| format!("{KEY}.{name}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
            });
        }
        return Ok(Applied { reports, preview });
    }
    // A blank line before a table added to a file that has content.
    let mut separate = !doc.to_string().trim().is_empty();
    for entry in &write {
        let mut item = item_of(&entry.spec);
        if separate {
            if let Some(table) = item.as_table_mut() {
                table.decor_mut().set_prefix("\n");
            }
        }
        separate = true;
        servers_mut(&mut doc)?.insert(&entry.name, item);
    }
    for name in &remove {
        servers_mut(&mut doc)?.remove(name);
    }
    fsx::replace(path, doc.to_string().as_bytes())
        .map_err(|e| Problem::Failed(format!("cannot write {}: {e}", path.display())))?;
    Ok(Applied {
        reports,
        preview: Vec::new(),
    })
}

/// The `mcp_servers` table of `doc`, made when it is not there.
fn servers_mut(doc: &mut DocumentMut) -> Result<&mut dyn toml_edit::TableLike, Problem> {
    let slot = doc.entry(KEY).or_insert_with(|| {
        let mut table = Table::new();
        table.set_implicit(true);
        Item::Table(table)
    });
    slot.as_table_like_mut()
        .ok_or_else(|| Problem::refused(format!("`{KEY}` is not a table")))
}
