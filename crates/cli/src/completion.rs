//! The zsh and PowerShell scripts `sbrs completions` prints: usage's scripts with one fix each.
//!
//! usage 6.12's zsh script adds every candidate with `compadd -S ''`. That stops zsh from
//! putting its space after a complete word, so `sbrs pl<Tab>` gives `sbrs plugin` and the
//! next Tab completes `plugin` again instead of offering `info` and `list`; the same
//! happens after every flag. The patched script adds complete words without `-S ''`, so
//! zsh appends its own auto-removable space, and keeps `-S ''` for words that go on: a
//! `--flag=`, a folder ending in `/`, a `key:`. A test fails when usage's script no longer
//! contains the line replaced here, so an upgrade cannot ship the script unpatched; drop
//! this module once usage fixes its script.

/// The line of usage's zsh script that adds the candidates.
const ZSH_COMPADD: &str = "        compadd -l -d display -U -Q -S '' -a inserts && __usage_ret=0\n";

/// Its replacement: complete words get zsh's space, words that go on get none.
const ZSH_COMPADD_PATCHED: &str = r#"        # Patched by sbrs (crates/cli/src/completion.rs): a complete word gets zsh's own
        # auto-removable space, so the next Tab reaches the next level; a word that goes on
        # (`--flag=`, a folder `/`, `key:`) gets no suffix.
        local -a __sbrs_done=() __sbrs_done_display=() __sbrs_open=() __sbrs_open_display=()
        for (( i = 1; i <= ${#inserts[@]}; i++ )); do
            if [[ "${inserts[i]}" == *[=/:] ]]; then
                __sbrs_open+=("${inserts[i]}")
                __sbrs_open_display+=("${display[i]}")
            else
                __sbrs_done+=("${inserts[i]}")
                __sbrs_done_display+=("${display[i]}")
            fi
        done
        (( ${#__sbrs_done[@]} )) && compadd -l -d __sbrs_done_display -U -Q -a __sbrs_done && __usage_ret=0
        (( ${#__sbrs_open[@]} )) && compadd -l -d __sbrs_open_display -U -Q -S '' -a __sbrs_open && __usage_ret=0
"#;

/// usage's zsh `script` with the candidate line patched (see the module docs).
pub(crate) fn zsh(script: &str) -> String {
    script.replacen(ZSH_COMPADD, ZSH_COMPADD_PATCHED, 1)
}

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
    use super::{powershell, zsh, PS_CUT, ZSH_COMPADD};
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
        for flag in command.flags.iter().filter(|flag| !flag.hide) {
            if flag.flag.takes_value
                && flag.choices.is_empty()
                && flag.complete_type().is_none()
                && flag.complete.is_none()
            {
                let name = flag.flag.longs.first().copied().unwrap_or("?");
                found.push(format!("{path} --{name}"));
            }
        }
        for arg in command.args.iter().filter(|arg| !arg.hide) {
            if arg.choices.is_empty() && arg.complete_type.is_none() && arg.complete.is_none() {
                found.push(format!("{path} <{}>", arg.arg.name));
            }
        }
        for subcommand in command.subcommands.iter().filter(|command| !command.hide) {
            let name = format!("{path} {}", subcommand.cmd.name);
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
    fn usage_still_writes_the_line_the_patch_replaces() {
        let script = Cli::completion_script(Shell::Zsh);
        assert!(
            script.contains(ZSH_COMPADD),
            "usage's zsh script changed: update crates/cli/src/completion.rs or drop it if usage adds the space itself"
        );
    }

    #[test]
    fn complete_words_get_zsh_s_space_and_open_words_none() {
        let patched = zsh(&Cli::completion_script(Shell::Zsh));
        assert!(!patched.contains("-S '' -a inserts"), "{patched}");
        assert!(patched.contains("compadd -l -d __sbrs_done_display -U -Q -a __sbrs_done"));
        assert!(patched.contains("compadd -l -d __sbrs_open_display -U -Q -S '' -a __sbrs_open"));
        assert!(
            patched.starts_with("#compdef sbrs"),
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
