//! `sbrs-mcp repair`: finds what is broken in the entries the MCP clients hold for sbrs-mcp and
//! fixes it.
//!
//! The check reads each client's config through the same readers `clients` and `install` use,
//! and the fix is written through the same editors (`apply::perform` with [`Task::Repair`]), so
//! the rest of a file keeps its bytes, the old file is kept as `FILE.bak-sbrs-mcp`, and a file
//! that cannot be parsed is reported and never rewritten.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use super::apply::{Job, Scope, Task, perform, registered};
use super::client::Client;
use super::entry::{Entry, Existing, LaunchSpec, Only, names};
use super::env::Env;
use super::report::ClientReport;
use super::{check_name, parse_clients};

/// What `repair` was asked for.
#[derive(Clone, Debug)]
pub struct RepairRequest {
    /// The client names, as typed; every client with a config when empty.
    pub clients: Vec<String>,
    /// The path to register for this program.
    pub exe: PathBuf,
    /// The name of the browser entry; the development entry is `NAME-dev`.
    pub name: String,
    /// Report and change nothing.
    pub dry_run: bool,
}

/// What is wrong, about one entry or about a whole config.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Finding {
    /// The config cannot be read, so nothing in it was checked and nothing is touched.
    ConfigUnreadable {
        /// Why: invalid JSON or TOML, comments in JSON, not UTF-8.
        why: String,
    },
    /// The program the entry starts is not there.
    BinaryMissing {
        /// The path the entry names.
        path: String,
    },
    /// The entry names something that is there but cannot be run as a program.
    BinaryNotExecutable {
        /// The path the entry names.
        path: String,
    },
    /// The entry starts a sbrs-mcp that is not this one.
    OtherBinary {
        /// The path the entry names.
        path: String,
    },
    /// The entry names the program without a full path, which a desktop app may not find.
    RelativeCommand {
        /// The command as written.
        command: String,
    },
    /// Another entry of the same config starts the same server.
    Duplicate {
        /// The entry that stays.
        of: String,
    },
    /// The entry starts sbrs-mcp under a name other than `NAME` and `NAME-dev`. Not a fault: it
    /// is reported and left as it is.
    OtherName,
}

impl Finding {
    /// Whether the finding means something is broken. `false` for a note.
    pub fn is_fault(&self) -> bool {
        !matches!(self, Self::OtherName)
    }

    /// Whether `repair` can fix the fault.
    pub fn is_fixable(&self) -> bool {
        !matches!(self, Self::ConfigUnreadable { .. } | Self::OtherName)
    }
}

/// A finding and the entry it is about.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Issue {
    /// The entry's name; `None` for a finding about the whole config.
    pub entry: Option<String>,
    /// What is wrong.
    pub finding: Finding,
}

/// What the check found in one config, and what the fix would write.
#[derive(Debug)]
pub struct Examination {
    /// The config file.
    pub location: PathBuf,
    /// Everything found, by entry name.
    pub issues: Vec<Issue>,
    /// The entries the fix writes, as they are to be.
    write: Vec<Entry>,
    /// The names of the entries the fix removes.
    remove: Vec<String>,
}

/// Checks the config of `client` in `scope`; nothing is written. `None` when the client has no
/// such config file here.
pub fn examine(
    env: &Env,
    request: &RepairRequest,
    client: Client,
    scope: Scope,
) -> Option<Examination> {
    let location = match scope {
        Scope::User => client.user_path(env),
        Scope::Project => client.project_path(env),
    }
    .filter(|path| path.exists())?;
    let existing = match registered(env, client, scope) {
        Ok(existing) => existing,
        Err(why) => {
            return Some(Examination {
                location,
                issues: vec![Issue {
                    entry: None,
                    finding: Finding::ConfigUnreadable { why },
                }],
                write: Vec::new(),
                remove: Vec::new(),
            });
        }
    };
    let probe = Probe::new(env, request);
    let analysis = analyze(&existing, &probe);
    Some(Examination {
        location,
        issues: analysis.issues,
        write: analysis.write,
        remove: analysis.remove,
    })
}

/// What the check compares the entries with.
struct Probe<'a> {
    env: &'a Env,
    /// The path to write into an entry that is fixed.
    exe: String,
    /// This program with its links resolved, to tell it from another sbrs-mcp.
    current: PathBuf,
    /// `NAME` and `NAME-dev`: the names `install` uses.
    names: Vec<String>,
}

impl<'a> Probe<'a> {
    fn new(env: &'a Env, request: &RepairRequest) -> Self {
        Self {
            env,
            exe: request.exe.to_string_lossy().into_owned(),
            current: resolved(&request.exe),
            names: names(&request.name, Only::Both),
        }
    }
}

/// `path` with its links resolved, or as it is when it cannot be (a program that is not there).
fn resolved(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// What an entry's command turns out to be.
enum Program {
    /// Nothing is there; the text is the path as it is shown.
    Missing(String),
    /// A file that cannot be run, or a folder; the text is the path as it is shown.
    NotExecutable(String),
    /// A program that can be run, with its links resolved.
    Runs(PathBuf),
}

/// The file `command` names: the path itself, a path relative to the working folder, or the
/// program of that name on `PATH`.
fn locate(env: &Env, command: &str) -> Option<PathBuf> {
    let path = Path::new(command);
    if path.is_absolute() {
        return Some(path.to_path_buf());
    }
    if path.components().count() > 1 {
        return Some(env.cwd.join(path));
    }
    env.find_program(command)
}

fn inspect(env: &Env, command: &str) -> Program {
    let Some(path) = locate(env, command) else {
        return Program::Missing(command.to_owned());
    };
    match fs::metadata(&path) {
        Err(_) => Program::Missing(path.display().to_string()),
        Ok(meta) if meta.is_file() && is_executable(&meta) => Program::Runs(resolved(&path)),
        Ok(_) => Program::NotExecutable(path.display().to_string()),
    }
}

#[cfg(unix)]
fn is_executable(meta: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_meta: &fs::Metadata) -> bool {
    true
}

/// Whether `command` names a program called `sbrs-mcp` (with `.exe` on Windows).
fn is_sbrs_mcp(command: &str) -> bool {
    Path::new(command)
        .file_stem()
        .is_some_and(|stem| stem.eq_ignore_ascii_case("sbrs-mcp"))
}

/// An entry that starts sbrs-mcp, or is called as one of ours and starts nothing that runs.
struct Candidate {
    name: String,
    /// Called `NAME` or `NAME-dev`.
    canonical: bool,
    /// The entry as it is to be: the same, or started by this program.
    fixed: LaunchSpec,
    /// What is wrong with the program the entry starts; empty when nothing is.
    faults: Vec<Finding>,
}

/// The entries of `existing` that are ours to look at.
///
/// An entry is ours when it starts a program called `sbrs-mcp` or this program under any name,
/// or has one of the names `install` uses and starts something that cannot run. An entry with
/// one of those names that starts another program that runs (Python SeleniumBase's own server)
/// is not ours, and neither is any other entry.
fn candidates(existing: &Existing, probe: &Probe<'_>) -> Vec<Candidate> {
    existing
        .iter()
        .filter_map(|(name, spec)| {
            let spec = spec.as_ref()?;
            let canonical = probe.names.contains(name);
            let program = inspect(probe.env, &spec.command);
            let runs = matches!(program, Program::Runs(_));
            let is_current = matches!(&program, Program::Runs(path) if *path == probe.current);
            if !(is_sbrs_mcp(&spec.command) || is_current || (canonical && !runs)) {
                return None;
            }
            let mut faults = Vec::new();
            match program {
                Program::Missing(path) => faults.push(Finding::BinaryMissing { path }),
                Program::NotExecutable(path) => faults.push(Finding::BinaryNotExecutable { path }),
                Program::Runs(_) if !is_current => faults.push(Finding::OtherBinary {
                    path: spec.command.clone(),
                }),
                Program::Runs(_) => {}
            }
            if faults.is_empty() && !Path::new(&spec.command).is_absolute() {
                faults.push(Finding::RelativeCommand {
                    command: spec.command.clone(),
                });
            }
            let fixed = if faults.is_empty() {
                spec.clone()
            } else {
                LaunchSpec {
                    command: probe.exe.clone(),
                    ..spec.clone()
                }
            };
            Some(Candidate {
                name: name.clone(),
                canonical,
                fixed,
                faults,
            })
        })
        .collect()
}

/// What the check found in the entries of one config, and what to write for it.
#[derive(Default)]
struct Analysis {
    issues: Vec<Issue>,
    write: Vec<Entry>,
    remove: Vec<String>,
}

/// Checks the entries of `existing`, by name.
///
/// Entries that start the same server once the program is fixed are duplicates of each other:
/// the one with a name `install` uses stays (else the first by name), the others are removed,
/// and only the one that stays is looked at further.
fn analyze(existing: &Existing, probe: &Probe<'_>) -> Analysis {
    let found = candidates(existing, probe);
    let keeper_of: Vec<Option<usize>> = found
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            let same: Vec<usize> = found
                .iter()
                .enumerate()
                .filter(|(_, other)| other.fixed == candidate.fixed)
                .map(|(other_index, _)| other_index)
                .collect();
            let keeper = same
                .iter()
                .copied()
                .find(|&other_index| found[other_index].canonical)
                .or_else(|| same.first().copied())?;
            (keeper != index).then_some(keeper)
        })
        .collect();
    let mut analysis = Analysis::default();
    for (candidate, keeper) in found.iter().zip(&keeper_of) {
        let issue = |finding| Issue {
            entry: Some(candidate.name.clone()),
            finding,
        };
        if let Some(keeper) = keeper {
            analysis.issues.push(issue(Finding::Duplicate {
                of: found[*keeper].name.clone(),
            }));
            analysis.remove.push(candidate.name.clone());
            continue;
        }
        analysis
            .issues
            .extend(candidate.faults.iter().cloned().map(issue));
        if !candidate.faults.is_empty() {
            analysis.write.push(Entry {
                name: candidate.name.clone(),
                spec: candidate.fixed.clone(),
            });
        }
        if !candidate.canonical {
            analysis.issues.push(issue(Finding::OtherName));
        }
    }
    analysis
}

/// Checks the clients, fixes what can be fixed, prints each finding and each fix, and returns
/// the exit status: 0 when nothing is left broken, 1 when a fix failed or a dry run found
/// something to fix, 2 when something cannot be fixed here.
pub fn repair(env: &Env, request: &RepairRequest, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    if let Err(status) = check_name(&request.name, err) {
        return status;
    }
    if request.exe.to_str().is_none() {
        let _ = writeln!(err, "sbrs-mcp: the program path must be valid UTF-8");
        return 2;
    }
    let clients = if request.clients.is_empty() {
        Client::ALL.to_vec()
    } else {
        match parse_clients(&request.clients, err) {
            Ok(clients) => clients,
            Err(status) => return status,
        }
    };
    if request.dry_run {
        let _ = writeln!(err, "dry run: nothing is written or run");
    }
    let mut tally = Tally::default();
    let mut status = 0;
    for client in clients {
        let mut found = false;
        for scope in [Scope::User, Scope::Project] {
            let Some(examination) = examine(env, request, client, scope) else {
                continue;
            };
            found = true;
            tally.configs += 1;
            let job = Job {
                env,
                scope,
                dry_run: request.dry_run,
            };
            status = status.max(settle(&job, client, &examination, &mut tally, out, err));
        }
        if !found && !request.clients.is_empty() {
            let _ = writeln!(out, "{}  no config file here", client.id());
        }
    }
    let _ = writeln!(out, "{}", tally.summary());
    status
}

/// How many configs were looked at and what became of the faults in them.
#[derive(Default)]
struct Tally {
    configs: usize,
    fixed: usize,
    would_fix: usize,
    left: usize,
}

impl Tally {
    fn count(&mut self, outcome: Outcome) {
        match outcome {
            Outcome::Fixed => self.fixed += 1,
            Outcome::WouldFix => self.would_fix += 1,
            Outcome::NotFixed | Outcome::Untouched => self.left += 1,
            Outcome::Kept => {}
        }
    }

    fn summary(&self) -> String {
        if self.configs == 0 {
            return "nothing to repair: no client config was found here".to_owned();
        }
        let mut parts = Vec::new();
        if self.fixed > 0 {
            parts.push(format!("fixed {}", self.fixed));
        }
        if self.would_fix > 0 {
            parts.push(format!("{} would be fixed", self.would_fix));
        }
        if self.left > 0 {
            parts.push(format!("{} left broken", self.left));
        }
        let word = if self.configs == 1 {
            "config"
        } else {
            "configs"
        };
        if parts.is_empty() {
            format!("nothing to repair: checked {} {word}", self.configs)
        } else {
            format!("checked {} {word}: {}", self.configs, parts.join(", "))
        }
    }
}

/// What became of one finding.
#[derive(Clone, Copy)]
enum Outcome {
    Fixed,
    WouldFix,
    NotFixed,
    Untouched,
    Kept,
}

impl Outcome {
    fn of(finding: &Finding, applied: bool, dry_run: bool) -> Self {
        match finding {
            Finding::ConfigUnreadable { .. } => Self::Untouched,
            _ if !finding.is_fault() => Self::Kept,
            _ if !applied => Self::NotFixed,
            _ if dry_run => Self::WouldFix,
            _ => Self::Fixed,
        }
    }

    fn text(self) -> &'static str {
        match self {
            Self::Fixed => "fixed",
            Self::WouldFix => "would be fixed",
            Self::NotFixed => "not fixed",
            Self::Untouched => "left untouched",
            Self::Kept => "left as it is",
        }
    }
}

/// Fixes what `examination` found in one config and prints the config, each finding with what
/// became of it, and what a dry run would write or run. Returns the exit status for this config.
fn settle(
    job: &Job<'_>,
    client: Client,
    examination: &Examination,
    tally: &mut Tally,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let fixable = examination
        .issues
        .iter()
        .any(|issue| issue.finding.is_fixable());
    let report: Option<ClientReport> = fixable.then(|| {
        let task = Task::Repair {
            write: &examination.write,
            remove: &examination.remove,
        };
        perform(job, client, &task)
    });
    let applied = report
        .as_ref()
        .is_some_and(|report| report.problem.is_none());
    let _ = writeln!(out, "{}  {}", client.id(), examination.location.display());
    let subject = |issue: &Issue| issue.entry.clone().unwrap_or_else(|| "config".to_owned());
    let width = examination
        .issues
        .iter()
        .map(|issue| subject(issue).len())
        .max()
        .unwrap_or(0);
    let mut status = 0;
    for issue in &examination.issues {
        let outcome = Outcome::of(&issue.finding, applied, job.dry_run);
        tally.count(outcome);
        let _ = writeln!(
            out,
            "  {:width$}  {}: {}",
            subject(issue),
            issue.finding,
            outcome.text()
        );
        if matches!(issue.finding, Finding::ConfigUnreadable { .. }) {
            status = 2;
        }
    }
    let Some(report) = report else {
        return status;
    };
    for warning in &report.warnings {
        let _ = writeln!(err, "warning: {}: {warning}", client.id());
    }
    let _ = report.print_preview(out);
    let _ = report.print_problem(out, err);
    let wrote = if report.problem.is_some() {
        report.exit_code()
    } else {
        i32::from(job.dry_run)
    };
    status.max(wrote)
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConfigUnreadable { why } => write!(f, "cannot be read: {why}"),
            Self::BinaryMissing { path } => write!(f, "starts {path}, which is not there"),
            Self::BinaryNotExecutable { path } => {
                write!(f, "starts {path}, which is not an executable file")
            }
            Self::OtherBinary { path } => write!(f, "starts {path}, which is not this sbrs-mcp"),
            Self::RelativeCommand { command } => write!(
                f,
                "starts `{command}` without a path, which a desktop app may not find"
            ),
            Self::Duplicate { of } => write!(f, "starts the same server as {of}"),
            Self::OtherName => write!(f, "starts sbrs-mcp under a name that install does not use"),
        }
    }
}
