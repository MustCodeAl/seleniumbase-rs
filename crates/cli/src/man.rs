//! The man pages `man/man1/sbrs.1` and `man/man1/sbrs-mcp.1`, rendered by usage from the same
//! spec as the completions, the CLI reference page and the Fig spec.
//!
//! usage writes one page for a program: NAME, SYNOPSIS, DESCRIPTION, the options that apply to
//! every command, a COMMANDS list, and then one section for each command (`SBRS OPEN`,
//! `SBRS PLUGIN LIST`) with its usage, arguments, flags and examples. It writes no page of its own
//! for a subcommand, so there is no `sbrs-open.1`. Built only for the tests that keep the
//! committed pages current; the binaries carry the committed text (`sbrs completions man`).

use usage_lib::docs::manpage::ManpageRenderer;
use usage_lib::Spec;

/// The manual section of both pages: user commands.
const SECTION: u8 = 1;

/// The man page of the program whose usage spec is `spec`, as roff.
pub(crate) fn render(spec: &Spec) -> String {
    ManpageRenderer::new(spec.clone())
        .with_section(SECTION)
        .render()
        .expect("usage renders the man page of its own spec")
}

/// Compares the file `relative` (a path from the repository root) with `generated`, or, when
/// `SB_UPDATE_COMPLETIONS` is set, writes the file, creating its folder.
///
/// The same switch as for the usage spec, the CLI reference and the Fig spec: the pages are
/// generated from the same declarations, so one command writes them all again.
pub(crate) fn assert_current(relative: &str, generated: &str) {
    // The files live at the repository root, two folders above this crate.
    let path = std::path::PathBuf::from(format!("{}/../../{relative}", env!("CARGO_MANIFEST_DIR")));
    if std::env::var_os("SB_UPDATE_COMPLETIONS").is_some() {
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder).expect("create the folder of the man page");
        }
        std::fs::write(&path, generated).expect("write the man page");
    } else {
        let committed = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            committed == generated,
            "{relative} is stale: run SB_UPDATE_COMPLETIONS=1 cargo test -p seleniumbase-rs-cli --bin sbrs --bin sbrs-mcp man_page"
        );
    }
}
