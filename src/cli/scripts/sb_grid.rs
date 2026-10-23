//! `sbrs grid-hub` and `sbrs grid-node`.
//!
//! Python SeleniumBase starts, stops and restarts a Selenium Grid hub or node
//! with `sbrs grid-hub {start|stop|restart}` and `sbrs grid-node ...`. This
//! is the same, built on [`GridProcess`]. The Selenium Server jar is supplied
//! by the user (`--jar`, `SB_SELENIUM_SERVER_JAR`, or the newest jar that
//! `sbrs download server` saved in `downloaded_drivers/`) and is never
//! downloaded by these commands themselves;
//! `status` is an addition that reports whether the Grid is up and ready.
//!
//! [`run`] does the work and returns the lines to show, so the command can be
//! tested without a terminal.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::browser::binaries::DOWNLOADS_DIR;

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
                "no Selenium Server jar: pass --jar PATH, set {JAR_ENV}, or run \
                 `sbrs download server`. sbrs does not \
                 download it by itself; or get selenium-server-<version>.jar from \
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

/// The jar to run: [`resolve_jar`]'s, else the newest `selenium-server-X.Y.Z.jar` in
/// `downloads` (what `sbrs download server` saves).
///
/// # Errors
///
/// Returns the error of [`resolve_jar`] when nothing names a jar and the folder has none.
pub fn resolve_server_jar(
    explicit: Option<&Path>,
    from_env: Option<OsString>,
    downloads: &Path,
) -> Result<PathBuf, SeleniumBaseError> {
    resolve_jar(explicit, from_env).or_else(|error| newest_downloaded_jar(downloads).ok_or(error))
}

/// The Selenium Server release `sbrs download server` fetches when no version is asked for.
pub const SERVER_VERSION: &str = "4.51.0";

/// The SHA-256 of `selenium-server-4.51.0.jar`, as GitHub lists it for the release asset. The
/// download of [`SERVER_VERSION`] is checked against it; another version is not.
pub const SERVER_SHA256: &str = "f4ddbb992fd99c4152934b832c5fe5502a5ca207a1d1cb96af8e5068955c5037";

/// Where Selenium's releases are downloaded from.
const SERVER_RELEASES: &str = "https://github.com/SeleniumHQ/selenium/releases/download";

/// The most bytes a Selenium Server jar may have (the jar of 4.51.0 has 56 MB).
const SERVER_MAX_BYTES: usize = 128 * 1024 * 1024;

/// A Selenium Server jar in the downloads folder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerJar {
    /// Where the jar is.
    pub path: PathBuf,
    /// The SHA-256 of the jar, in lower-case hex.
    pub sha256: String,
    /// Whether the SHA-256 is the one pinned in this build ([`SERVER_SHA256`]).
    pub verified: bool,
    /// Whether the jar was in the folder already, and nothing was downloaded.
    pub reused: bool,
}

/// Downloads Selenium Server 4 into `dir` as `selenium-server-VERSION.jar`.
///
/// `version` is a Selenium 4 release such as `4.51.0`; without one it is [`SERVER_VERSION`].
/// That release is checked against [`SERVER_SHA256`] and a download that differs is not saved;
/// any other release is saved as it arrives and [`ServerJar::verified`] is `false`. A jar of the
/// same name that is already in `dir` is not downloaded again, unless the version is the pinned
/// one and the file does not match its hash. Needs network access otherwise.
///
/// # Errors
///
/// Returns an error for a version that is not a Selenium 4 release (Selenium 3's Grid is gone),
/// a download that fails, is larger than 128 MiB or does not match the pinned hash, or a file
/// that cannot be written.
pub async fn download_server(
    version: Option<&str>,
    dir: &Path,
) -> Result<ServerJar, SeleniumBaseError> {
    let version = server_version(version.unwrap_or(SERVER_VERSION))?;
    let name = format!("selenium-server-{version}.jar");
    let url = format!("{SERVER_RELEASES}/selenium-{version}/{name}");
    let expected = (version == SERVER_VERSION).then_some(SERVER_SHA256);
    fetch_jar(&url, &dir.join(name), expected, SERVER_MAX_BYTES).await
}

/// `text` if it names a Selenium 4 release: three numbers separated by dots, the first 4.
fn server_version(text: &str) -> Result<&str, SeleniumBaseError> {
    let parts: Vec<&str> = text.split('.').collect();
    let numbers = parts.len() == 3
        && parts
            .iter()
            .all(|part| (1..=4).contains(&part.len()) && part.bytes().all(|b| b.is_ascii_digit()));
    if !numbers {
        return Err(SeleniumBaseError::parse(
            text,
            "a Selenium Server version such as 4.51.0",
        ));
    }
    if parts[0] != "4" {
        return Err(SeleniumBaseError::invalid_config(format!(
            "only Selenium Server 4 can be downloaded, not {text}: Selenium Grid 3 is gone"
        )));
    }
    Ok(text)
}

/// Downloads `url` to `dest`, or reuses the file already there (see [`download_server`]).
async fn fetch_jar(
    url: &str,
    dest: &Path,
    expected: Option<&str>,
    max_bytes: usize,
) -> Result<ServerJar, SeleniumBaseError> {
    if let Ok(existing) = tokio::fs::read(dest).await {
        let sha256 = sha256_hex(&existing);
        // A pinned jar that does not match its hash is damaged or replaced: fetch it again.
        if expected.is_none_or(|hash| hash == sha256) {
            return Ok(ServerJar {
                path: dest.to_path_buf(),
                sha256,
                verified: expected.is_some(),
                reused: true,
            });
        }
    }
    crate::install_tls_provider();
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .read_timeout(Duration::from_secs(60))
        .build()
        .map_err(|error| SeleniumBaseError::download(url, error.to_string()))?;
    let network = |error: reqwest::Error| {
        SeleniumBaseError::network(
            url,
            error.status().map_or(0, |status| status.as_u16()),
            error.to_string(),
        )
    };
    let too_large = || {
        SeleniumBaseError::download(
            url,
            format!("the file is larger than {max_bytes} bytes, which no Selenium Server jar is"),
        )
    };
    let mut response = client
        .get(url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(network)?;
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes as u64)
    {
        return Err(too_large());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(network)? {
        if bytes.len() + chunk.len() > max_bytes {
            return Err(too_large());
        }
        bytes.extend_from_slice(&chunk);
    }
    let sha256 = sha256_hex(&bytes);
    if let Some(hash) = expected {
        if sha256 != hash {
            return Err(SeleniumBaseError::invalid_config(format!(
                "the download from {url} has SHA-256 {sha256}, not the pinned {hash}; \
                 it was not saved"
            )));
        }
    }
    let path = dest.to_path_buf();
    let target = path.clone();
    tokio::task::spawn_blocking(move || save(&target, &bytes))
        .await
        .map_err(|error| {
            SeleniumBaseError::download(url, format!("saving the jar stopped: {error}"))
        })??;
    Ok(ServerJar {
        path,
        sha256,
        verified: expected.is_some(),
        reused: false,
    })
}

/// Writes `bytes` to a temporary file in the folder of `dest` and renames it to `dest`, so a
/// cut write never looks like a jar.
fn save(dest: &Path, bytes: &[u8]) -> Result<(), SeleniumBaseError> {
    use std::io::Write as _;
    let dir = match dest.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => Path::new("."),
    };
    std::fs::create_dir_all(dir)?;
    let mut file = tempfile::NamedTempFile::new_in(dir)?;
    file.write_all(bytes)?;
    file.persist(dest)
        .map_err(|error| SeleniumBaseError::Io(error.error))?;
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let digest = ring::digest::digest(&ring::digest::SHA256, bytes);
    digest.as_ref().iter().fold(String::new(), |mut hex, byte| {
        // Writing to a String cannot fail.
        let _ = write!(hex, "{byte:02x}");
        hex
    })
}

/// The newest `selenium-server-X.Y.Z.jar` in `dir`, by version number.
#[must_use]
pub fn newest_downloaded_jar(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let version = jar_version(entry.file_name().to_str()?)?;
            entry.path().is_file().then(|| (version, entry.path()))
        })
        .max_by_key(|(version, _)| *version)
        .map(|(_, path)| path)
}

/// The version in the name of a jar `sbrs download server` saves.
fn jar_version(name: &str) -> Option<(u32, u32, u32)> {
    let version = name
        .strip_prefix("selenium-server-")?
        .strip_suffix(".jar")?;
    let mut parts = version.split('.').map(|part| part.parse::<u32>().ok());
    let found = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(found)
}

fn launch_for(command: &GridCommand) -> Result<GridLaunch, SeleniumBaseError> {
    let jar = resolve_server_jar(
        command.jar.as_deref(),
        std::env::var_os(JAR_ENV),
        Path::new(DOWNLOADS_DIR),
    )?;
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

    // Wait for the Grid to answer, but stop at once if its process ends: a Java
    // that dies after the first moments would otherwise be waited on for the
    // whole timeout and then reported as slow, without its log.
    let ready = wait_until_ready(&address, command.wait, Duration::from_millis(500));
    let failure = tokio::select! {
        ready = ready => ready.err().map(|error| {
            format!(
                "the Grid {role} did not come up: {error}. Its log is {}",
                process.log_path().display()
            )
        }),
        note = ended(process) => Some(format!("the Grid {role} exited while starting. {note}")),
    };
    if let Some(reason) = failure {
        // Do not leave a Grid that never came up running in the background.
        let _ = process.stop(Duration::from_secs(5));
        return Err(SeleniumBaseError::browser_launch(
            launch.java_path().display().to_string(),
            reason,
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
                "Run tests against it with: sbrs --webdriver {address} test"
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

/// Resolves, with the end of the log, once the Grid process `start` began has
/// ended. Pending while it runs.
async fn ended(process: &GridProcess) -> String {
    loop {
        // A record that cannot be read is left to the readiness wait's own
        // timeout; it is not a reason to claim the process ended.
        if let Ok(Some(note)) = process.ended() {
            return note;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
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
            "The Grid {role} is not running (process {} is gone). Start it with `sbrs grid-{role} start`.",
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

    fn jar_server(body: &[u8]) -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let listener = tokio::net::TcpListener::from_std(listener).unwrap();
        let hits = Arc::new(AtomicUsize::new(0));
        let (counter, body) = (Arc::clone(&hits), body.to_vec());
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                counter.fetch_add(1, Ordering::SeqCst);
                let body = body.clone();
                tokio::spawn(async move {
                    let mut seen = Vec::new();
                    let mut chunk = [0_u8; 1024];
                    while !seen.windows(4).any(|window| window == b"\r\n\r\n") {
                        match stream.read(&mut chunk).await {
                            Ok(count) if count > 0 => seen.extend_from_slice(&chunk[..count]),
                            _ => return,
                        }
                    }
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(head.as_bytes()).await;
                    let _ = stream.write_all(&body).await;
                });
            }
        });
        (format!("http://{address}/selenium-server.jar"), hits)
    }

    #[tokio::test]
    async fn a_download_that_matches_its_hash_is_saved_and_verified() {
        let body = b"pretend this is a jar".to_vec();
        let (url, _) = jar_server(&body);
        let dir = tempfile::tempdir().unwrap();
        let dest = dir
            .path()
            .join("downloaded_drivers/selenium-server-4.99.0.jar");
        let hash = sha256_hex(&body);

        let jar = fetch_jar(&url, &dest, Some(&hash), 1024).await.unwrap();

        assert_eq!(jar.path, dest);
        assert_eq!(jar.sha256, hash);
        assert!(jar.verified && !jar.reused);
        assert_eq!(std::fs::read(&dest).unwrap(), body);
    }

    #[tokio::test]
    async fn a_download_that_does_not_match_its_hash_is_not_saved() {
        let (url, _) = jar_server(b"something else");
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("selenium-server-4.99.0.jar");

        let error = fetch_jar(&url, &dest, Some(&sha256_hex(b"the real jar")), 1024)
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains("not the pinned"), "{error}");
        assert!(
            !dest.exists(),
            "a jar that fails its check must not be left behind"
        );
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn a_version_without_a_pinned_hash_reports_its_hash_as_not_verified() {
        let body = b"another release".to_vec();
        let (url, _) = jar_server(&body);
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("selenium-server-4.1.0.jar");

        let jar = fetch_jar(&url, &dest, None, 1024).await.unwrap();

        assert!(!jar.verified && !jar.reused);
        assert_eq!(jar.sha256, sha256_hex(&body));
        assert_eq!(std::fs::read(&dest).unwrap(), body);
    }

    #[tokio::test]
    async fn a_jar_that_is_there_already_is_not_downloaded_again() {
        let body = b"the jar".to_vec();
        let (url, hits) = jar_server(&body);
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("selenium-server-4.99.0.jar");
        std::fs::write(&dest, &body).unwrap();

        let pinned = fetch_jar(&url, &dest, Some(&sha256_hex(&body)), 1024)
            .await
            .unwrap();
        let unpinned = fetch_jar(&url, &dest, None, 1024).await.unwrap();

        assert!(pinned.reused && pinned.verified);
        assert!(unpinned.reused && !unpinned.verified);
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn a_pinned_jar_that_is_damaged_is_downloaded_again() {
        let body = b"the real jar".to_vec();
        let (url, hits) = jar_server(&body);
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("selenium-server-4.99.0.jar");
        std::fs::write(&dest, b"half a jar").unwrap();

        let jar = fetch_jar(&url, &dest, Some(&sha256_hex(&body)), 1024)
            .await
            .unwrap();

        assert!(!jar.reused && jar.verified);
        assert_eq!(std::fs::read(&dest).unwrap(), body);
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_download_over_the_size_limit_is_refused() {
        let (url, _) = jar_server(&[7_u8; 2048]);
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("selenium-server-4.99.0.jar");

        let error = fetch_jar(&url, &dest, None, 1024)
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains("larger than 1024 bytes"), "{error}");
        assert!(!dest.exists());
    }

    #[test]
    fn only_selenium_four_releases_can_be_asked_for() {
        assert_eq!(server_version("4.51.0").unwrap(), "4.51.0");
        assert_eq!(server_version("4.9.12").unwrap(), "4.9.12");
        let old = server_version("3.141.59").unwrap_err().to_string();
        assert!(old.contains("Grid 3 is gone"), "{old}");
        for bad in [
            "4.51",
            "4.51.0.1",
            "latest",
            "4.x.0",
            "../4.1.0",
            "4.1.0/../../x",
            "",
            "4..1",
            "4.1.0-beta",
        ] {
            assert!(server_version(bad).is_err(), "{bad:?} was accepted");
        }
    }

    #[test]
    fn the_pinned_release_is_a_selenium_four_version_with_a_sha256() {
        assert!(server_version(SERVER_VERSION).is_ok());
        assert_eq!(SERVER_SHA256.len(), 64);
        assert!(SERVER_SHA256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
    }

    #[test]
    fn hashes_are_lower_case_hex_sha256() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn the_newest_jar_is_the_highest_version_not_the_last_name() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "selenium-server-4.9.0.jar",
            "selenium-server-4.51.0.jar",
            "selenium-server-4.10.1.jar",
            "selenium-server-4.60.0.jar.part",
            "selenium-server-4.70.0.jar.sha256",
            "selenium-server-latest.jar",
            "chromedriver",
        ] {
            std::fs::write(dir.path().join(name), b"x").unwrap();
        }
        std::fs::create_dir(dir.path().join("selenium-server-4.99.0.jar")).unwrap();

        assert_eq!(
            newest_downloaded_jar(dir.path()),
            Some(dir.path().join("selenium-server-4.51.0.jar"))
        );
        assert_eq!(newest_downloaded_jar(&dir.path().join("missing")), None);
    }

    #[test]
    fn a_downloaded_jar_is_used_only_when_nothing_else_names_one() {
        let dir = tempfile::tempdir().unwrap();
        let downloaded = dir.path().join("selenium-server-4.2.0.jar");
        std::fs::write(&downloaded, b"x").unwrap();

        assert_eq!(
            resolve_server_jar(None, None, dir.path()).unwrap(),
            downloaded
        );
        assert_eq!(
            resolve_server_jar(None, Some("env.jar".into()), dir.path()).unwrap(),
            Path::new("env.jar")
        );
        assert_eq!(
            resolve_server_jar(
                Some(Path::new("flag.jar")),
                Some("env.jar".into()),
                dir.path()
            )
            .unwrap(),
            Path::new("flag.jar")
        );
        let error = resolve_server_jar(None, None, &dir.path().join("empty"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("sbrs download server"), "{error}");
    }

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
            std::fs::write(
                &java,
                format!("#!/bin/sh\nif [ \"$1\" = \"--warm\" ]; then exit 0; fi\n{java_body}\n"),
            )
            .unwrap();
            std::fs::set_permissions(&java, std::fs::Permissions::from_mode(0o755)).unwrap();
            // macOS scans a freshly written script on its first run, which can take seconds
            // under load: run it once now so the Grid under test starts it promptly.
            std::process::Command::new(&java)
                .arg("--warm")
                .status()
                .unwrap();
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
            // The stand-in `java` writes its arguments to the log after the
            // ready stub has already answered, so under load (a freshly written
            // script is slow to start on macOS) the log can still be empty here.
            let log_path = GridProcess::new(&f.command.state_dir, GridRole::Hub).log_path();
            let wanted = format!("hub --port {port} --session-timeout 230");
            let mut log = String::new();
            for _ in 0..100 {
                log = std::fs::read_to_string(&log_path).unwrap_or_default();
                if log.contains(&wanted) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            assert!(log.contains(&wanted), "{log}");

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

            // Caught by the first watch ("exited at once") or, on a busy machine
            // where the script has not run yet, by the readiness wait.
            assert!(error.contains("exited"), "{error}");
            assert!(error.contains("bad jar"), "{error}");
        }

        #[tokio::test]
        async fn a_java_that_dies_later_is_reported_without_waiting_out_the_timeout() {
            // Alive for longer than the first watch (500 ms), so only the
            // readiness wait can notice that it ended.
            let mut f = fixture(
                GridRole::Hub,
                GridAction::Start,
                "sleep 2\necho 'Error: late failure' >&2\nexit 2",
            );
            f.command.port = Some(free_port().await);
            f.command.wait = Duration::from_secs(60);
            let began = std::time::Instant::now();

            let error = run(&f.command).await.unwrap_err().to_string();

            assert!(
                began.elapsed() < Duration::from_secs(30),
                "waited out the timeout: {:?}",
                began.elapsed()
            );
            assert!(error.contains("exited while starting"), "{error}");
            assert!(error.contains("late failure"), "{error}");
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
