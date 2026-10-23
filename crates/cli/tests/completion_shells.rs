//! The completion scripts `sbrs completions SHELL` prints, run in the real shells.
//!
//! `completion_values.rs` reads what the hidden `__complete_word__` command answers; these
//! tests check that each shell acts on that answer: a complete word gets its space (zsh), a
//! second level is reached after a space (zsh, PowerShell), a free-text value offers no file
//! names (zsh, fish, bash), and folders and extensions are honoured. Each test is `#[ignore]`d
//! and skips itself with a message when its shell is not installed. Run them with
//! `cargo test -p seleniumbase-rs-cli --test completion_shells -- --ignored`.
//!
//! The scripts call `sbrs` by name, so the folder of the `sbrs` under test goes first on
//! `PATH`. Every shell runs in a folder that holds one file, `only.txt`, and one folder,
//! `sub`, so a fallback to file names shows up as one of them.

use std::path::Path;
use std::process::{Command, Output};

/// `PATH` with the folder of the `sbrs` under test first.
fn path_with_sbrs() -> String {
    let sbrs = Path::new(env!("CARGO_BIN_EXE_sbrs"));
    format!(
        "{}:{}",
        sbrs.parent().expect("sbrs has a folder").display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

/// Whether `shell` starts and runs `probe`.
fn installed(shell: &str, probe: &[&str]) -> bool {
    Command::new(shell)
        .args(probe)
        .output()
        .is_ok_and(|output| output.status.success())
}

/// A folder with one file and one folder in it, to run a shell in.
fn workdir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temporary folder");
    std::fs::write(dir.path().join("only.txt"), "x").expect("write only.txt");
    std::fs::create_dir(dir.path().join("sub")).expect("create sub");
    dir
}

fn text(output: &Output) -> String {
    format!(
        "status {:?}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// The lines of the standard output.
fn lines(output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| line.trim_end().to_owned())
        .collect()
}

// ---------------------------------------------------------------------------
// zsh: a real interactive shell on a pseudo-terminal
// ---------------------------------------------------------------------------

/// Usage's zsh script run for the line in `$SBRS_LINE`, the cursor at its end, with stand-ins
/// for the completion builtins: `compadd` prints `space WORDS` when zsh would add its space
/// after the words (no `-S ''`) and `none WORDS` when it would not, and `_files` and
/// `_path_files` print `files ARGUMENTS` where the script falls back to file names.
const ZSH_DRIVER: &str = r#"
compadd() {
    local suffix=space array=
    while (( $# )); do
        case $1 in
            -S) suffix=none; shift 2 ;;
            -a) array=$2; shift 2 ;;
            -d) shift 2 ;;
            *) shift ;;
        esac
    done
    print -r -- "$suffix ${(P)array}"
}
compdef() { :; }
_files() { print -r -- "files $*"; }
_path_files() { print -r -- "files $*"; }
_command_names() { :; }
typeset -A compstate
source <(sbrs completions zsh)
BUFFER=$SBRS_LINE
CURSOR=${#BUFFER}
_sbrs
true
"#;

/// What the zsh script does for `line`: one line per `compadd` or file completion call.
fn zsh_answers(line: &str) -> Vec<String> {
    let dir = workdir();
    let output = Command::new("zsh")
        .args(["-fc", ZSH_DRIVER])
        .current_dir(dir.path())
        .env("PATH", path_with_sbrs())
        .env("SBRS_LINE", line)
        .output()
        .expect("run zsh");
    assert!(output.status.success(), "{line:?}: {}", text(&output));
    lines(&output)
}

#[test]
#[ignore = "runs a real zsh"]
fn zsh_reaches_the_second_level_and_offers_no_file_for_a_free_text_value() {
    if !installed("zsh", &["-fc", "true"]) {
        eprintln!("skipped: zsh is not installed");
        return;
    }
    // A complete word gets zsh's own space, so the next Tab reaches the next level.
    assert_eq!(zsh_answers("sbrs plu"), ["space plugin"]);
    assert_eq!(zsh_answers("sbrs plugin "), ["space info list"]);
    assert_eq!(zsh_answers("sbrs plugin li"), ["space list"]);
    // A flag gets its space too, and a global flag completes after the command.
    assert_eq!(zsh_answers("sbrs get-title --headl"), ["space --headless"]);
    assert_eq!(
        zsh_answers("sbrs get-title --hea"),
        ["space --headed --headless"]
    );
    // The driver of `install`, then its version.
    assert_eq!(zsh_answers("sbrs install ge"), ["space geckodriver"]);
    assert_eq!(
        zsh_answers("sbrs install chromedriver sta"),
        ["space stable"]
    );
    // A selector or a URL is not a file name: no `compadd` call and no file completion.
    assert_eq!(zsh_answers("sbrs get-text --css "), Vec::<String>::new());
    assert_eq!(zsh_answers("sbrs open "), Vec::<String>::new());
    assert_eq!(zsh_answers("sbrs mkfile x.rs --url "), Vec::<String>::new());
    // A folder or a file is, for a command that takes one.
    for line in ["sbrs mkdir ", "sbrs --config ", "sbrs export-recording "] {
        let answers = zsh_answers(line);
        assert!(
            answers.iter().any(|answer| answer.starts_with("files")),
            "{line:?}: {answers:?}"
        );
    }
}
// ---------------------------------------------------------------------------
// fish
// ---------------------------------------------------------------------------

/// The candidates fish offers for `line`: the words before the tab of each `complete -C` line.
fn fish_candidates(line: &str) -> Vec<String> {
    let dir = workdir();
    let output = Command::new("fish")
        .args([
            "--no-config",
            "-c",
            "sbrs completions fish | source; complete -C $SBRS_LINE",
        ])
        .current_dir(dir.path())
        .env("PATH", path_with_sbrs())
        .env("SBRS_LINE", line)
        .output()
        .expect("run fish");
    assert!(output.status.success(), "{line:?}: {}", text(&output));
    lines(&output)
        .into_iter()
        .map(|candidate| candidate.split('\t').next().unwrap_or_default().to_owned())
        .collect()
}

#[test]
#[ignore = "runs a real fish"]
fn fish_completes_every_level_and_offers_no_file_for_a_free_text_value() {
    if !installed("fish", &["--no-config", "-c", "true"]) {
        eprintln!("skipped: fish is not installed");
        return;
    }
    let mut plugin = fish_candidates("sbrs plugin ");
    plugin.sort();
    assert_eq!(plugin, ["info", "list"]);
    let mut grid = fish_candidates("sbrs grid-hub ");
    grid.sort();
    assert_eq!(grid, ["restart", "start", "status", "stop"]);
    // A global flag completes after the command.
    let mut flags = fish_candidates("sbrs get-title --hea");
    flags.sort();
    assert_eq!(flags, ["--headed", "--headless"]);
    let mut drivers = fish_candidates("sbrs install ");
    drivers.sort();
    assert_eq!(
        drivers,
        [
            "cft",
            "chromedriver",
            "chromium",
            "chs",
            "edgedriver",
            "geckodriver",
            "uc_driver"
        ]
    );
    assert_eq!(fish_candidates("sbrs install chromedriver sta"), ["stable"]);
    // Nothing for a selector or a URL, where the shell would offer `only.txt` and `sub/`.
    assert_eq!(
        fish_candidates("sbrs get-text --css "),
        Vec::<String>::new()
    );
    assert_eq!(fish_candidates("sbrs open "), Vec::<String>::new());
    // Folders only for a command that takes one.
    assert_eq!(fish_candidates("sbrs mkdir "), ["sub/"]);
}

// ---------------------------------------------------------------------------
// bash
// ---------------------------------------------------------------------------

/// Calls the completion function the way bash's readline does: with `COMP_LINE`, `COMP_POINT`,
/// `COMP_WORDS` and `COMP_CWORD` set. The words of `line` are split on spaces; a line that
/// ends in a space ends in an empty word, which is the one being completed.
const BASH_DRIVER: &str = r#"
eval "$(sbrs completions bash)"
COMP_WORDBREAKS=$' \t\n"\'><=;|&(:'
COMP_LINE="$SBRS_LINE"
COMP_POINT=${#COMP_LINE}
read -r -a COMP_WORDS <<< "$COMP_LINE"
[[ $COMP_LINE == *' ' ]] && COMP_WORDS+=('')
COMP_CWORD=$(( ${#COMP_WORDS[@]} - 1 ))
_usage_complete_sbrs
printf '%s\n' "${COMPREPLY[@]}"
"#;

/// The words bash's `COMPREPLY` holds for `line`.
fn bash_candidates(line: &str) -> Vec<String> {
    let dir = workdir();
    let output = Command::new("bash")
        .args(["-c", BASH_DRIVER])
        .current_dir(dir.path())
        .env("PATH", path_with_sbrs())
        .env("SBRS_LINE", line)
        .output()
        .expect("run bash");
    assert!(output.status.success(), "{line:?}: {}", text(&output));
    lines(&output)
        .into_iter()
        .filter(|line| !line.is_empty())
        .collect()
}

#[test]
#[ignore = "runs a real bash"]
fn bash_completes_every_level_and_offers_no_file_for_a_free_text_value() {
    if !installed("bash", &["-c", "true"]) {
        eprintln!("skipped: bash is not installed");
        return;
    }
    assert_eq!(bash_candidates("sbrs plugin "), ["info", "list"]);
    assert_eq!(
        bash_candidates("sbrs grid-hub "),
        ["restart", "start", "status", "stop"]
    );
    assert_eq!(
        bash_candidates("sbrs get-title --hea"),
        ["--headed", "--headless"]
    );
    assert_eq!(
        bash_candidates("sbrs install "),
        [
            "cft",
            "chromedriver",
            "chromium",
            "chs",
            "edgedriver",
            "geckodriver",
            "uc_driver"
        ]
    );
    assert_eq!(bash_candidates("sbrs install chromedriver sta"), ["stable"]);
    // Nothing for a selector or a URL, where bash would offer `only.txt` and `sub`.
    assert_eq!(
        bash_candidates("sbrs get-text --css "),
        Vec::<String>::new()
    );
    assert_eq!(bash_candidates("sbrs open "), Vec::<String>::new());
    // Folders only for a command that takes one.
    assert_eq!(bash_candidates("sbrs mkdir "), ["sub"]);
}

// ---------------------------------------------------------------------------
// PowerShell
// ---------------------------------------------------------------------------

/// The `CompletionText` of each match `TabExpansion2` finds for `$env:SBRS_LINE`.
const PWSH_DRIVER: &str = "
. ([scriptblock]::Create((& sbrs completions powershell | Out-String)))
$line = $env:SBRS_LINE
(TabExpansion2 -inputScript $line -cursorColumn $line.Length).CompletionMatches |
    ForEach-Object { $_.CompletionText }
";

/// The words PowerShell completes `line` to.
fn pwsh_candidates(line: &str) -> Vec<String> {
    let dir = workdir();
    let output = Command::new("pwsh")
        .args(["-NoProfile", "-Command", PWSH_DRIVER])
        .current_dir(dir.path())
        .env("PATH", path_with_sbrs())
        .env("SBRS_LINE", line)
        .output()
        .expect("run pwsh");
    assert!(output.status.success(), "{line:?}: {}", text(&output));
    lines(&output)
        .into_iter()
        .filter(|line| !line.is_empty())
        .collect()
}

#[test]
#[ignore = "runs a real PowerShell"]
fn powershell_completes_every_level_after_a_space() {
    if !installed("pwsh", &["-NoProfile", "-Command", "exit 0"]) {
        eprintln!("skipped: pwsh is not installed");
        return;
    }
    let mut plugin = pwsh_candidates("sbrs plugin ");
    plugin.sort();
    assert_eq!(plugin, ["info", "list"]);
    let mut grid = pwsh_candidates("sbrs grid-hub ");
    grid.sort();
    assert_eq!(grid, ["restart", "start", "status", "stop"]);
    let mut flags = pwsh_candidates("sbrs get-title --hea");
    flags.sort();
    assert_eq!(flags, ["--headed", "--headless"]);
    let mut drivers = pwsh_candidates("sbrs install ");
    drivers.sort();
    assert_eq!(
        drivers,
        [
            "cft",
            "chromedriver",
            "chromium",
            "chs",
            "edgedriver",
            "geckodriver",
            "uc_driver"
        ]
    );
    assert_eq!(pwsh_candidates("sbrs install chromedriver sta"), ["stable"]);
}
