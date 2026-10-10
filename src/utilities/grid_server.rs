//! Starting and stopping a Selenium Grid hub or node.
//!
//! Python SeleniumBase has `sbase grid-hub` and `sbase grid-node`, which run
//! the Selenium Server jar as a background process and stop it again. This is
//! the same for Rust, with one deliberate difference: the jar is never
//! downloaded. You supply a `selenium-server-*.jar` (from the Selenium
//! releases page) and a Java runtime, and this module runs
//! `java -jar JAR hub` or `java -jar JAR node --hub URL` for you.
//!
//! - [`GridLaunch`] describes the command line.
//! - [`GridProcess`] starts it detached, remembers the process in a small JSON
//!   record, and later finds and stops it. A record whose process is gone, or
//!   whose process id now belongs to something else, is recognised and never
//!   killed.
//!
//! The record and the server's log live in a state directory (see
//! [`default_state_dir`]).
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::utilities::grid_server::GridLaunch;
//!
//! let launch = GridLaunch::hub("selenium-server.jar").port(4445);
//! let args: Vec<_> = launch.args().iter().map(|a| a.to_string_lossy().into_owned()).collect();
//! assert_eq!(&args[..5], ["-jar", "selenium-server.jar", "hub", "--port", "4445"]);
//! ```

use std::ffi::OsString;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::selenium_grid::{GridUrl, DEFAULT_HUB_PORT};
use crate::error::SeleniumBaseError;

/// The port a Grid node listens on unless told otherwise.
pub const DEFAULT_NODE_PORT: u16 = 5555;

/// How often process state is checked while waiting for it to change.
const POLL: Duration = Duration::from_millis(50);

/// Which part of a Grid to run.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GridRole {
    /// The hub, which tests connect to.
    Hub,
    /// A node, which runs browsers for a hub.
    Node,
}

impl GridRole {
    /// The Selenium Server sub-command: `hub` or `node`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Hub => "hub",
            Self::Node => "node",
        }
    }

    /// The port this role listens on by default.
    #[must_use]
    pub const fn default_port(self) -> u16 {
        match self {
            Self::Hub => DEFAULT_HUB_PORT,
            Self::Node => DEFAULT_NODE_PORT,
        }
    }
}

/// The command line of a Selenium Grid hub or node.
///
/// Build it with [`hub`](Self::hub) or [`node`](Self::node) and adjust it with
/// the other methods; [`args`](Self::args) and [`command`](Self::command)
/// show what would run, and [`GridProcess::start`] runs it.
#[derive(Clone, Debug)]
pub struct GridLaunch {
    role: GridRole,
    jar: PathBuf,
    java: PathBuf,
    host: Option<String>,
    port: u16,
    hub: Option<GridUrl>,
    session_timeout: Option<Duration>,
    verbose: bool,
    extra_args: Vec<OsString>,
}

impl GridLaunch {
    fn new(role: GridRole, jar: PathBuf, hub: Option<GridUrl>) -> Self {
        Self {
            role,
            jar,
            java: PathBuf::from("java"),
            host: None,
            port: role.default_port(),
            hub,
            session_timeout: None,
            verbose: false,
            extra_args: Vec::new(),
        }
    }

    /// A hub, run from the Selenium Server jar at `jar`.
    #[must_use]
    pub fn hub(jar: impl Into<PathBuf>) -> Self {
        Self::new(GridRole::Hub, jar.into(), None)
    }

    /// A node that registers with `hub`.
    #[must_use]
    pub fn node(jar: impl Into<PathBuf>, hub: GridUrl) -> Self {
        Self::new(GridRole::Node, jar.into(), Some(hub))
    }

    /// The Java executable to run. The default, `java`, is looked up on `PATH`.
    #[must_use]
    pub fn java(mut self, java: impl Into<PathBuf>) -> Self {
        self.java = java.into();
        self
    }

    /// The address to listen on; by default the server decides.
    #[must_use]
    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.host = Some(host.into());
        self
    }

    /// The port to listen on (4444 for a hub, 5555 for a node by default).
    #[must_use]
    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Closes sessions that have been idle this long (`--session-timeout`).
    #[must_use]
    pub fn session_timeout(mut self, timeout: Duration) -> Self {
        self.session_timeout = Some(timeout);
        self
    }

    /// Logs at `INFO` instead of `WARNING`.
    #[must_use]
    pub fn verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// More arguments for the Selenium Server, put last.
    #[must_use]
    pub fn extra_args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.extra_args = args.into_iter().map(Into::into).collect();
        self
    }

    /// Whether this is a hub or a node.
    #[must_use]
    pub fn role(&self) -> GridRole {
        self.role
    }

    /// The jar that will run.
    #[must_use]
    pub fn jar(&self) -> &Path {
        &self.jar
    }

    /// The Java executable that will run.
    #[must_use]
    pub fn java_path(&self) -> &Path {
        &self.java
    }

    /// The port it listens on.
    #[must_use]
    pub fn listen_port(&self) -> u16 {
        self.port
    }

    /// The address to ask for `/status` once it is running: this machine, or
    /// the `--host` it was told to bind when that names one.
    #[must_use]
    pub fn local_address(&self) -> GridUrl {
        let host = match self.host.as_deref() {
            None | Some("0.0.0.0" | "::") => "127.0.0.1",
            Some(host) => host,
        };
        GridUrl::new(host, self.port)
    }

    /// The arguments after the Java executable.
    #[must_use]
    pub fn args(&self) -> Vec<OsString> {
        let mut args: Vec<OsString> = vec![
            "-jar".into(),
            self.jar.clone().into_os_string(),
            self.role.as_str().into(),
        ];
        if let Some(host) = &self.host {
            args.extend(["--host".into(), host.into()]);
        }
        args.extend(["--port".into(), self.port.to_string().into()]);
        if let Some(hub) = &self.hub {
            args.extend(["--hub".into(), hub.origin().into()]);
        }
        if let Some(timeout) = self.session_timeout {
            args.extend([
                "--session-timeout".into(),
                timeout.as_secs().to_string().into(),
            ]);
        }
        args.extend([
            "--log-level".into(),
            if self.verbose { "INFO" } else { "WARNING" }.into(),
        ]);
        args.extend(self.extra_args.iter().cloned());
        args
    }

    /// The command to run, not yet started.
    #[must_use]
    pub fn command(&self) -> Command {
        let mut command = Command::new(&self.java);
        command.args(self.args());
        command
    }
}

/// What is remembered about a running Grid process.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GridRecord {
    /// Hub or node.
    pub role: GridRole,
    /// The operating-system process id.
    pub pid: u32,
    /// The jar it was started from.
    pub jar: PathBuf,
    /// The port it was told to listen on.
    pub port: u16,
    /// When it was started, in seconds since the Unix epoch.
    pub started_at: u64,
    /// The file its output goes to.
    pub log: PathBuf,
}

/// Whether a recorded process is still the Grid that was started.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RunState {
    /// Nothing was started, or it was stopped.
    NotRunning,
    /// The recorded process is running.
    Running(GridRecord),
    /// A record exists, but its process is gone or its id now belongs to
    /// another program.
    Stale(GridRecord),
}

/// What [`GridProcess::stop`] did.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StopOutcome {
    /// There was nothing to stop.
    NotRunning,
    /// The process was stopped.
    Stopped(GridRecord),
    /// The record was out of date; it was removed and no process was touched.
    StaleRemoved(GridRecord),
}

/// The Grid process of one role, found through its record in a state directory.
#[derive(Clone, Debug)]
pub struct GridProcess {
    dir: PathBuf,
    role: GridRole,
}

impl GridProcess {
    /// The hub or node whose record is kept in `state_dir`.
    #[must_use]
    pub fn new(state_dir: impl Into<PathBuf>, role: GridRole) -> Self {
        Self {
            dir: state_dir.into(),
            role,
        }
    }

    /// The file the process record is kept in.
    #[must_use]
    pub fn record_path(&self) -> PathBuf {
        self.dir.join(format!("grid-{}.json", self.role.as_str()))
    }

    /// The file the process writes its output to.
    #[must_use]
    pub fn log_path(&self) -> PathBuf {
        self.dir.join(format!("grid-{}.log", self.role.as_str()))
    }

    /// Whether the recorded process is running.
    ///
    /// # Errors
    ///
    /// Returns an error if the record exists but cannot be read or parsed, or
    /// if the system cannot be asked about processes.
    pub fn state(&self) -> Result<RunState, SeleniumBaseError> {
        let path = self.record_path();
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(RunState::NotRunning);
            }
            Err(error) => return Err(error.into()),
        };
        let record: GridRecord = serde_json::from_str(&text).map_err(|error| {
            SeleniumBaseError::invalid_config(format!(
                "the Grid record {} is damaged ({error}); delete it",
                path.display()
            ))
        })?;
        Ok(if is_ours(&record)? {
            RunState::Running(record)
        } else {
            RunState::Stale(record)
        })
    }

    /// Starts the Grid in the background and returns its record.
    ///
    /// The process's output goes to [`log_path`](Self::log_path). `settle` is
    /// how long to watch for it dying straight away, which is what a bad jar,
    /// a missing Java or a busy port look like; in that case the error carries
    /// the end of the log.
    ///
    /// # Errors
    ///
    /// Returns an error if this role is already running, the jar is not a
    /// file, the port is 0, Java cannot be started, the process exits during
    /// `settle`, or the record cannot be written.
    pub fn start(
        &self,
        launch: &GridLaunch,
        settle: Duration,
    ) -> Result<GridRecord, SeleniumBaseError> {
        if launch.role != self.role {
            return Err(SeleniumBaseError::invalid_config(
                "the launch is for a different Grid role than this process",
            ));
        }
        if launch.port == 0 {
            return Err(SeleniumBaseError::invalid_config(
                "the Grid port must not be 0",
            ));
        }
        if !launch.jar.is_file() {
            return Err(SeleniumBaseError::invalid_config(format!(
                "the Selenium Server jar {} does not exist; pass its path (it is not downloaded)",
                launch.jar.display()
            )));
        }
        match self.state()? {
            RunState::Running(record) => {
                return Err(SeleniumBaseError::invalid_config(format!(
                    "the Grid {} is already running (process {}); stop it or restart",
                    self.role.as_str(),
                    record.pid
                )));
            }
            RunState::Stale(_) => fs::remove_file(self.record_path())?,
            RunState::NotRunning => {}
        }

        fs::create_dir_all(&self.dir)?;
        let log_path = self.log_path();
        let log = File::create(&log_path)?;
        let mut command = launch.command();
        command
            .stdin(Stdio::null())
            .stdout(log.try_clone()?)
            .stderr(log);
        detach(&mut command);
        let mut child = spawn_patiently(&mut command).map_err(|error| {
            SeleniumBaseError::browser_launch(
                launch.java.display().to_string(),
                format!("could not start Java ({error}); is a Java runtime installed?"),
            )
        })?;

        let deadline = Instant::now() + settle;
        while Instant::now() < deadline {
            if let Some(status) = child.try_wait()? {
                return Err(SeleniumBaseError::browser_launch(
                    launch.java.display().to_string(),
                    format!(
                        "the Grid {} exited at once ({status}). End of {}:\n{}",
                        self.role.as_str(),
                        log_path.display(),
                        log_tail(&log_path, 15)
                    ),
                ));
            }
            std::thread::sleep(POLL);
        }

        let record = GridRecord {
            role: self.role,
            pid: child.id(),
            jar: launch.jar.clone(),
            port: launch.port,
            started_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_secs()),
            log: log_path,
        };
        if let Err(error) = self.write_record(&record) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        // Collect the exit status when the process ends, so it does not linger
        // as a zombie while this program keeps running.
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(record)
    }

    /// Stops the recorded process and removes its record.
    ///
    /// The process is asked to end and given `grace` to do so, then forced. A
    /// process is stopped only if it still looks like the Grid that was
    /// started; a record that does not match is removed without touching
    /// anything.
    ///
    /// # Errors
    ///
    /// Returns an error if the record cannot be read, the process cannot be
    /// signalled, or it is still there after being forced.
    pub fn stop(&self, grace: Duration) -> Result<StopOutcome, SeleniumBaseError> {
        let record = match self.state()? {
            RunState::NotRunning => return Ok(StopOutcome::NotRunning),
            RunState::Stale(record) => {
                fs::remove_file(self.record_path())?;
                return Ok(StopOutcome::StaleRemoved(record));
            }
            RunState::Running(record) => record,
        };
        signal(record.pid, false)?;
        if !wait_until_gone(&record, grace)? {
            signal(record.pid, true)?;
            if !wait_until_gone(&record, Duration::from_secs(5))? {
                return Err(SeleniumBaseError::browser_launch(
                    format!("process {}", record.pid),
                    "the Grid did not stop even when forced",
                ));
            }
        }
        fs::remove_file(self.record_path())?;
        Ok(StopOutcome::Stopped(record))
    }

    fn write_record(&self, record: &GridRecord) -> Result<(), SeleniumBaseError> {
        let path = self.record_path();
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(record)?)?;
        fs::rename(&temporary, &path)?;
        Ok(())
    }
}

/// The directory that holds Grid records and logs unless one is chosen.
///
/// The `SB_GRID_DIR` environment variable, else a `seleniumbase-rs/grid`
/// folder in the user's state (or local data) directory.
#[must_use]
pub fn default_state_dir() -> PathBuf {
    state_dir_from(std::env::var_os("SB_GRID_DIR"))
}

fn state_dir_from(configured: Option<OsString>) -> PathBuf {
    if let Some(dir) = configured.filter(|dir| !dir.is_empty()) {
        return PathBuf::from(dir);
    }
    dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("seleniumbase-rs")
        .join("grid")
}

/// Spawns `command`, trying again a few times if the executable is busy.
///
/// A program that was written a moment ago can be reported busy while another
/// thread's `fork` still holds the descriptor it was written through; the
/// condition clears as soon as that child runs `exec`.
fn spawn_patiently(command: &mut Command) -> std::io::Result<std::process::Child> {
    let mut attempts = 0;
    loop {
        match command.spawn() {
            Err(error)
                if error.kind() == std::io::ErrorKind::ExecutableFileBusy && attempts < 10 =>
            {
                attempts += 1;
                std::thread::sleep(POLL);
            }
            other => return other,
        }
    }
}

/// The last `lines` lines of a text file, or a note if it cannot be read.
fn log_tail(path: &Path, lines: usize) -> String {
    let mut text = String::new();
    match File::open(path).and_then(|mut file| file.read_to_string(&mut text)) {
        Ok(_) => {
            let all: Vec<&str> = text.lines().collect();
            let start = all.len().saturating_sub(lines);
            let tail = all[start..].join("\n");
            if tail.trim().is_empty() {
                "(the log is empty)".to_owned()
            } else {
                tail
            }
        }
        Err(error) => format!("(the log could not be read: {error})"),
    }
}

/// Waits for `record`'s process to stop being the Grid. Returns whether it did.
fn wait_until_gone(record: &GridRecord, within: Duration) -> Result<bool, SeleniumBaseError> {
    let deadline = Instant::now() + within;
    loop {
        if !is_ours(record)? {
            return Ok(true);
        }
        if Instant::now() >= deadline {
            return Ok(false);
        }
        std::thread::sleep(POLL);
    }
}

/// Puts the child in its own process group (Unix) or console-less group
/// (Windows), so a Ctrl-C in the terminal that ran `sbase` does not stop it.
fn detach(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = command;
    }
}

/// Whether `record`'s process id is a live process that looks like the Grid.
///
/// On Unix the command line must contain the jar's file name and the role. On
/// Windows only the executable name (`java`) can be checked.
fn is_ours(record: &GridRecord) -> Result<bool, SeleniumBaseError> {
    let Some(command) = process_command(record.pid)? else {
        return Ok(false);
    };
    #[cfg(unix)]
    {
        let jar = record
            .jar
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(!jar.is_empty() && command.contains(&jar) && command.contains(record.role.as_str()))
    }
    #[cfg(not(unix))]
    {
        Ok(command.to_ascii_lowercase().contains("java"))
    }
}

/// The command line (Unix) or image name (Windows) of a live process.
#[cfg(unix)]
fn process_command(pid: u32) -> Result<Option<String>, SeleniumBaseError> {
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "command="])
        .output()
        .map_err(|error| {
            SeleniumBaseError::Unsupported(format!("cannot list processes with `ps`: {error}"))
        })?;
    let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    Ok((output.status.success() && !text.is_empty()).then_some(text))
}

/// The command line (Unix) or image name (Windows) of a live process.
#[cfg(not(unix))]
fn process_command(pid: u32) -> Result<Option<String>, SeleniumBaseError> {
    let output = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
        .output()
        .map_err(|error| {
            SeleniumBaseError::Unsupported(format!(
                "cannot list processes with `tasklist`: {error}"
            ))
        })?;
    let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let listed = text.contains(&format!("\"{pid}\""));
    Ok(listed.then_some(text))
}

/// Asks a process to end (`force` false) or makes it end (`force` true).
fn signal(pid: u32, force: bool) -> Result<(), SeleniumBaseError> {
    #[cfg(unix)]
    let status = Command::new("kill")
        .arg(if force { "-KILL" } else { "-TERM" })
        .arg(pid.to_string())
        .status();
    #[cfg(not(unix))]
    let status = {
        let mut command = Command::new("taskkill");
        command.args(["/PID", &pid.to_string(), "/T"]);
        if force {
            command.arg("/F");
        }
        command.status()
    };
    match status {
        Ok(status) if status.success() => Ok(()),
        // The process may have ended between the check and the signal.
        Ok(_) if process_command(pid)?.is_none() => Ok(()),
        Ok(status) => Err(SeleniumBaseError::browser_launch(
            format!("process {pid}"),
            format!("could not signal the Grid process ({status})"),
        )),
        Err(error) => Err(SeleniumBaseError::Unsupported(format!(
            "cannot signal processes: {error}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(args: &[OsString]) -> Vec<String> {
        args.iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn a_hub_runs_the_jar_with_its_default_port_and_quiet_logging() {
        let args = strings(&GridLaunch::hub("selenium-server.jar").args());
        assert_eq!(
            args,
            [
                "-jar",
                "selenium-server.jar",
                "hub",
                "--port",
                "4444",
                "--log-level",
                "WARNING"
            ]
        );
    }

    #[test]
    fn a_node_registers_with_the_hubs_origin_not_its_path() {
        let hub: GridUrl = "http://hub.example.com:4444/wd/hub".parse().unwrap();
        let args = strings(&GridLaunch::node("s.jar", hub).args());
        assert_eq!(
            args,
            [
                "-jar",
                "s.jar",
                "node",
                "--port",
                "5555",
                "--hub",
                "http://hub.example.com:4444",
                "--log-level",
                "WARNING"
            ]
        );
    }

    #[test]
    fn every_option_reaches_the_command_line_in_a_fixed_order() {
        let launch = GridLaunch::hub("s.jar")
            .host("0.0.0.0")
            .port(4455)
            .session_timeout(Duration::from_secs(230))
            .verbose(true)
            .extra_args(["--tracing", "false"]);
        assert_eq!(
            strings(&launch.args()),
            [
                "-jar",
                "s.jar",
                "hub",
                "--host",
                "0.0.0.0",
                "--port",
                "4455",
                "--session-timeout",
                "230",
                "--log-level",
                "INFO",
                "--tracing",
                "false"
            ]
        );
    }

    #[test]
    fn the_command_uses_the_chosen_java() {
        let command = GridLaunch::hub("s.jar").java("/opt/jdk/bin/java").command();
        assert_eq!(command.get_program(), "/opt/jdk/bin/java");
        assert_eq!(GridLaunch::hub("s.jar").command().get_program(), "java");
    }

    #[test]
    fn the_status_address_is_local_unless_a_host_was_chosen() {
        let local = GridLaunch::hub("s.jar").port(4500);
        assert_eq!(local.local_address().to_string(), "http://127.0.0.1:4500");
        let wildcard = GridLaunch::hub("s.jar").host("0.0.0.0").port(4500);
        assert_eq!(
            wildcard.local_address().to_string(),
            "http://127.0.0.1:4500"
        );
        let named = GridLaunch::hub("s.jar").host("grid.test").port(4500);
        assert_eq!(named.local_address().to_string(), "http://grid.test:4500");
    }

    #[test]
    fn roles_know_their_names_and_ports() {
        assert_eq!(GridRole::Hub.as_str(), "hub");
        assert_eq!(GridRole::Node.as_str(), "node");
        assert_eq!(GridRole::Hub.default_port(), 4444);
        assert_eq!(GridRole::Node.default_port(), 5555);
    }

    #[test]
    fn the_state_dir_is_the_configured_one_or_a_folder_of_ours() {
        assert_eq!(
            state_dir_from(Some("/var/grid".into())),
            PathBuf::from("/var/grid")
        );
        for unset in [None, Some(OsString::new())] {
            let dir = state_dir_from(unset);
            assert!(
                dir.ends_with(Path::new("seleniumbase-rs").join("grid")),
                "{dir:?}"
            );
        }
    }

    #[test]
    fn the_log_tail_keeps_the_last_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log");
        fs::write(&path, "one\ntwo\nthree\nfour\n").unwrap();
        assert_eq!(log_tail(&path, 2), "three\nfour");
        assert_eq!(log_tail(&path, 10), "one\ntwo\nthree\nfour");
        fs::write(&path, "").unwrap();
        assert_eq!(log_tail(&path, 3), "(the log is empty)");
        assert!(log_tail(&dir.path().join("none"), 3).contains("could not be read"));
    }

    #[test]
    fn a_launch_for_the_other_role_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let jar = dir.path().join("s.jar");
        fs::write(&jar, b"jar").unwrap();
        let process = GridProcess::new(dir.path().join("state"), GridRole::Node);

        let error = process
            .start(&GridLaunch::hub(&jar), Duration::from_millis(10))
            .unwrap_err();

        assert!(error.to_string().contains("different Grid role"), "{error}");
    }

    #[test]
    fn a_missing_jar_is_refused_with_a_note_that_nothing_is_downloaded() {
        let dir = tempfile::tempdir().unwrap();
        let process = GridProcess::new(dir.path(), GridRole::Hub);

        let error = process
            .start(
                &GridLaunch::hub(dir.path().join("none.jar")),
                Duration::from_millis(10),
            )
            .unwrap_err();

        assert!(error.to_string().contains("not downloaded"), "{error}");
    }

    #[test]
    fn port_zero_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let jar = dir.path().join("s.jar");
        fs::write(&jar, b"jar").unwrap();
        let process = GridProcess::new(dir.path(), GridRole::Hub);

        assert!(process
            .start(&GridLaunch::hub(&jar).port(0), Duration::from_millis(10))
            .is_err());
    }

    #[test]
    fn a_missing_java_is_a_launch_error_that_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let jar = dir.path().join("s.jar");
        fs::write(&jar, b"jar").unwrap();
        let process = GridProcess::new(dir.path(), GridRole::Hub);

        let error = process
            .start(
                &GridLaunch::hub(&jar).java(dir.path().join("no-such-java")),
                Duration::from_millis(10),
            )
            .unwrap_err();

        assert!(error.to_string().contains("Java"), "{error}");
        assert_eq!(process.state().unwrap(), RunState::NotRunning);
    }

    #[test]
    fn with_no_record_nothing_is_running_and_stopping_does_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let process = GridProcess::new(dir.path(), GridRole::Hub);
        assert_eq!(process.state().unwrap(), RunState::NotRunning);
        assert_eq!(
            process.stop(Duration::from_millis(10)).unwrap(),
            StopOutcome::NotRunning
        );
    }

    #[test]
    fn a_damaged_record_is_an_error_not_a_guess() {
        let dir = tempfile::tempdir().unwrap();
        let process = GridProcess::new(dir.path(), GridRole::Hub);
        fs::write(process.record_path(), "{ not json").unwrap();
        let error = process.state().unwrap_err();
        assert!(error.to_string().contains("damaged"), "{error}");
    }

    #[cfg(unix)]
    mod with_a_stand_in_java {
        use super::*;
        use std::os::unix::fs::PermissionsExt;

        /// A `java` that records its arguments and then waits, ending cleanly
        /// when asked to.
        fn fake_java(dir: &Path, body: &str) -> PathBuf {
            let path = dir.join("fake-java");
            fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
            path
        }

        const WAITS: &str = "echo \"fake java: $*\"\n\
                             trap 'kill $child 2>/dev/null; exit 0' TERM\n\
                             sleep 120 &\n\
                             child=$!\n\
                             wait $child";

        struct Fixture {
            _dir: tempfile::TempDir,
            jar: PathBuf,
            java: PathBuf,
            process: GridProcess,
        }

        fn fixture(role: GridRole, java_body: &str) -> Fixture {
            let dir = tempfile::tempdir().unwrap();
            let jar = dir.path().join("selenium-server-test.jar");
            fs::write(&jar, b"not a real jar").unwrap();
            let java = fake_java(dir.path(), java_body);
            let process = GridProcess::new(dir.path().join("state"), role);
            Fixture {
                _dir: dir,
                jar,
                java,
                process,
            }
        }

        #[test]
        fn a_grid_can_be_started_found_and_stopped() {
            let f = fixture(GridRole::Hub, WAITS);
            let launch = GridLaunch::hub(&f.jar).java(&f.java).port(4999);

            let record = f
                .process
                .start(&launch, Duration::from_millis(200))
                .unwrap();

            assert_eq!(record.role, GridRole::Hub);
            assert_eq!(record.port, 4999);
            assert_eq!(
                f.process.state().unwrap(),
                RunState::Running(record.clone())
            );
            let log = fs::read_to_string(f.process.log_path()).unwrap();
            assert!(log.contains("fake java: -jar"), "{log}");
            assert!(log.contains("hub --port 4999"), "{log}");

            let outcome = f.process.stop(Duration::from_secs(10)).unwrap();

            assert_eq!(outcome, StopOutcome::Stopped(record));
            assert_eq!(f.process.state().unwrap(), RunState::NotRunning);
            assert!(!f.process.record_path().exists());
        }

        #[test]
        fn a_second_start_while_running_is_refused_and_leaves_the_first_alone() {
            let f = fixture(GridRole::Hub, WAITS);
            let launch = GridLaunch::hub(&f.jar).java(&f.java);
            let first = f
                .process
                .start(&launch, Duration::from_millis(200))
                .unwrap();

            let error = f
                .process
                .start(&launch, Duration::from_millis(200))
                .unwrap_err();

            assert!(error.to_string().contains("already running"), "{error}");
            assert_eq!(f.process.state().unwrap(), RunState::Running(first));
            f.process.stop(Duration::from_secs(10)).unwrap();
        }

        #[test]
        fn a_grid_that_dies_at_once_reports_the_end_of_its_log() {
            let f = fixture(
                GridRole::Hub,
                "echo 'Exception: Address already in use' >&2\nexit 3",
            );
            let launch = GridLaunch::hub(&f.jar).java(&f.java);

            let error = f
                .process
                .start(&launch, Duration::from_secs(5))
                .unwrap_err();

            let shown = error.to_string();
            assert!(shown.contains("exited at once"), "{shown}");
            assert!(shown.contains("Address already in use"), "{shown}");
            assert_eq!(f.process.state().unwrap(), RunState::NotRunning);
        }

        #[test]
        fn a_record_whose_process_id_belongs_to_someone_else_is_never_killed() {
            let f = fixture(GridRole::Hub, WAITS);
            fs::create_dir_all(f.process.record_path().parent().unwrap()).unwrap();
            // The test process itself: alive, but not a Selenium Grid.
            let impostor = GridRecord {
                role: GridRole::Hub,
                pid: std::process::id(),
                jar: f.jar.clone(),
                port: 4444,
                started_at: 0,
                log: f.process.log_path(),
            };
            fs::write(
                f.process.record_path(),
                serde_json::to_vec(&impostor).unwrap(),
            )
            .unwrap();

            assert_eq!(
                f.process.state().unwrap(),
                RunState::Stale(impostor.clone())
            );
            let outcome = f.process.stop(Duration::from_millis(100)).unwrap();

            assert_eq!(outcome, StopOutcome::StaleRemoved(impostor));
            assert!(!f.process.record_path().exists());
            // Still here to assert it.
        }

        #[test]
        fn a_record_of_a_dead_process_is_stale_and_a_new_start_replaces_it() {
            let f = fixture(GridRole::Hub, WAITS);
            fs::create_dir_all(f.process.record_path().parent().unwrap()).unwrap();
            // Start and stop one to learn a pid that is certainly gone.
            let launch = GridLaunch::hub(&f.jar).java(&f.java);
            let old = f
                .process
                .start(&launch, Duration::from_millis(200))
                .unwrap();
            f.process.stop(Duration::from_secs(10)).unwrap();
            fs::write(f.process.record_path(), serde_json::to_vec(&old).unwrap()).unwrap();
            assert_eq!(f.process.state().unwrap(), RunState::Stale(old.clone()));

            let fresh = f
                .process
                .start(&launch, Duration::from_millis(200))
                .unwrap();

            assert_ne!(fresh.pid, old.pid);
            assert_eq!(f.process.state().unwrap(), RunState::Running(fresh));
            f.process.stop(Duration::from_secs(10)).unwrap();
        }

        #[test]
        fn a_hub_and_a_node_are_independent() {
            let hub = fixture(GridRole::Hub, WAITS);
            let node_process =
                GridProcess::new(hub.process.record_path().parent().unwrap(), GridRole::Node);
            let hub_launch = GridLaunch::hub(&hub.jar).java(&hub.java);
            let node_launch =
                GridLaunch::node(&hub.jar, "127.0.0.1".parse().unwrap()).java(&hub.java);

            hub.process
                .start(&hub_launch, Duration::from_millis(200))
                .unwrap();
            node_process
                .start(&node_launch, Duration::from_millis(200))
                .unwrap();

            assert!(matches!(hub.process.state().unwrap(), RunState::Running(_)));
            assert!(matches!(
                node_process.state().unwrap(),
                RunState::Running(_)
            ));
            node_process.stop(Duration::from_secs(10)).unwrap();
            assert!(matches!(hub.process.state().unwrap(), RunState::Running(_)));
            hub.process.stop(Duration::from_secs(10)).unwrap();
        }

        #[test]
        fn a_grid_that_ignores_the_polite_request_is_forced() {
            let f = fixture(GridRole::Hub, "trap '' TERM\nwhile true; do sleep 1; done");
            let launch = GridLaunch::hub(&f.jar).java(&f.java);
            f.process
                .start(&launch, Duration::from_millis(200))
                .unwrap();

            let outcome = f.process.stop(Duration::from_millis(300)).unwrap();

            assert!(matches!(outcome, StopOutcome::Stopped(_)));
            assert_eq!(f.process.state().unwrap(), RunState::NotRunning);
        }
    }
}
