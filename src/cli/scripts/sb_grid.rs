//! `sbase grid-hub` and `sbase grid-node`.
//!
//! Python SeleniumBase starts, stops and restarts a Selenium Grid hub or node
//! with `sbase grid-hub {start|stop|restart}` and `sbase grid-node ...`. This
//! is the same, built on [`GridProcess`]. The Selenium Server jar is supplied
//! by the user (`--jar` or `SB_SELENIUM_SERVER_JAR`) and is never downloaded;
//! `status` is an addition that reports whether the Grid is up and ready.
//!
//! [`run`] does the work and returns the lines to show, so the command can be
//! tested without a terminal.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::error::SeleniumBaseError;
use crate::utilities::grid_server::{GridLaunch, GridProcess, GridRole, RunState, StopOutcome};
use crate::utilities::selenium_grid::{fetch_status, wait_until_ready, GridUrl};

/// The environment variable that names the Selenium Server jar.
pub const JAR_ENV: &str = "SB_SELENIUM_SERVER_JAR";

/// What to do with the Grid process.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GridAction {
    /// Start it and wait until it is ready.
    Start,
    /// Stop it.
    Stop,
    /// Stop it if it is running, then start it.
    Restart,
    /// Say whether it is running and ready.
    Status,
}

/// A `grid-hub` or `grid-node` command line.
#[derive(Clone, Debug)]
pub struct GridCommand {
    /// Hub or node.
    pub role: GridRole,
    /// What to do.
    pub action: GridAction,
    /// The Selenium Server jar; `SB_SELENIUM_SERVER_JAR` is used if `None`.
    pub jar: Option<PathBuf>,
    /// The Java executable; `java` on `PATH` if `None`.
    pub java: Option<PathBuf>,
    /// The address to listen on.
    pub host: Option<String>,
    /// The port to listen on; the role's default if `None`.
    pub port: Option<u16>,
    /// For a node, the hub to register with (a host or a URL); this machine's
    /// hub if `None`.
    pub hub: Option<String>,
    /// Close sessions idle for this long.
    pub session_timeout: Option<Duration>,
    /// How long `start` waits for the Grid to become ready.
    pub wait: Duration,
    /// Log at `INFO` instead of `WARNING`.
    pub verbose: bool,
    /// More arguments for the Selenium Server.
    pub extra_args: Vec<String>,
    /// Where the process record and log are kept.
    pub state_dir: PathBuf,
    /// Start even if something already accepts connections on the port.
    ///
    /// Normally that is an error, because the readiness check would be
    /// answered by the other program. Set it when something else is meant to
    /// hold the port, such as a forwarding proxy in front of the Grid.
    pub skip_port_check: bool,
}

impl GridCommand {
    /// A command for `role` that does `action`, with every option at its
    /// default and records kept in `state_dir`.
    #[must_use]
    pub fn new(role: GridRole, action: GridAction, state_dir: impl Into<PathBuf>) -> Self {
        Self {
            role,
            action,
            jar: None,
            java: None,
            host: None,
            port: None,
            hub: None,
            session_timeout: None,
            wait: Duration::from_secs(60),
            verbose: false,
            extra_args: Vec::new(),
            state_dir: state_dir.into(),
            skip_port_check: false,
        }
    }
}

/// The jar to run: the explicit one, else the one the environment names.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidConfig`] if neither is given, saying
/// where to get a jar (nothing is downloaded).
pub fn resolve_jar(
    explicit: Option<&Path>,
    from_env: Option<OsString>,
) -> Result<PathBuf, SeleniumBaseError> {
    explicit
        .map(Path::to_path_buf)
        .or_else(|| {
            from_env
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        })
        .ok_or_else(|| {
            SeleniumBaseError::invalid_config(format!(
                "no Selenium Server jar: pass --jar PATH or set {JAR_ENV}. sbase does not \
                 download it; get selenium-server-<version>.jar from \
                 https://github.com/SeleniumHQ/selenium/releases"
            ))
        })
}

/// Runs the command and returns the lines to show the user.
///
/// # Errors
///
/// Returns an error if the jar is missing, Java cannot be started, the Grid
/// dies or does not become ready in time (it is stopped again in that case),
/// or its process cannot be found or stopped.
pub async fn run(command: &GridCommand) -> Result<Vec<String>, SeleniumBaseError> {
    let process = GridProcess::new(&command.state_dir, command.role);
    let role = command.role.as_str();
    match command.action {
        GridAction::Start => start(command, &process).await,
        GridAction::Stop => Ok(stop(&process, role)?),
        GridAction::Restart => {
            let mut lines = stop(&process, role)?;
            lines.extend(start(command, &process).await?);
            Ok(lines)
        }
        GridAction::Status => status(command, &process).await,
    }
}

fn launch_for(command: &GridCommand) -> Result<GridLaunch, SeleniumBaseError> {
    let jar = resolve_jar(command.jar.as_deref(), std::env::var_os(JAR_ENV))?;
    let mut launch = match command.role {
        GridRole::Hub => GridLaunch::hub(jar),
        GridRole::Node => {
            let hub: GridUrl = command.hub.as_deref().unwrap_or("127.0.0.1").parse()?;
            GridLaunch::node(jar, hub)
        }
    };
    if let Some(java) = &command.java {
        launch = launch.java(java);
    }
    if let Some(host) = &command.host {
        launch = launch.host(host);
    }
    if let Some(port) = command.port {
        launch = launch.port(port);
    }
    if let Some(timeout) = command.session_timeout {
        launch = launch.session_timeout(timeout);
    }
    Ok(launch
        .verbose(command.verbose)
        .extra_args(command.extra_args.clone()))
}

async fn start(
    command: &GridCommand,
    process: &GridProcess,
) -> Result<Vec<String>, SeleniumBaseError> {
    let role = command.role.as_str();
    let launch = launch_for(command)?;
    let address = launch.local_address();
    // A server already on the port would answer the readiness check on behalf
    // of a Grid that then fails to start, so refuse before starting anything.
    if !command.skip_port_check && port_in_use(&address).await {
        return Err(SeleniumBaseError::invalid_config(format!(
            "port {} on {} is already in use; stop what is using it or choose another with --port",
            address.port(),
            address.host()
        )));
    }
    let record = process.start(&launch, Duration::from_millis(500))?;

    if let Err(error) = wait_until_ready(&address, command.wait, Duration::from_millis(500)).await {
        // Do not leave a Grid that never came up running in the background.
        let _ = process.stop(Duration::from_secs(5));
        return Err(SeleniumBaseError::browser_launch(
            launch.java_path().display().to_string(),
            format!(
                "the Grid {role} did not come up: {error}. Its log is {}",
                process.log_path().display()
            ),
        ));
    }

    let mut lines = vec![format!(
        "Selenium Grid {role} started (process {}) and ready at {address}",
        record.pid
    )];
    match command.role {
        GridRole::Hub => {
            lines.push(format!("Console: {}", address.ui_url()));
            lines.push(format!(
                "Run tests against it with: sbase --webdriver {address} test"
            ));
        }
        GridRole::Node => lines.push(format!(
            "Registered with the hub at {}",
            command.hub.as_deref().unwrap_or("127.0.0.1")
        )),
    }
    lines.push(format!("Log: {}", record.log.display()));
    Ok(lines)
}

/// Whether something accepts connections at `address` right now.
async fn port_in_use(address: &GridUrl) -> bool {
    let connect = tokio::net::TcpStream::connect((address.host(), address.port()));
    matches!(
        tokio::time::timeout(Duration::from_millis(500), connect).await,
        Ok(Ok(_))
    )
}

fn stop(process: &GridProcess, role: &str) -> Result<Vec<String>, SeleniumBaseError> {
    Ok(vec![match process.stop(Duration::from_secs(10))? {
        StopOutcome::NotRunning => format!("The Grid {role} is not running."),
        StopOutcome::Stopped(record) => {
            format!("Stopped the Grid {role} (process {}).", record.pid)
        }
        StopOutcome::StaleRemoved(record) => format!(
            "The Grid {role} (process {}) was no longer running; removed its record.",
            record.pid
        ),
    }])
}

async fn status(
    command: &GridCommand,
    process: &GridProcess,
) -> Result<Vec<String>, SeleniumBaseError> {
    let role = command.role.as_str();
    Ok(match process.state()? {
        RunState::NotRunning => vec![format!("The Grid {role} is not running.")],
        RunState::Stale(record) => vec![format!(
            "The Grid {role} is not running (process {} is gone). Start it with `sbase grid-{role} start`.",
            record.pid
        )],
        RunState::Running(record) => {
            let address = GridUrl::new("127.0.0.1", record.port);
            let mut lines = vec![format!(
                "The Grid {role} is running (process {}, port {}, up {}).",
                record.pid,
                record.port,
                describe_age(record.started_at)
            )];
            lines.push(match fetch_status(&address).await {
                Ok(status) if command.role == GridRole::Hub => format!(
                    "{}: {} node(s), {} slot(s), {} in use.",
                    if status.ready { "Ready" } else { "Not ready" },
                    status.nodes,
                    status.slots,
                    status.busy_slots
                ),
                Ok(status) => (if status.ready { "Ready." } else { "Not ready." }).to_owned(),
                Err(error) => format!("It is not answering at {address}: {error}"),
            });
            lines.push(format!("Log: {}", record.log.display()));
            lines
        }
    })
}

/// "3 min", "2 h" and so on, from a start time in Unix seconds.
fn describe_age(started_at: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let secs = now.saturating_sub(started_at);
    match secs {
        0..=59 => format!("{secs} s"),
        60..=3599 => format!("{} min", secs / 60),
        _ => format!("{} h", secs / 3600),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_explicit_jar_beats_the_environment() {
        let jar = resolve_jar(Some(Path::new("a.jar")), Some("b.jar".into())).unwrap();
        assert_eq!(jar, Path::new("a.jar"));
        let from_env = resolve_jar(None, Some("b.jar".into())).unwrap();
        assert_eq!(from_env, Path::new("b.jar"));
    }

    #[test]
    fn no_jar_is_an_error_that_says_nothing_is_downloaded() {
        for env in [None, Some(OsString::new())] {
            let error = resolve_jar(None, env).unwrap_err().to_string();
            assert!(error.contains("--jar"), "{error}");
            assert!(error.contains(JAR_ENV), "{error}");
            assert!(error.contains("does not download"), "{error}");
        }
    }

    #[test]
    fn ages_are_given_in_the_largest_whole_unit() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert_eq!(describe_age(now - 5), "5 s");
        assert_eq!(describe_age(now - 300), "5 min");
        assert_eq!(describe_age(now - 7300), "2 h");
        assert_eq!(
            describe_age(now + 100),
            "0 s",
            "a clock step back is not an error"
        );
    }

    #[cfg(unix)]
    mod with_a_stand_in_java {
        use super::*;
        use serde_json::json;
        use std::os::unix::fs::PermissionsExt;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        /// Answers every request with a ready hub status listing one node.
        async fn serve_ready() -> u16 {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            let body = json!({ "value": {
                "ready": true, "message": "Selenium Grid ready.",
                "nodes": [ { "slots": [ { "session": null }, { "session": {} } ] } ]
            }})
            .to_string();
            tokio::spawn(async move {
                while let Ok((mut socket, _)) = listener.accept().await {
                    let mut buffer = [0_u8; 1024];
                    let _ = socket.read(&mut buffer).await;
                    let reply = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = socket.write_all(reply.as_bytes()).await;
                    let _ = socket.shutdown().await;
                }
            });
            port
        }

        struct Fixture {
            _dir: tempfile::TempDir,
            command: GridCommand,
        }

        fn fixture(role: GridRole, action: GridAction, java_body: &str) -> Fixture {
            let dir = tempfile::tempdir().unwrap();
            let jar = dir.path().join("selenium-server-test.jar");
            std::fs::write(&jar, b"not a real jar").unwrap();
            let java = dir.path().join("fake-java");
            std::fs::write(&java, format!("#!/bin/sh\n{java_body}\n")).unwrap();
            std::fs::set_permissions(&java, std::fs::Permissions::from_mode(0o755)).unwrap();
            let mut command = GridCommand::new(role, action, dir.path().join("state"));
            command.jar = Some(jar);
            command.java = Some(java);
            command.wait = Duration::from_secs(10);
            // Most tests have the stand-in status server running before the
            // Grid starts, which the port check would (rightly) refuse.
            command.skip_port_check = true;
            Fixture { _dir: dir, command }
        }

        /// A port that nothing listens on right now.
        async fn free_port() -> u16 {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            listener.local_addr().unwrap().port()
        }

        /// Starts answering on `port` shortly from now, as a Grid does once
        /// its JVM is up.
        fn serve_ready_on_later(port: u16) {
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(200)).await;
                let listener = TcpListener::bind(("127.0.0.1", port)).await.unwrap();
                let body = r#"{"value":{"ready":true,"message":"ready"}}"#;
                while let Ok((mut socket, _)) = listener.accept().await {
                    let mut buffer = [0_u8; 1024];
                    let _ = socket.read(&mut buffer).await;
                    let reply = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = socket.write_all(reply.as_bytes()).await;
                    let _ = socket.shutdown().await;
                }
            });
        }

        const WAITS: &str = "echo \"fake java: $*\"\n\
                             trap 'kill $child 2>/dev/null; exit 0' TERM\n\
                             sleep 120 &\n\
                             child=$!\n\
                             wait $child";

        fn with_action(f: &Fixture, action: GridAction) -> GridCommand {
            GridCommand {
                action,
                ..f.command.clone()
            }
        }

        #[tokio::test]
        async fn start_waits_for_ready_then_status_and_stop_work() {
            let port = serve_ready().await;
            let mut f = fixture(GridRole::Hub, GridAction::Start, WAITS);
            f.command.port = Some(port);
            f.command.session_timeout = Some(Duration::from_secs(230));

            let started = run(&f.command).await.unwrap();

            assert!(started[0].contains("started (process"), "{started:?}");
            assert!(
                started[0].contains(&format!("ready at http://127.0.0.1:{port}")),
                "{started:?}"
            );
            assert!(
                started.iter().any(|l| l.starts_with("Console: ")),
                "{started:?}"
            );
            let log = std::fs::read_to_string(
                GridProcess::new(&f.command.state_dir, GridRole::Hub).log_path(),
            )
            .unwrap();
            assert!(
                log.contains(&format!("hub --port {port} --session-timeout 230")),
                "{log}"
            );

            let status = run(&with_action(&f, GridAction::Status)).await.unwrap();
            assert!(status[0].contains("is running (process"), "{status:?}");
            assert_eq!(status[1], "Ready: 1 node(s), 2 slot(s), 1 in use.");

            let stopped = run(&with_action(&f, GridAction::Stop)).await.unwrap();
            assert!(
                stopped[0].starts_with("Stopped the Grid hub"),
                "{stopped:?}"
            );
            let after = run(&with_action(&f, GridAction::Status)).await.unwrap();
            assert_eq!(after, ["The Grid hub is not running."]);
        }

        #[tokio::test]
        async fn restart_replaces_the_process() {
            let port = serve_ready().await;
            let mut f = fixture(GridRole::Hub, GridAction::Start, WAITS);
            f.command.port = Some(port);
            let first = run(&f.command).await.unwrap();

            let restarted = run(&with_action(&f, GridAction::Restart)).await.unwrap();

            assert!(
                restarted[0].starts_with("Stopped the Grid hub"),
                "{restarted:?}"
            );
            assert!(restarted[1].contains("started (process"), "{restarted:?}");
            assert_ne!(first[0], restarted[1], "a different process");
            run(&with_action(&f, GridAction::Stop)).await.unwrap();
        }

        #[tokio::test]
        async fn restart_of_a_grid_that_was_not_running_just_starts_it() {
            let port = serve_ready().await;
            let mut f = fixture(GridRole::Hub, GridAction::Restart, WAITS);
            f.command.port = Some(port);

            let lines = run(&f.command).await.unwrap();

            assert_eq!(lines[0], "The Grid hub is not running.");
            assert!(lines[1].contains("started (process"), "{lines:?}");
            run(&with_action(&f, GridAction::Stop)).await.unwrap();
        }

        #[tokio::test]
        async fn a_node_registers_with_the_hub_it_was_told() {
            let port = serve_ready().await;
            let mut f = fixture(GridRole::Node, GridAction::Start, WAITS);
            f.command.port = Some(port);
            f.command.hub = Some("hub.example.com".to_owned());

            let lines = run(&f.command).await.unwrap();

            assert!(
                lines[1].contains("Registered with the hub at hub.example.com"),
                "{lines:?}"
            );
            let log = std::fs::read_to_string(
                GridProcess::new(&f.command.state_dir, GridRole::Node).log_path(),
            )
            .unwrap();
            assert!(log.contains("--hub http://hub.example.com:4444"), "{log}");
            run(&with_action(&f, GridAction::Stop)).await.unwrap();
        }

        #[tokio::test]
        async fn a_grid_that_never_becomes_ready_is_stopped_and_reported() {
            // Nothing answers on this port.
            let port = {
                let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
                listener.local_addr().unwrap().port()
            };
            let mut f = fixture(GridRole::Hub, GridAction::Start, WAITS);
            f.command.port = Some(port);
            f.command.wait = Duration::from_millis(600);

            let error = run(&f.command).await.unwrap_err().to_string();

            assert!(error.contains("did not come up"), "{error}");
            assert!(error.contains("grid-hub.log"), "{error}");
            let after = run(&with_action(&f, GridAction::Status)).await.unwrap();
            assert_eq!(
                after,
                ["The Grid hub is not running."],
                "it must not be left running"
            );
        }

        #[tokio::test]
        async fn a_busy_port_is_refused_before_anything_is_started() {
            // Something else already answers on the port; it would pass the
            // readiness check for a Grid that then fails to start.
            let port = serve_ready().await;
            let mut f = fixture(GridRole::Hub, GridAction::Start, WAITS);
            f.command.port = Some(port);
            f.command.skip_port_check = false;

            let error = run(&f.command).await.unwrap_err().to_string();

            assert!(error.contains(&format!("port {port}")), "{error}");
            assert!(error.contains("already in use"), "{error}");
            let process = GridProcess::new(&f.command.state_dir, GridRole::Hub);
            assert_eq!(process.state().unwrap(), RunState::NotRunning);
            assert!(!process.log_path().exists(), "nothing was started");
        }

        #[tokio::test]
        async fn a_free_port_passes_the_check() {
            let port = free_port().await;
            serve_ready_on_later(port);
            let mut f = fixture(GridRole::Hub, GridAction::Start, WAITS);
            f.command.port = Some(port);
            f.command.skip_port_check = false;

            let lines = run(&f.command).await.unwrap();

            assert!(lines[0].contains("started (process"), "{lines:?}");
            run(&with_action(&f, GridAction::Stop)).await.unwrap();
        }

        #[tokio::test]
        async fn a_java_that_dies_at_once_is_reported_with_its_log() {
            let mut f = fixture(
                GridRole::Hub,
                GridAction::Start,
                "echo 'Error: bad jar' >&2\nexit 2",
            );
            f.command.port = Some(free_port().await);

            let error = run(&f.command).await.unwrap_err().to_string();

            assert!(error.contains("exited at once"), "{error}");
            assert!(error.contains("bad jar"), "{error}");
        }

        #[tokio::test]
        async fn stop_with_nothing_running_says_so() {
            let f = fixture(GridRole::Hub, GridAction::Stop, WAITS);
            let lines = run(&f.command).await.unwrap();
            assert_eq!(lines, ["The Grid hub is not running."]);
        }

        #[tokio::test]
        async fn a_node_with_a_bad_hub_address_is_refused_before_anything_starts() {
            let mut f = fixture(GridRole::Node, GridAction::Start, WAITS);
            f.command.hub = Some("ftp://nope".to_owned());

            let error = run(&f.command).await.unwrap_err().to_string();

            assert!(error.contains("not a Selenium Grid address"), "{error}");
            let process = GridProcess::new(&f.command.state_dir, GridRole::Node);
            assert_eq!(process.state().unwrap(), RunState::NotRunning);
        }
    }
}
