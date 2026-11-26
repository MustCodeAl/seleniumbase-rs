//! Regression tests for the shell completion scripts `sbrs completions` prints.
//!
//! Both the zsh script (fixed in usage 7.4) and the PowerShell script (fixed in
//! usage-argv 7.4.1 via `$extent.Text.PadRight($offset).Substring(0, $offset)`)
//! are now emitted directly by `usage` without local string patching; these
//! tests keep both fixes from regressing on future upgrades.

#[cfg(test)]
mod tests {
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
    fn usage_powershell_script_pads_line_past_last_word() {
        let script = Cli::completion_script(Shell::PowerShell);
        assert!(
            script.contains("$extent.Text.PadRight($offset).Substring(0, $offset)"),
            "usage's PowerShell script no longer pads past the last word: patch it again or update this test\n{script}"
        );
    }
}
