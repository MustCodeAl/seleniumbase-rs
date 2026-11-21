//! `sbrs-mcp upgrade`: rewrites the entries the MCP clients hold to this program, then says how
//! to upgrade the program itself and, with `--run`, does it.
//!
//! The entries are rewritten through the editors `install` uses (`apply::perform` with a forced
//! install of the entry `install` would write today). The program is upgraded by the tool that
//! installed it: Homebrew, `cargo install`, or by hand from a release archive. The command to
//! run goes through the [`Runner`](super::runner::Runner) of the [`Env`], so a test sees the
//! command and never starts it.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use super::apply::{perform, registered, Job, Scope, Task};
use super::client::Client;
use super::entry::{names, plan, Entry, LaunchSpec, Only, PlanInput};
use super::env::{homebrew_prefix, Env};
use super::external::Cmd;
use super::report::{exit_code, ClientReport, Problem};
use super::{check_name, parse_clients, print_reports};

/// What `upgrade` was asked for.
#[derive(Clone, Debug)]
pub struct UpgradeRequest {
    /// The client names, as typed; every client with a config when empty.
    pub clients: Vec<String>,
    /// The path to register for this program.
    pub exe: PathBuf,
    /// The name of the browser entry; the development entry is `NAME-dev`.
    pub name: String,
    /// Show what would be rewritten and run, and do neither.
    pub dry_run: bool,
    /// Run the command that upgrades the program; without it the command is only printed.
    pub run: bool,
}

/// How this program was installed, which decides the command that upgrades it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallMethod {
    /// Homebrew: `brew upgrade sbrs`.
    Homebrew,
    /// `cargo install`: the program is in `CARGO_HOME/bin`.
    Cargo,
    /// A release archive, or anything else: download the new release.
    Archive,
}

/// How the program at `exe` was installed.
pub fn detect_method(env: &Env, exe: &Path) -> InstallMethod {
    let exe = fs::canonicalize(exe).unwrap_or_else(|_| exe.to_path_buf());
    if homebrew_prefix(&exe).is_some() {
        return InstallMethod::Homebrew;
    }
    let cargo_bin = env.cargo_home.join("bin");
    let cargo_bin = fs::canonicalize(&cargo_bin).unwrap_or(cargo_bin);
    if exe.starts_with(&cargo_bin) {
        return InstallMethod::Cargo;
    }
    if brew_lists_sbrs(env) {
        return InstallMethod::Homebrew;
    }
    InstallMethod::Archive
}

/// Whether `brew list sbrs` succeeds: Homebrew has the `sbrs` formula installed.
fn brew_lists_sbrs(env: &Env) -> bool {
    env.runner
        .output(&tool(env, "brew", &["list", "sbrs"]))
        .is_some()
}

/// The command `name` with `args`, started from the program of that name on `PATH` when it is
/// there.
fn tool(env: &Env, name: &str, args: &[&str]) -> Cmd {
    Cmd {
        program: env
            .find_program(name)
            .unwrap_or_else(|| PathBuf::from(name)),
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        cwd: None,
    }
}

/// Rewrites the entries, prints how to upgrade the program, runs the command when asked, and
/// returns the exit status: 0 when it worked, 1 when a write or the command failed, 2 when
/// something was refused or there is nothing to run.
pub fn upgrade(
    env: &Env,
    request: &UpgradeRequest,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    if let Err(status) = check_name(&request.name, err) {
        return status;
    }
    let Some(exe) = request.exe.to_str() else {
        let _ = writeln!(err, "sbrs-mcp: the program path must be valid UTF-8");
        return 2;
    };
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
    let _ = writeln!(out, "sbrs-mcp {} at {exe}", env!("CARGO_PKG_VERSION"));
    let reports = rewrite_entries(env, request, exe, &clients);
    if reports.is_empty() {
        let _ = writeln!(
            out,
            "no client holds an entry called {name} or {name}-dev: nothing to rewrite",
            name = request.name
        );
    }
    print_reports(&reports, request.dry_run, out, err);
    exit_code(&reports).max(upgrade_program(env, request, out, err))
}

/// Rewrites the entries `NAME` and `NAME-dev` of each config that holds them, and returns what
/// became of each config. An entry that does not start a local program is not rewritten; a
/// config that cannot be read is refused.
fn rewrite_entries(
    env: &Env,
    request: &UpgradeRequest,
    exe: &str,
    clients: &[Client],
) -> Vec<ClientReport> {
    let wanted = names(&request.name, Only::Both);
    let mut reports = Vec::new();
    for client in clients {
        for scope in [Scope::User, Scope::Project] {
            let path = match scope {
                Scope::User => client.user_path(env),
                Scope::Project => client.project_path(env),
            };
            let Some(path) = path.filter(|path| path.exists()) else {
                continue;
            };
            let job = Job {
                env,
                scope,
                dry_run: request.dry_run,
            };
            match registered(env, *client, scope) {
                Err(why) => {
                    let mut report = ClientReport::new(*client);
                    report.location = path.display().to_string();
                    report.problem = Some(Problem::refused(format!(
                        "{why}, so sbrs-mcp does not rewrite it"
                    )));
                    reports.push(report);
                }
                Ok(existing) => {
                    let entries: Vec<Entry> = wanted
                        .iter()
                        .filter_map(|name| {
                            let spec = existing.get(name)?.as_ref()?;
                            Some(upgraded(name, &request.name, spec, exe))
                        })
                        .collect();
                    if !entries.is_empty() {
                        let task = Task::Install {
                            entries: &entries,
                            force: true,
                        };
                        reports.push(perform(&job, *client, &task));
                    }
                }
            }
        }
    }
    reports
}

/// The entry `name` (`base` or `base-dev`) as `install` writes it today for the program at
/// `exe`, with the server, plugins and output folder `spec` has. When `spec` has arguments or
/// an environment `install` would not have written, they stay and only the program changes.
fn upgraded(name: &str, base: &str, spec: &LaunchSpec, exe: &str) -> Entry {
    let kept = || Entry {
        name: name.to_owned(),
        spec: LaunchSpec {
            command: exe.to_owned(),
            ..spec.clone()
        },
    };
    let only = if name == base {
        Only::Browser
    } else {
        Only::Dev
    };
    let Some(shape) = Shape::of(spec, only) else {
        return kept();
    };
    plan(&PlanInput {
        exe,
        name: base,
        browser: &shape.browser,
        output_dir: &shape.output_dir,
        plugins: &shape.plugins,
        plugin_opts: &shape.plugin_opts,
        only,
    })
    .into_iter()
    .next()
    .unwrap_or_else(kept)
}

/// The options of an entry that has the shape `install` writes.
struct Shape {
    browser: String,
    plugins: Vec<String>,
    plugin_opts: Vec<String>,
    output_dir: String,
}

impl Shape {
    /// The options of `spec`, or `None` when it is not what `install` writes for `only`: other
    /// environment variables, other arguments, or a server the entry should not start.
    fn of(spec: &LaunchSpec, only: Only) -> Option<Self> {
        if spec.env.len() != 1 {
            return None;
        }
        let output_dir = spec.env.get("SB_MCP_OUTPUT_DIR")?.clone();
        let mut args = spec.args.iter();
        if args.next()? != "--server" {
            return None;
        }
        let browser = args.next()?.clone();
        let mut plugins = Vec::new();
        let mut plugin_opts = Vec::new();
        while let Some(flag) = args.next() {
            let value = args.next()?.clone();
            match flag.as_str() {
                "--plugin" => plugins.push(value),
                "--plugin-opt" => plugin_opts.push(value),
                _ => return None,
            }
        }
        let fits = match only {
            Only::Dev => browser == "dev" && plugins.is_empty() && plugin_opts.is_empty(),
            Only::Browser | Only::Both => matches!(browser.as_str(), "cdp" | "driver" | "sb"),
        };
        fits.then_some(Self {
            browser,
            plugins,
            plugin_opts,
            output_dir,
        })
    }
}

/// The repository, for the command that installs a release of it.
const REPOSITORY: &str = "https://github.com/MustCodeAl/seleniumbase-rs";

/// The page of the releases, for a program that no tool upgrades.
const RELEASES: &str = "https://github.com/MustCodeAl/seleniumbase-rs/releases";

/// How a program is upgraded: said, and run when asked.
struct HowTo {
    /// How the program was installed, as a sentence.
    headline: &'static str,
    /// The command that upgrades it, when there is one to run.
    command: Option<Cmd>,
    /// What to do by hand when there is no command.
    by_hand: String,
    /// Why there is no command to run.
    why_not: &'static str,
}

/// Says how to upgrade this program and, with `--run` and no `--dry-run`, runs the command.
/// Returns the exit status of this part.
fn upgrade_program(
    env: &Env,
    request: &UpgradeRequest,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let how = how_to_upgrade(env, detect_method(env, &request.exe), &request.exe);
    let _ = writeln!(out, "{}.", how.headline);
    let Some(command) = &how.command else {
        for line in how.by_hand.lines() {
            let _ = writeln!(out, "{line}");
        }
        if request.run {
            let _ = writeln!(err, "sbrs-mcp: --run has nothing to run: {}", how.why_not);
            return 2;
        }
        return 0;
    };
    if !request.run {
        let _ = writeln!(out, "To upgrade it, run:\n  {}", command.display());
        return 0;
    }
    if request.dry_run {
        let _ = writeln!(out, "would run: {}", command.display());
        return 0;
    }
    let _ = writeln!(out, "running: {}", command.display());
    match env.runner.run(command) {
        Ok(()) => {
            let _ = writeln!(
                out,
                "upgraded; restart the clients that use sbrs-mcp so that they start the new version"
            );
            0
        }
        Err(message) => {
            let _ = writeln!(err, "sbrs-mcp: {message}");
            1
        }
    }
}

fn how_to_upgrade(env: &Env, method: InstallMethod, exe: &Path) -> HowTo {
    match method {
        InstallMethod::Homebrew => HowTo {
            headline: "sbrs-mcp is installed with Homebrew",
            command: Some(tool(env, "brew", &["upgrade", "sbrs"])),
            by_hand: String::new(),
            why_not: "",
        },
        InstallMethod::Cargo => cargo_install(env),
        InstallMethod::Archive => HowTo {
            headline: "sbrs-mcp was installed neither with Homebrew nor with cargo install",
            command: None,
            by_hand: format!(
                "Download the latest release from {RELEASES} and replace {}.",
                exe.display()
            ),
            why_not: "a release archive has no command that upgrades it",
        },
    }
}

/// `cargo install` of the latest release tag; without the tag, the command with a placeholder
/// for it and no command to run.
fn cargo_install(env: &Env) -> HowTo {
    let headline = "sbrs-mcp is installed with cargo install";
    let Some(tag) = latest_tag(env) else {
        return HowTo {
            headline,
            command: None,
            by_hand: format!(
                "The latest release tag could not be looked up here; find it at {RELEASES}, \
                 then run:\n  cargo install --git {REPOSITORY} --tag <latest tag> \
                 seleniumbase-rs-cli --locked"
            ),
            why_not: "the latest release tag could not be looked up",
        };
    };
    HowTo {
        headline,
        command: Some(tool(
            env,
            "cargo",
            &[
                "install",
                "--git",
                REPOSITORY,
                "--tag",
                &tag,
                "seleniumbase-rs-cli",
                "--locked",
            ],
        )),
        by_hand: String::new(),
        why_not: "",
    }
}

/// The newest release tag of the repository, from `git ls-remote`; `None` without `git`, a
/// network or a tag. The lookup gives up on a connection that stalls.
fn latest_tag(env: &Env) -> Option<String> {
    let listing = env.runner.output(&tool(
        env,
        "git",
        &[
            "-c",
            "http.lowSpeedLimit=1000",
            "-c",
            "http.lowSpeedTime=15",
            "ls-remote",
            "--tags",
            "--refs",
            REPOSITORY,
        ],
    ))?;
    latest_release(&listing)
}

/// The highest release tag (`vX.Y.Z`, no prerelease) in the output of `git ls-remote --tags`.
fn latest_release(refs: &str) -> Option<String> {
    refs.lines()
        .filter_map(|line| line.split_once('\t')?.1.strip_prefix("refs/tags/"))
        .filter_map(|tag| Some((release_number(tag)?, tag)))
        .max_by_key(|(number, _)| *number)
        .map(|(_, tag)| tag.to_owned())
}

/// The three numbers of a tag such as `v0.8.10`; `None` for any other tag.
fn release_number(tag: &str) -> Option<[u64; 3]> {
    let mut parts = tag.strip_prefix('v')?.split('.');
    let number = [
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    ];
    parts.next().is_none().then_some(number)
}
