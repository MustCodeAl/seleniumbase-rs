//! Registering with one client, or removing the registration: which file or which command
//! changes for each client, and the checks that come first.

use std::path::{Path, PathBuf};

use serde_json::json;

use super::client::{Client, Kind};
use super::entry::{decide, Decision, Entry, Existing};
use super::env::Env;
use super::external::{codex_has_mcp_commands, Cmd};
use super::jsonfile::{self, Op};
use super::report::{ClientReport, EntryReport, Preview, Problem, Status};
use super::tomlfile;

/// Where a registration lives.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Scope {
    /// The user's own config, for every project.
    User,
    /// The config file in the current folder.
    Project,
}

impl Scope {
    fn word(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Project => "project",
        }
    }
}

/// What a command line asks for, shared by every client of the request.
#[derive(Clone, Copy, Debug)]
pub struct Job<'a> {
    /// The machine.
    pub env: &'a Env,
    /// User or project config.
    pub scope: Scope,
    /// Show what would be done and change nothing.
    pub dry_run: bool,
}

/// What to do with the entries.
#[derive(Clone, Copy, Debug)]
pub enum Task<'a> {
    /// Register `entries`.
    Install {
        /// What to register.
        entries: &'a [Entry],
        /// Replace a different entry of the same name.
        force: bool,
    },
    /// Remove the entries called `names`.
    Uninstall {
        /// The names.
        names: &'a [String],
    },
}

/// Printed for Claude Desktop with every install and uninstall.
pub const CLAUDE_DESKTOP_WARNING: &str = "the Claude app rewrites claude_desktop_config.json \
while it runs, and an entry written while it ran was dropped: quit Claude before this \
and open it again after";

/// The config file for `client` in `scope`, or the reason there is none.
fn location(job: &Job<'_>, client: Client) -> Result<PathBuf, Problem> {
    match job.scope {
        Scope::User => client.user_path(job.env).ok_or_else(|| {
            Problem::refused(format!(
                "{} is documented for macOS and Windows only, so sbrs-mcp has no config \
                 location for it on this system",
                client.title()
            ))
        }),
        Scope::Project => client.project_path(job.env).ok_or_else(|| {
            Problem::refused(format!(
                "{} has no project config for --scope project ({}); use --scope user",
                client.title(),
                client.no_project_reason()
            ))
        }),
    }
}

/// Registers with `client`, or removes the registration, as `task` says.
pub fn perform(job: &Job<'_>, client: Client, task: &Task<'_>) -> ClientReport {
    let mut report = ClientReport::new(client);
    if client == Client::ClaudeDesktop {
        report.warnings.push(CLAUDE_DESKTOP_WARNING.to_owned());
    }
    let path = match location(job, client) {
        Ok(path) => path,
        Err(problem) => {
            report.problem = Some(problem);
            return report;
        }
    };
    report.location = path.display().to_string();
    match client.kind() {
        Kind::Json => json_client(job, client, &path, task, &mut report),
        Kind::ClaudeCli => claude_code(job, &path, task, &mut report),
        Kind::Codex => codex(job, &path, task, &mut report),
    }
    report
}

/// The entries `client` holds in `scope`, for `clients` to show. Empty when the config is not
/// there or the client has none for that scope.
///
/// # Errors
///
/// A message when the config exists but cannot be read.
pub fn registered(env: &Env, client: Client, scope: Scope) -> Result<Existing, String> {
    let path = match scope {
        Scope::User => client.user_path(env),
        Scope::Project => client.project_path(env),
    };
    let Some(path) = path.filter(|path| path.exists()) else {
        return Ok(Existing::new());
    };
    match client.layout() {
        Some(layout) => jsonfile::read_existing(&path, &layout),
        None => tomlfile::read_existing(&path),
    }
}

fn json_client(
    job: &Job<'_>,
    client: Client,
    path: &Path,
    task: &Task<'_>,
    report: &mut ClientReport,
) {
    let Some(layout) = client.layout() else {
        return;
    };
    let ops: Vec<Op<'_>> = match *task {
        Task::Install { entries, force } => entries
            .iter()
            .map(|entry| Op::Upsert { entry, force })
            .collect(),
        Task::Uninstall { names } => names.iter().map(|name| Op::Remove { name }).collect(),
    };
    match jsonfile::apply(path, &layout, &ops, job.dry_run) {
        Ok(applied) => {
            report.entries = applied.reports;
            report.preview = applied.preview;
        }
        Err(mut problem) => {
            if client == Client::Vscode {
                add_vscode_command(&mut problem, task);
            }
            report.problem = Some(problem);
        }
    }
}

/// VS Code's own `code --add-mcp` writes into a file with comments without losing them, so a
/// refusal to rewrite such a file names it too.
fn add_vscode_command(problem: &mut Problem, task: &Task<'_>) {
    let (
        Problem::Refused {
            by_hand: Some(text),
            ..
        },
        Task::Install { entries, .. },
    ) = (problem, task)
    else {
        return;
    };
    text.push_str("\nor with VS Code's own command, which keeps the comments:");
    for entry in *entries {
        let mut definition = json!({
            "name": entry.name,
            "command": entry.spec.command,
            "args": entry.spec.args,
        });
        if !entry.spec.env.is_empty() {
            definition["env"] = json!(entry.spec.env);
        }
        let cmd = Cmd {
            program: PathBuf::from("code"),
            args: vec!["--add-mcp".to_owned(), definition.to_string()],
            cwd: None,
        };
        text.push('\n');
        text.push_str(&cmd.display());
    }
}

/// One entry's part of a run through a client's own commands.
struct Step {
    name: String,
    status: Status,
    cmds: Vec<Cmd>,
}

/// The steps that bring the client from `existing` to what `task` asks for.
fn plan_steps(
    task: &Task<'_>,
    existing: &Existing,
    add: impl Fn(&Entry) -> Cmd,
    remove: impl Fn(&str) -> Cmd,
) -> Vec<Step> {
    match *task {
        Task::Install { entries, force } => entries
            .iter()
            .map(|entry| {
                let (status, cmds) = match decide(existing.get(&entry.name), &entry.spec, force) {
                    Decision::Add => (Status::Added, vec![add(entry)]),
                    Decision::Replace => (Status::Replaced, vec![remove(&entry.name), add(entry)]),
                    Decision::Unchanged => (Status::Unchanged, Vec::new()),
                    Decision::Conflict => (Status::Conflict, Vec::new()),
                };
                Step {
                    name: entry.name.clone(),
                    status,
                    cmds,
                }
            })
            .collect(),
        Task::Uninstall { names } => names
            .iter()
            .map(|name| {
                if existing.contains_key(name) {
                    Step {
                        name: name.clone(),
                        status: Status::Removed,
                        cmds: vec![remove(name)],
                    }
                } else {
                    Step {
                        name: name.clone(),
                        status: Status::Absent,
                        cmds: Vec::new(),
                    }
                }
            })
            .collect(),
    }
}

/// Runs the steps, or shows their commands in a dry run. A client whose program is not on
/// `PATH` is refused with the commands to type, in a dry run too, because the real run would
/// stop there.
fn drive(job: &Job<'_>, report: &mut ClientReport, steps: Vec<Step>, program: &str, found: bool) {
    if !found && steps.iter().any(|step| !step.cmds.is_empty()) {
        let by_hand = steps
            .iter()
            .flat_map(|step| &step.cmds)
            .map(Cmd::display)
            .collect::<Vec<_>>()
            .join("\n");
        report.entries.extend(
            steps
                .into_iter()
                .filter(|step| step.cmds.is_empty())
                .map(|step| EntryReport {
                    name: step.name,
                    status: step.status,
                }),
        );
        report.problem = Some(Problem::Refused {
            reason: format!("`{program}` is not on PATH, so sbrs-mcp cannot run it"),
            by_hand: Some(by_hand),
        });
        return;
    }
    if job.dry_run {
        for step in steps {
            report
                .preview
                .extend(step.cmds.iter().map(|cmd| Preview::Command(cmd.display())));
            report.entries.push(EntryReport {
                name: step.name,
                status: step.status,
            });
        }
        return;
    }
    for step in steps {
        for cmd in &step.cmds {
            if let Err(message) = cmd.run() {
                report.problem = Some(Problem::Failed(message));
                return;
            }
        }
        report.entries.push(EntryReport {
            name: step.name,
            status: step.status,
        });
    }
}

/// Claude Code: `claude mcp add-json` and `claude mcp remove` write, because the CLI owns
/// `~/.claude.json` (large, and written by Claude Code at the same time). The files are only
/// read, to tell an entry that is there from one that is not.
fn claude_code(job: &Job<'_>, path: &Path, task: &Task<'_>, report: &mut ClientReport) {
    let Some(layout) = Client::ClaudeCode.layout() else {
        return;
    };
    let existing = match jsonfile::read_existing(path, &layout) {
        Ok(existing) => existing,
        Err(message) => {
            report.problem = Some(Problem::Failed(message));
            return;
        }
    };
    let found = job.env.find_program("claude");
    let program = found.clone().unwrap_or_else(|| PathBuf::from("claude"));
    let scope = job.scope.word();
    let cwd = (job.scope == Scope::Project).then(|| job.env.cwd.clone());
    let steps = plan_steps(
        task,
        &existing,
        |entry| Cmd {
            program: program.clone(),
            args: vec![
                "mcp".to_owned(),
                "add-json".to_owned(),
                "--scope".to_owned(),
                scope.to_owned(),
                entry.name.clone(),
                layout.entry(&entry.spec).to_string(),
            ],
            cwd: cwd.clone(),
        },
        |name| Cmd {
            program: program.clone(),
            args: vec![
                "mcp".to_owned(),
                "remove".to_owned(),
                "--scope".to_owned(),
                scope.to_owned(),
                name.to_owned(),
            ],
            cwd: cwd.clone(),
        },
    );
    drive(job, report, steps, "claude", found.is_some());
}

/// Codex: `codex mcp add` and `codex mcp remove` when the installed `codex` has them,
/// otherwise the `[mcp_servers.NAME]` tables of `config.toml`.
fn codex(job: &Job<'_>, path: &Path, task: &Task<'_>, report: &mut ClientReport) {
    let found = job
        .env
        .find_program("codex")
        .filter(|program| codex_has_mcp_commands(program));
    let Some(program) = found else {
        let ops: Vec<tomlfile::Op<'_>> = match *task {
            Task::Install { entries, force } => entries
                .iter()
                .map(|entry| tomlfile::Op::Upsert { entry, force })
                .collect(),
            Task::Uninstall { names } => names
                .iter()
                .map(|name| tomlfile::Op::Remove { name })
                .collect(),
        };
        match tomlfile::apply(path, &ops, job.dry_run) {
            Ok(applied) => {
                report.entries = applied.reports;
                report.preview = applied.preview;
            }
            Err(problem) => report.problem = Some(problem),
        }
        return;
    };
    let existing = match tomlfile::read_existing(path) {
        Ok(existing) => existing,
        Err(message) => {
            report.problem = Some(Problem::Failed(message));
            return;
        }
    };
    let steps = plan_steps(
        task,
        &existing,
        |entry| {
            let mut args = vec!["mcp".to_owned(), "add".to_owned(), entry.name.clone()];
            for (key, value) in &entry.spec.env {
                args.push("--env".to_owned());
                args.push(format!("{key}={value}"));
            }
            args.push("--".to_owned());
            args.push(entry.spec.command.clone());
            args.extend(entry.spec.args.iter().cloned());
            Cmd {
                program: program.clone(),
                args,
                cwd: None,
            }
        },
        |name| Cmd {
            program: program.clone(),
            args: vec!["mcp".to_owned(), "remove".to_owned(), name.to_owned()],
            cwd: None,
        },
    );
    drive(job, report, steps, "codex", true);
}
