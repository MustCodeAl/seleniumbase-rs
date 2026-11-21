//! The machine a registration is made on: the home and config folders, the working folder and
//! `PATH`. Every path the installer touches derives from an [`Env`], so a test points it at a
//! temporary folder without changing the process environment.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[cfg(test)]
use super::runner::NoRunner;
use super::runner::{Runner, SystemRunner};

/// The operating systems whose config locations differ.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Os {
    /// macOS.
    Mac,
    /// Windows.
    Windows,
    /// Linux and the other Unix systems.
    Linux,
}

impl Os {
    /// The system this program was built for.
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Mac
        } else if cfg!(windows) {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}

/// Where things are on this machine.
#[derive(Clone, Debug)]
pub struct Env {
    /// Decides which config locations apply.
    pub os: Os,
    /// The user's home folder.
    pub home: PathBuf,
    /// Where desktop apps keep their config: `~/Library/Application Support` (macOS),
    /// `%APPDATA%` (Windows), `~/.config` (Linux).
    pub config_dir: PathBuf,
    /// Where per-user program data goes (`dirs::data_dir`).
    pub data_dir: PathBuf,
    /// `$XDG_CONFIG_HOME` when it is set to an absolute path (Zed on Linux reads it).
    pub xdg_config_home: Option<PathBuf>,
    /// The folder of Codex's `config.toml`: `$CODEX_HOME`, else `~/.codex`.
    pub codex_home: PathBuf,
    /// The folder of Grok Build's `config.toml`: `$GROK_HOME`, else `~/.grok`.
    pub grok_home: PathBuf,
    /// The folder `--scope project` writes into.
    pub cwd: PathBuf,
    /// `PATH`, where `claude`, `codex` and `code` are looked for. `None` finds nothing.
    pub path: Option<OsString>,
    /// Cargo's home folder: `$CARGO_HOME`, else `~/.cargo`. `cargo install` puts its programs in
    /// the `bin` folder of it.
    pub cargo_home: PathBuf,
    /// Runs the programs `upgrade` looks at and starts; a test swaps it for one that records.
    pub runner: Arc<dyn Runner>,
}

impl Env {
    /// Reads the environment of this process.
    ///
    /// # Errors
    ///
    /// A message when the home folder or the working folder cannot be found.
    pub fn detect() -> Result<Self, String> {
        let home = dirs::home_dir().ok_or("cannot find the home folder")?;
        let config_dir = dirs::config_dir().unwrap_or_else(|| home.join(".config"));
        let data_dir = dirs::data_dir().unwrap_or_else(|| home.join(".local").join("share"));
        let xdg_config_home = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute());
        let codex_home = std::env::var_os("CODEX_HOME")
            .filter(|value| !value.is_empty())
            .map_or_else(|| home.join(".codex"), PathBuf::from);
        let grok_home = std::env::var_os("GROK_HOME")
            .filter(|value| !value.is_empty())
            .map_or_else(|| home.join(".grok"), PathBuf::from);
        let cwd =
            std::env::current_dir().map_err(|e| format!("cannot read the current folder: {e}"))?;
        let cargo_home = std::env::var_os("CARGO_HOME")
            .filter(|value| !value.is_empty())
            .map_or_else(
                || home.join(".cargo"),
                |value| absolute(Path::new(&value), &cwd),
            );
        Ok(Self {
            os: Os::current(),
            home,
            config_dir,
            data_dir,
            xdg_config_home,
            codex_home,
            grok_home,
            cwd,
            path: std::env::var_os("PATH"),
            cargo_home,
            runner: Arc::new(SystemRunner),
        })
    }

    /// The path of the program `name` on `PATH`, if there is one.
    pub fn find_program(&self, name: &str) -> Option<PathBuf> {
        let path = self.path.as_deref()?;
        which::which_in(name, Some(path), &self.cwd).ok()
    }

    /// An environment laid out under `root` as `os` would lay it out, with nothing on `PATH`.
    #[cfg(test)]
    pub fn rooted(root: &Path, os: Os) -> Self {
        let home = root.join("home");
        let (config_dir, data_dir) = match os {
            Os::Mac => {
                let support = home.join("Library").join("Application Support");
                (support.clone(), support)
            }
            Os::Windows => {
                let roaming = root.join("AppData").join("Roaming");
                (roaming.clone(), roaming)
            }
            Os::Linux => (home.join(".config"), home.join(".local").join("share")),
        };
        Self {
            os,
            codex_home: home.join(".codex"),
            grok_home: home.join(".grok"),
            cargo_home: home.join(".cargo"),
            home,
            config_dir,
            data_dir,
            xdg_config_home: None,
            cwd: root.join("project"),
            path: None,
            runner: Arc::new(NoRunner),
        }
    }
}

/// `path` as an absolute path, joined to `base` when it is relative. Nothing is read from the
/// disk, so the folder need not exist yet.
pub fn absolute(path: &Path, base: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

/// The path of the running binary, resolved through links, without the `\\?\` prefix that
/// `canonicalize` puts on a Windows path (programs that read a config file do not all accept it).
///
/// # Errors
///
/// A message when the path cannot be found or resolved.
pub fn running_binary() -> Result<PathBuf, String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("cannot find the path of this program: {e}"))?;
    let resolved = std::fs::canonicalize(&exe)
        .map_err(|e| format!("cannot resolve {}: {e}", exe.display()))?;
    Ok(strip_verbatim(resolved))
}

/// The prefix of the Homebrew install that holds `path`: the folders before `Cellar`, as in
/// `/opt/homebrew/Cellar/sbrs/0.8.5/bin/sbrs-mcp`. `None` for a path outside a Cellar.
pub fn homebrew_prefix(path: &Path) -> Option<PathBuf> {
    let mut prefix = PathBuf::new();
    let mut parts = path.components().peekable();
    while let Some(part) = parts.next() {
        if part.as_os_str() == "Cellar" && parts.peek().is_some() {
            return Some(prefix);
        }
        prefix.push(part);
    }
    None
}

/// The path to register for the program at `exe`.
///
/// Homebrew keeps each version in `PREFIX/Cellar/NAME/VERSION/` and `brew upgrade` removes the
/// old one, so the resolved path of a Homebrew program stops working at the next upgrade. The
/// link `PREFIX/bin/NAME` follows the upgrade; it is returned when it is there and leads to
/// `exe`. Any other path is returned as it was.
pub fn stable_path(exe: PathBuf) -> PathBuf {
    let Some(prefix) = homebrew_prefix(&exe) else {
        return exe;
    };
    let Some(name) = exe.file_name() else {
        return exe;
    };
    let link = prefix.join("bin").join(name);
    match (std::fs::canonicalize(&link), std::fs::canonicalize(&exe)) {
        (Ok(through), Ok(own)) if through == own => link,
        _ => exe,
    }
}

/// The path of the running binary as a client should start it: [`running_binary`], through
/// [`stable_path`].
///
/// # Errors
///
/// A message when the path cannot be found or resolved.
pub fn registration_binary() -> Result<PathBuf, String> {
    running_binary().map(stable_path)
}

/// Removes the `\\?\` prefix of a Windows drive path (`\\?\C:\x` becomes `C:\x`); every other
/// path is returned as it was.
fn strip_verbatim(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_verbatim_keeps_drive_paths_and_leaves_others() {
        assert_eq!(
            strip_verbatim(PathBuf::from(r"\\?\C:\Users\me\sbrs-mcp.exe")),
            PathBuf::from(r"C:\Users\me\sbrs-mcp.exe")
        );
        assert_eq!(
            strip_verbatim(PathBuf::from(r"\\?\UNC\server\share\x")),
            PathBuf::from(r"\\?\UNC\server\share\x")
        );
        assert_eq!(
            strip_verbatim(PathBuf::from("/usr/bin/sbrs-mcp")),
            PathBuf::from("/usr/bin/sbrs-mcp")
        );
    }

    #[test]
    fn the_homebrew_prefix_is_what_comes_before_the_cellar() {
        assert_eq!(
            homebrew_prefix(Path::new("/opt/homebrew/Cellar/sbrs/0.8.5/bin/sbrs-mcp")),
            Some(PathBuf::from("/opt/homebrew"))
        );
        assert_eq!(
            homebrew_prefix(Path::new("/home/linuxbrew/.linuxbrew/Cellar/sbrs/1/bin/x")),
            Some(PathBuf::from("/home/linuxbrew/.linuxbrew"))
        );
        assert_eq!(homebrew_prefix(Path::new("/usr/local/bin/sbrs-mcp")), None);
        assert_eq!(homebrew_prefix(Path::new("/opt/homebrew/Cellar")), None);
    }

    #[cfg(unix)]
    #[test]
    fn absolute_joins_a_relative_path_to_the_base() {
        let base = Path::new("/work");
        assert_eq!(absolute(Path::new("out"), base), Path::new("/work/out"));
        assert_eq!(absolute(Path::new("/abs/out"), base), Path::new("/abs/out"));
    }
}
