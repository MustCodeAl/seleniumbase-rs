//! Reads and writes a `StealthIdentity` file, atomically and with owner-only permissions.
//!
//! The file is the identity as pretty-printed JSON. It holds cookies, which are credentials, so:
//!
//! - **Writing** goes through the atomic writer of the JSON state store: a sibling temporary file created new with mode
//!   `0600` on Unix, synced and renamed over the target, so a reader sees the old file or the new
//!   one, and an older file with wider permissions is replaced by one that has `0600`. An identity
//!   that [`StealthIdentity::verify`] would refuse (another schema, an incoherent fingerprint, a
//!   proxy) is not written.
//! - **Reading** refuses a file that group or others can access (mode bits `0o077`) on Unix, as
//!   `ssh` does for a private key. The file is **not** corrected: wider permissions mean the
//!   cookies may already have been read, and the owner should decide what to do. The schema is read
//!   first, so a file from another version is reported as that. The jar validates itself while it
//!   is read, so a file with a cookie the jar would never store is refused. Last, the identity is
//!   verified as it is before it is written.
//!
//! Errors never echo what the file holds. A parse error says where it went wrong (line and column)
//! and in which way, not the value it met, because that value may be a cookie.
//!
//! One process writes a file at a time; there is no lock between processes.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::scrape::passport::identity::{
    json_failure, IdentityError, StealthIdentity, IDENTITY_SCHEMA,
};
use crate::scrape::runtime::json_store::atomic_write;

#[cfg(test)]
mod tests;

/// The largest identity file that is read. A jar at its bounds fits well inside it.
pub const MAX_IDENTITY_FILE_BYTES: u64 = 16 * 1024 * 1024;

/// The bits of a Unix mode that give access to the group or to others.
#[cfg(unix)]
const GROUP_AND_OTHER_BITS: u32 = 0o077;

/// The bits of a Unix mode that are permissions.
#[cfg(unix)]
const PERMISSION_BITS: u32 = 0o777;

/// Just the schema of a file, read first so that a file of another layout is reported as that and
/// not as corrupt.
#[derive(Deserialize)]
struct Header {
    schema: u32,
}

/// Why an identity file could not be read or written.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum IdentityFileError {
    /// The file could not be read or written.
    #[error("cannot read or write the identity file {}: {source}", .path.display())]
    Io {
        /// The identity file.
        path: PathBuf,
        /// What the operating system said.
        #[source]
        source: io::Error,
    },
    /// The file is not an identity. It is left as it is.
    #[error("the identity file {} is not valid: {reason} at line {line}, column {column}", .path.display())]
    Corrupt {
        /// The identity file.
        path: PathBuf,
        /// In which way it is not valid. Never the value that was met.
        reason: &'static str,
        /// The line where the parse failed, from one.
        line: usize,
        /// The column where the parse failed, from one.
        column: usize,
    },
    /// The file is larger than an identity file is allowed to be.
    #[error("the identity file {} is larger than {limit} bytes", .path.display())]
    TooLarge {
        /// The identity file.
        path: PathBuf,
        /// The most it may hold.
        limit: u64,
    },
    /// The file can be read or written by someone other than its owner.
    #[error("the identity file {} has mode {mode:03o} and holds cookies; run `chmod 600` on it, or delete it", .path.display())]
    Permissions {
        /// The identity file.
        path: PathBuf,
        /// Its permission bits.
        mode: u32,
    },
    /// The file holds an identity this version refuses.
    #[error("the identity file {} holds an identity that is refused: {source}", .path.display())]
    Identity {
        /// The identity file.
        path: PathBuf,
        /// Why. Boxed, so that this error stays small to return.
        #[source]
        source: Box<IdentityError>,
    },
}

impl IdentityFileError {
    /// Whether the file does not exist.
    #[must_use]
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::Io { source, .. } if source.kind() == io::ErrorKind::NotFound)
    }
}

/// Writes `identity` to `path`, creating the directories above it. This blocks on the file system;
/// use [`save`] from async code.
///
/// # Errors
///
/// Returns [`IdentityFileError::Identity`] when the identity is refused by
/// [`StealthIdentity::verify`], and [`IdentityFileError::Io`] when the file cannot be written.
pub fn save_blocking(path: &Path, identity: &StealthIdentity) -> Result<(), IdentityFileError> {
    let bytes = encode(path, identity)?;
    write_file(path, &bytes).map_err(|source| io_error(path, source))
}

/// The file's content for `identity`, after checking that it would be read back.
fn encode(path: &Path, identity: &StealthIdentity) -> Result<Vec<u8>, IdentityFileError> {
    identity.verify().map_err(|source| refused(path, source))?;
    serde_json::to_vec_pretty(identity)
        .map_err(|error| io_error(path, io::Error::other(json_failure(&error))))
}

/// Writes `bytes` to `path` atomically, creating the directories above it first.
fn write_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    atomic_write(path, bytes)
}

fn io_error(path: &Path, source: io::Error) -> IdentityFileError {
    IdentityFileError::Io {
        path: path.to_path_buf(),
        source,
    }
}

fn refused(path: &Path, source: IdentityError) -> IdentityFileError {
    IdentityFileError::Identity {
        path: path.to_path_buf(),
        source: Box::new(source),
    }
}

/// A parse failure, described by where and in which way, never by the value met.
fn corrupt(path: &Path, error: &serde_json::Error) -> IdentityFileError {
    IdentityFileError::Corrupt {
        path: path.to_path_buf(),
        reason: json_failure(error),
        line: error.line(),
        column: error.column(),
    }
}

/// Reads the identity at `path`. This blocks on the file system; use [`load`] from async code.
///
/// # Errors
///
/// Returns [`IdentityFileError::Io`] when the file is missing or cannot be read,
/// [`IdentityFileError::Permissions`] when others can access it, [`IdentityFileError::TooLarge`]
/// when it is over [`MAX_IDENTITY_FILE_BYTES`], [`IdentityFileError::Corrupt`] when it is not an
/// identity, and [`IdentityFileError::Identity`] for another schema or an identity that
/// [`StealthIdentity::verify`] refuses.
pub fn load_blocking(path: &Path) -> Result<StealthIdentity, IdentityFileError> {
    load_with_limit(path, MAX_IDENTITY_FILE_BYTES)
}

/// [`load_blocking`] with the size limit given.
fn load_with_limit(path: &Path, limit: u64) -> Result<StealthIdentity, IdentityFileError> {
    let bytes = read_checked(path, limit)?;
    let header: Header = serde_json::from_slice(&bytes).map_err(|error| corrupt(path, &error))?;
    if header.schema != IDENTITY_SCHEMA {
        return Err(refused(path, IdentityError::Schema(header.schema)));
    }
    let identity: StealthIdentity =
        serde_json::from_slice(&bytes).map_err(|error| corrupt(path, &error))?;
    identity.verify().map_err(|source| refused(path, source))?;
    Ok(identity)
}

/// Opens the identity file, checks that it is a regular file only its owner can reach, and reads
/// at most `limit` bytes of it.
fn read_checked(path: &Path, limit: u64) -> Result<Vec<u8>, IdentityFileError> {
    let metadata = fs::metadata(path).map_err(|source| io_error(path, source))?;
    if !metadata.is_file() {
        let source = io::Error::new(io::ErrorKind::InvalidInput, "not a regular file");
        return Err(io_error(path, source));
    }
    let file = fs::File::open(path).map_err(|source| io_error(path, source))?;
    check_permissions(path, &file)?;
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|source| io_error(path, source))?;
    if u64::try_from(bytes.len()).map_or(true, |length| length > limit) {
        return Err(IdentityFileError::TooLarge {
            path: path.to_path_buf(),
            limit,
        });
    }
    Ok(bytes)
}

/// Refuses a file that group or others can access. The mode is read from the open file, so it is
/// the one of the file that is about to be read.
#[cfg(unix)]
fn check_permissions(path: &Path, file: &fs::File) -> Result<(), IdentityFileError> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = file.metadata().map_err(|source| io_error(path, source))?;
    let mode = metadata.permissions().mode() & PERMISSION_BITS;
    if mode & GROUP_AND_OTHER_BITS == 0 {
        Ok(())
    } else {
        Err(IdentityFileError::Permissions {
            path: path.to_path_buf(),
            mode,
        })
    }
}

/// Other systems have no mode to check; the file there has the permissions of its directory.
#[cfg(not(unix))]
fn check_permissions(_path: &Path, _file: &fs::File) -> Result<(), IdentityFileError> {
    Ok(())
}

/// Writes `identity` to `path` without blocking the async threads. See [`save_blocking`].
///
/// # Errors
///
/// As [`save_blocking`].
#[tracing::instrument(skip_all, fields(path = %path.as_ref().display()))]
pub async fn save(
    path: impl AsRef<Path>,
    identity: &StealthIdentity,
) -> Result<(), IdentityFileError> {
    let path = path.as_ref().to_path_buf();
    let bytes = encode(&path, identity)?;
    let target = path.clone();
    tokio::task::spawn_blocking(move || write_file(&target, &bytes))
        .await
        .map_err(io::Error::other)
        .and_then(|written| written)
        .map_err(|source| io_error(&path, source))
}

/// Reads the identity at `path` without blocking the async threads. See [`load_blocking`].
///
/// # Errors
///
/// As [`load_blocking`].
#[tracing::instrument(skip_all, fields(path = %path.as_ref().display()))]
pub async fn load(path: impl AsRef<Path>) -> Result<StealthIdentity, IdentityFileError> {
    let path = path.as_ref().to_path_buf();
    let target = path.clone();
    tokio::task::spawn_blocking(move || load_blocking(&target))
        .await
        .map_err(|source| io_error(&path, io::Error::other(source)))?
}
