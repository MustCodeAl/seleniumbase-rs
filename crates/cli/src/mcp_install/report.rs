//! What an install or uninstall did for one client, and how it is printed.

use std::io::{self, Write};

use super::client::Client;

/// What happened to one entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    /// The name was free and the entry was added.
    Added,
    /// A different entry of that name was replaced (`--force`).
    Replaced,
    /// The same entry was there already; nothing changed.
    Unchanged,
    /// A different entry of that name is there and was left alone.
    Conflict,
    /// The entry was removed.
    Removed,
    /// There was no such entry to remove.
    Absent,
}

/// An entry and what happened to it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntryReport {
    /// The entry's name.
    pub name: String,
    /// What was done, or in a dry run what would be.
    pub status: Status,
}

/// Why a client was not changed, wholly or in part.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Problem {
    /// The request cannot be carried out here and nothing was written: an unsupported scope,
    /// a file that cannot be rewritten safely, a program that is not installed. Exit status 2.
    Refused {
        /// Why.
        reason: String,
        /// What to do by hand instead: a snippet, or the commands.
        by_hand: Option<String>,
    },
    /// Something went wrong while changing it: a write or a program failed. Exit status 1.
    Failed(String),
}

impl Problem {
    /// A refusal with nothing to do by hand.
    pub fn refused(reason: impl Into<String>) -> Self {
        Self::Refused {
            reason: reason.into(),
            by_hand: None,
        }
    }
}

/// Something a dry run shows instead of doing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Preview {
    /// Text that would be written to the config, headed by where.
    Snippet {
        /// The heading, such as `would write to /h/.cursor/mcp.json`.
        heading: String,
        /// The text.
        text: String,
    },
    /// A command that would be run.
    Command(String),
}

/// The outcome for one client.
#[derive(Clone, Debug)]
pub struct ClientReport {
    /// The client.
    pub client: Client,
    /// The config it was done in, as text (empty when the client has none here).
    pub location: String,
    /// One report for each entry that was decided on, in the order of the request.
    pub entries: Vec<EntryReport>,
    /// What is left undone, if anything.
    pub problem: Option<Problem>,
    /// What a dry run shows.
    pub preview: Vec<Preview>,
    /// Things the user should know, such as to quit the app first.
    pub warnings: Vec<String>,
}

impl ClientReport {
    /// An empty report for `client`.
    pub fn new(client: Client) -> Self {
        Self {
            client,
            location: String::new(),
            entries: Vec::new(),
            problem: None,
            preview: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// The exit status this report asks for: 0, 1 when something failed, 2 when something was
    /// refused or a different entry was left alone.
    pub fn exit_code(&self) -> i32 {
        match &self.problem {
            Some(Problem::Failed(_)) => 1,
            Some(Problem::Refused { .. }) => 2,
            None if self
                .entries
                .iter()
                .any(|entry| entry.status == Status::Conflict) =>
            {
                2
            }
            None => 0,
        }
    }

    /// Writes the report: the outcome of each entry and what a dry run shows to `out`, the
    /// warnings and the reason for a refusal to `err`.
    ///
    /// # Errors
    ///
    /// An error of the writers.
    pub fn print(&self, dry_run: bool, out: &mut dyn Write, err: &mut dyn Write) -> io::Result<()> {
        for warning in &self.warnings {
            writeln!(err, "warning: {}: {warning}", self.client.id())?;
        }
        if self.location.is_empty() {
            writeln!(out, "{}", self.client.id())?;
        } else {
            writeln!(out, "{}  {}", self.client.id(), self.location)?;
        }
        let width = self
            .entries
            .iter()
            .map(|entry| entry.name.len())
            .max()
            .unwrap_or(0);
        for entry in &self.entries {
            writeln!(
                out,
                "  {:width$}  {}",
                entry.name,
                status_text(entry.status, dry_run)
            )?;
        }
        for preview in &self.preview {
            match preview {
                Preview::Snippet { heading, text } => {
                    writeln!(out, "  {heading}:")?;
                    for line in text.lines() {
                        writeln!(out, "    {line}")?;
                    }
                }
                Preview::Command(command) => writeln!(out, "  would run: {command}")?,
            }
        }
        match &self.problem {
            Some(Problem::Refused { reason, by_hand }) => {
                writeln!(err, "{}: not changed: {reason}", self.client.id())?;
                if let Some(text) = by_hand {
                    writeln!(out, "  to do it by hand:")?;
                    for line in text.lines() {
                        writeln!(out, "    {line}")?;
                    }
                }
            }
            Some(Problem::Failed(message)) => {
                writeln!(err, "{}: failed: {message}", self.client.id())?;
            }
            None => {}
        }
        Ok(())
    }
}

fn status_text(status: Status, dry_run: bool) -> &'static str {
    match (status, dry_run) {
        (Status::Added, false) => "added",
        (Status::Added, true) => "would be added",
        (Status::Replaced, false) => "replaced",
        (Status::Replaced, true) => "would be replaced",
        (Status::Unchanged, _) => "unchanged (the same entry is already there)",
        (Status::Conflict, _) => {
            "conflict: a different entry of this name is there and was left alone (--force replaces it)"
        }
        (Status::Removed, false) => "removed",
        (Status::Removed, true) => "would be removed",
        (Status::Absent, _) => "not installed",
    }
}

/// The exit status for several clients: the largest of theirs.
pub fn exit_code(reports: &[ClientReport]) -> i32 {
    reports
        .iter()
        .map(ClientReport::exit_code)
        .max()
        .unwrap_or(0)
}
