//! The PowerShell script `sbrs completions` prints: usage's script with one fix.
//!
//! usage's PowerShell script cuts the command line at the cursor with `Extent.Text`, which
//! leaves out the spaces after the last word, so for `sbrs plugin ` the engine saw `sbrs plugin`
//! and completed `plugin` again instead of offering `info` and `list`. The patched script pads
//! the line with spaces when the cursor is past the text, instead of cutting it. A test fails
//! when usage's script no longer contains the lines replaced here, so an upgrade cannot ship
//! the script unpatched; drop this module once usage fixes its script. The zsh script needed a
//! fix too until usage 7.4, which gives a complete word zsh's own space and a word that goes on
//! none; a test keeps it so and says when the zsh patch has to come back.

/// The lines of usage's PowerShell script that cut the command line at the cursor. They clamp
/// the cursor to `Extent.Text`, which leaves out the spaces after the last word, so for
/// `sbrs plugin ` the engine saw `sbrs plugin` and completed `plugin` again.
const PS_CUT: &str = "    if ($offset -gt $extent.Text.Length) { $offset = $extent.Text.Length }
    $line = $extent.Text.Substring(0, $offset)
";

/// Their replacement: a cursor past the text means a new word starts, so pad with spaces.
const PS_CUT_PATCHED: &str =
    "    # Patched by sbrs (crates/cli/src/completion.rs): Extent.Text leaves out the spaces
    # after the last word, so a cursor past it starts a new word: pad instead of cutting.
    if ($offset -gt $extent.Text.Length) {
        $line = $extent.Text + (' ' * ($offset - $extent.Text.Length))
    } else {
        $line = $extent.Text.Substring(0, $offset)
    }
";

/// usage's PowerShell `script` with the line cut patched (see [`PS_CUT`]).
pub(crate) fn powershell(script: &str) -> String {
    script.replacen(PS_CUT, PS_CUT_PATCHED, 1)
}

#[cfg(test)]
mod tests {
    use super::{powershell, PS_CUT};
    use crate::Cli;
    use usage::complete::Shell;
    use usage::spec::CommandMeta;

    #[test]
    fn install_offers_only_versions_it_reads() {
        for (word, _) in crate::INSTALL_VERSIONS {
            assert!(
                word.parse::<seleniumbase_rs::browser::options::DriverVersion>()
                    .is_ok(),
                "`sbrs install DRIVER {word}` is offered but not read"
            );
        }
    }

    /// The visible flags and arguments below `command` that take a value and say nothing
    /// about it: no choices, no value hint, no completer. The shell completes file names
    /// there, which is wrong for a selector, a number or a URL, so every declaration must say
    /// what it takes.
    fn unhinted(command: &CommandMeta<'_>, path: &str, found: &mut Vec<String>) {
        for flag in command.flags().iter().filter(|flag| !flag.hide()) {
            if flag.flag().takes_value
                && flag.choices().is_empty()
                && flag.complete_type().is_none()
                && flag.complete().is_none()
            {
                let name = flag.flag().longs.first().copied().unwrap_or("?");
                found.push(format!("{path} --{name}"));
            }
        }
        for arg in command.args().iter().filter(|arg| !arg.hide()) {
            if arg.choices().is_empty() && arg.complete_type().is_none() && arg.complete().is_none()
            {
                found.push(format!("{path} <{}>", arg.arg().name));
            }
        }
        for subcommand in command
            .subcommands()
            .iter()
            .filter(|command| !command.hide())
        {
            let name = format!("{path} {}", subcommand.cmd().name);
            unhinted(subcommand, &name, &mut *found);
        }
    }

    #[test]
    fn every_value_says_what_it_takes() {
        let mut found = Vec::new();
        unhinted(Cli::spec().root, "sbrs", &mut found);
        assert!(
            found.is_empty(),
            "these values fall back to file names in the shell; give each a value_hint, choices or complete:\n{}",
            found.join("\n")
        );
    }

    #[test]
    fn usage_zsh_script_gives_a_complete_word_its_space() {
        let script = Cli::completion_script(Shell::Zsh);
        assert!(
                script.contains("compadd -l -d display_done -U -Q -S ' ' -a inserts_done"),
                "usage's zsh script no longer gives a complete word its space: patch it again (the zsh patch is in the history of crates/cli/src/completion.rs) or update this test"
            );
    }

    #[test]
    fn usage_zsh_script_gives_a_word_that_goes_on_no_space() {
        let script = Cli::completion_script(Shell::Zsh);
        assert!(
            script.contains("compadd -l -d display_open -U -Q -S '' -a inserts_open"),
            "{script}"
        );
        assert!(
            script.starts_with("#compdef sbrs"),
            "usage's installer looks for this header"
        );
    }

    #[test]
    fn usage_still_cuts_the_powershell_line_the_way_the_patch_replaces() {
        let script = Cli::completion_script(Shell::PowerShell);
        assert!(
            script.contains(PS_CUT),
            "usage's PowerShell script changed: update crates/cli/src/completion.rs"
        );
    }

    #[test]
    fn the_powershell_line_is_padded_past_the_last_word() {
        let patched = powershell(&Cli::completion_script(Shell::PowerShell));
        assert!(!patched.contains(PS_CUT), "{patched}");
        assert!(patched.contains("$extent.Text + (' ' * ($offset - $extent.Text.Length))"));
    }
}
