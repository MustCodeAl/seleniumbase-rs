//! The arguments of the tools: `proxy` and `download`.
//!
//! (Plain comments on the structs, not docs: a doc comment on a struct that derives `Args`
//! would be help text.)

// The options of `sbrs proxy`.
#[derive(Debug, usage::Args)]
#[usage(args_override_self = false)]
pub(crate) struct ProxyArgs {
    /// The address to listen on. Anything but a loopback address makes this an open proxy
    /// for other machines, and the command says so.
    #[usage(long, value_name = "HOST", default = "127.0.0.1", value_hint = usage::ValueHint::Hostname)]
    pub(crate) hostname: String,
    /// The port to listen on; 0 picks a free one.
    #[usage(long, value_name = "PORT", default = "8899", value_hint = usage::ValueHint::Other)]
    pub(crate) port: u16,
}

// The `download` command: it holds only its own subcommands.
#[derive(Debug, usage::Args)]
pub(crate) struct DownloadArgs {
    #[usage(subcommand)]
    pub(crate) command: DownloadCommand,
}

#[derive(Debug, usage::Subcommands)]
pub(crate) enum DownloadCommand {
    /// Download the Selenium Server 4 jar that `grid-hub` and `grid-node` run.
    ///
    /// Saves selenium-server-VERSION.jar in ./downloaded_drivers/ and prints its path. The
    /// default version is pinned in this build and the download is checked against its
    /// SHA-256; another --version is saved as it arrives, with its SHA-256 printed and
    /// `not verified`. When neither --jar nor SB_SELENIUM_SERVER_JAR is given, `grid-hub` and
    /// `grid-node` use the newest jar in that folder. Python's Selenium 3 server is not
    /// offered, because Grid 3 is gone.
    #[usage(example = "sbrs download server")]
    Server(ServerArgs),
}

// The option of `download server`.
#[derive(Debug, usage::Args)]
pub(crate) struct ServerArgs {
    /// The release to download, such as 4.51.0 (default: the version this build pins).
    #[usage(long, value_name = "VERSION", value_hint = usage::ValueHint::Other)]
    pub(crate) version: Option<String>,
}
