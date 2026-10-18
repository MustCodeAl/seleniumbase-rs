//! The machine a registration is made on: the home and config folders, the working folder and
//! `PATH`. Every path the installer touches derives from an [`Env`], so a test points it at a
//! temporary folder without changing the process environment.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

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
    /// The folder `--scope project` writes into.
    pub cwd: PathBuf,
    /// `PATH`, where `claude`, `codex` and `code` are looked for. `None` finds nothing.
    pub path: Option<OsString>,
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
        let cwd =
            std::env::current_dir().map_err(|e| format!("cannot read the current folder: {e}"))?;
        Ok(Self {
            os: Os::current(),
            home,
            config_dir,
            data_dir,
            xdg_config_home,
            codex_home,
            cwd,
            path: std::env::var_os("PATH"),
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
            home,
            config_dir,
            data_dir,
            xdg_config_home: None,
            cwd: root.join("project"),
            path: None,
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

    #[cfg(unix)]
    #[test]
    fn absolute_joins_a_relative_path_to_the_base() {
        let base = Path::new("/work");
        assert_eq!(absolute(Path::new("out"), base), Path::new("/work/out"));
        assert_eq!(absolute(Path::new("/abs/out"), base), Path::new("/abs/out"));
    }
}
