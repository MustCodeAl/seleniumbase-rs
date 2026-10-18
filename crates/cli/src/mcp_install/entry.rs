//! What gets registered: the entries `sbrs-mcp` asks a client to start, and how an entry
//! already in a client's config compares with one.

use std::collections::BTreeMap;

/// How a client starts the server: the program, its arguments and its environment.
///
/// Two registrations are the same when they start the same process, whichever extra keys a
/// client keeps beside these (a timeout, a trust flag, `"type": "stdio"`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchSpec {
    /// The program: the absolute path of this binary.
    pub command: String,
    /// Its arguments.
    pub args: Vec<String>,
    /// Environment variables for the server (`SB_MCP_OUTPUT_DIR`).
    pub env: BTreeMap<String, String>,
}

/// A named registration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    /// The name the client lists the server under.
    pub name: String,
    /// What it starts.
    pub spec: LaunchSpec,
}

/// The entries a client holds, by name. `None` is an entry that is not a stdio server this
/// program can read, which counts as different from every spec.
pub type Existing = BTreeMap<String, Option<LaunchSpec>>;

/// Which of the two entries a command touches.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Only {
    /// The browser server and the development server.
    Both,
    /// The browser server (`NAME`).
    Browser,
    /// The development server (`NAME-dev`).
    Dev,
}

impl Only {
    fn browser(self) -> bool {
        self != Self::Dev
    }

    fn dev(self) -> bool {
        self != Self::Browser
    }
}

/// The name of the development entry that goes with the browser entry `name`.
pub fn dev_name(name: &str) -> String {
    format!("{name}-dev")
}

/// The names `only` selects, browser entry first.
pub fn names(name: &str, only: Only) -> Vec<String> {
    let mut names = Vec::new();
    if only.browser() {
        names.push(name.to_owned());
    }
    if only.dev() {
        names.push(dev_name(name));
    }
    names
}

/// What `install` registers.
#[derive(Clone, Copy, Debug)]
pub struct PlanInput<'a> {
    /// The absolute path of the running binary.
    pub exe: &'a str,
    /// The name of the browser entry.
    pub name: &'a str,
    /// The browser toolset: `cdp`, `driver` or `sb`.
    pub browser: &'a str,
    /// The folder the tools write their files to (`SB_MCP_OUTPUT_DIR`).
    pub output_dir: &'a str,
    /// `--plugin` values for the browser server.
    pub plugins: &'a [String],
    /// `--plugin-opt` values for the browser server.
    pub plugin_opts: &'a [String],
    /// Which entries to register.
    pub only: Only,
}

/// The entries to register, browser entry first. The development server starts no browser,
/// so it gets no plugin arguments.
pub fn plan(input: &PlanInput<'_>) -> Vec<Entry> {
    let env = BTreeMap::from([("SB_MCP_OUTPUT_DIR".to_owned(), input.output_dir.to_owned())]);
    let mut entries = Vec::new();
    if input.only.browser() {
        let mut args = vec!["--server".to_owned(), input.browser.to_owned()];
        for plugin in input.plugins {
            args.push("--plugin".to_owned());
            args.push(plugin.clone());
        }
        for option in input.plugin_opts {
            args.push("--plugin-opt".to_owned());
            args.push(option.clone());
        }
        entries.push(Entry {
            name: input.name.to_owned(),
            spec: LaunchSpec {
                command: input.exe.to_owned(),
                args,
                env: env.clone(),
            },
        });
    }
    if input.only.dev() {
        entries.push(Entry {
            name: dev_name(input.name),
            spec: LaunchSpec {
                command: input.exe.to_owned(),
                args: vec!["--server".to_owned(), "dev".to_owned()],
                env,
            },
        });
    }
    entries
}

/// What to do with one entry on install.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Decision {
    /// Nothing of that name is there: add it.
    Add,
    /// A different entry is there and `--force` was given: replace it.
    Replace,
    /// The same entry is there already.
    Unchanged,
    /// A different entry is there and `--force` was not given: leave it.
    Conflict,
}

/// Compares the entry already there (`existing`, `None` when the name is free) with `desired`.
pub fn decide(
    existing: Option<&Option<LaunchSpec>>,
    desired: &LaunchSpec,
    force: bool,
) -> Decision {
    match existing {
        None => Decision::Add,
        Some(Some(spec)) if spec == desired => Decision::Unchanged,
        Some(_) if force => Decision::Replace,
        Some(_) => Decision::Conflict,
    }
}
