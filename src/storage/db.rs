//! Opening a Turso database and keeping its schema honest.

use std::path::Path;

use turso::{Builder, Connection, Database, Row, Value};

use crate::error::{Result, SeleniumBaseError};

/// Opens the database file at `path`, creating it and its directory if needed.
pub(super) async fn open_file(path: &Path) -> Result<Database> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        tokio::fs::create_dir_all(parent).await?;
    }
    let path = path.to_str().ok_or_else(|| {
        SeleniumBaseError::InvalidConfig(format!(
            "the database path is not valid UTF-8: {}",
            path.display()
        ))
    })?;
    Ok(Builder::new_local(path).build().await?)
}

/// Opens a database that lives only as long as the process.
pub(super) async fn open_memory() -> Result<Database> {
    Ok(Builder::new_local(":memory:").build().await?)
}

/// Prepares a freshly opened database for use as a `kind` store.
///
/// A new file gets `schema` applied and is stamped with `kind` and `version`.
/// An existing one is checked instead: opening a results database as a profile
/// vault, or a file written by a newer release, is an error rather than a
/// silent misreading.
pub(super) async fn prepare(
    conn: &Connection,
    kind: &str,
    version: i64,
    schema: &str,
) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
    )
    .await?;
    match (meta(conn, "kind").await?, meta(conn, "version").await?) {
        (None, None) => {
            conn.execute_batch(schema).await?;
            conn.execute("INSERT INTO meta (key, value) VALUES ('kind', ?1)", [kind])
                .await?;
            conn.execute(
                "INSERT INTO meta (key, value) VALUES ('version', ?1)",
                [version.to_string()],
            )
            .await?;
            Ok(())
        }
        (Some(found), Some(stored)) => {
            if found != kind {
                return Err(SeleniumBaseError::Database(format!(
                    "this file is a {found} database, not a {kind} database"
                )));
            }
            let stored: i64 = stored.parse().map_err(|_| {
                SeleniumBaseError::Database(format!("unreadable schema version {stored:?}"))
            })?;
            if stored > version {
                return Err(SeleniumBaseError::Database(format!(
                    "this {kind} database uses schema {stored}, but this release only \
                     understands up to {version}; upgrade seleniumbase-rs"
                )));
            }
            Ok(())
        }
        _ => Err(SeleniumBaseError::Database(
            "the database's metadata is incomplete".to_owned(),
        )),
    }
}

async fn meta(conn: &Connection, key: &str) -> Result<Option<String>> {
    let mut rows = conn
        .query("SELECT value FROM meta WHERE key = ?1", [key])
        .await?;
    match rows.next().await? {
        Some(row) => Ok(Some(text(&row, 0)?)),
        None => Ok(None),
    }
}

/// Reads column `idx` as text.
pub(super) fn text(row: &Row, idx: usize) -> Result<String> {
    match row.get_value(idx)? {
        Value::Text(text) => Ok(text),
        other => Err(unexpected(idx, "text", &other)),
    }
}

/// Reads column `idx` as an integer.
pub(super) fn integer(row: &Row, idx: usize) -> Result<i64> {
    match row.get_value(idx)? {
        Value::Integer(n) => Ok(n),
        other => Err(unexpected(idx, "an integer", &other)),
    }
}

/// Reads column `idx` as an integer that may be absent.
pub(super) fn optional_integer(row: &Row, idx: usize) -> Result<Option<i64>> {
    match row.get_value(idx)? {
        Value::Null => Ok(None),
        Value::Integer(n) => Ok(Some(n)),
        other => Err(unexpected(idx, "an integer or null", &other)),
    }
}

/// Reads column `idx` as a real number. SQLite stores a whole-number real as
/// an integer when it can, so both are accepted.
pub(super) fn real(row: &Row, idx: usize) -> Result<f64> {
    match row.get_value(idx)? {
        Value::Real(n) => Ok(n),
        #[expect(
            clippy::cast_precision_loss,
            reason = "durations and counts are far below 2^53"
        )]
        Value::Integer(n) => Ok(n as f64),
        other => Err(unexpected(idx, "a number", &other)),
    }
}

/// Names what a column holds without echoing the value, which may be large or
/// sensitive.
fn unexpected(idx: usize, wanted: &str, found: &Value) -> SeleniumBaseError {
    let found = match found {
        Value::Null => "null",
        Value::Integer(_) => "an integer",
        Value::Real(_) => "a real number",
        Value::Text(_) => "text",
        Value::Blob(_) => "a blob",
    };
    SeleniumBaseError::Database(format!("column {idx} should be {wanted}, found {found}"))
}
