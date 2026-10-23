//! The zsh completion script `sbrs completions zsh` prints: usage's script with one fix.
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

#[cfg(test)]
mod tests {
    use super::{zsh, ZSH_COMPADD};
    use crate::Cli;
    use usage::complete::Shell;

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
}
