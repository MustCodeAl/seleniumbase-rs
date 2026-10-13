//! Upstream command names that `sbrs` accepts: `version`, `obfuscate` and
//! `unobfuscate`, run as the real binary.

use std::process::{Command, Output, Stdio};

const SBASE: &str = env!("CARGO_BIN_EXE_sbrs");

fn run(args: &[&str], passphrase: Option<&str>) -> Output {
    let mut command = Command::new(SBASE);
    command
        .args(args)
        .env_remove("SB_ENCRYPTION_KEY")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(passphrase) = passphrase {
        command.env("SB_ENCRYPTION_KEY", passphrase);
    }
    command.output().expect("sbrs runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn the_version_command_prints_what_the_version_flag_prints() {
    let command = run(&["version"], None);
    let flag = run(&["--version"], None);

    assert!(command.status.success(), "{}", stderr(&command));
    assert!(flag.status.success(), "{}", stderr(&flag));
    assert_eq!(stdout(&command), stdout(&flag));
    assert!(
        stdout(&command).contains(env!("CARGO_PKG_VERSION")),
        "{}",
        stdout(&command)
    );
}

#[test]
fn obfuscate_and_unobfuscate_are_python_names_for_encrypt_and_decrypt() {
    let obfuscated = run(&["obfuscate", "my p@ssw0rd"], Some("correct horse"));
    assert!(obfuscated.status.success(), "{}", stderr(&obfuscated));
    let token = stdout(&obfuscated);
    assert!(token.starts_with("sbenc1:"), "{token}");

    // The same token comes back through either name.
    let by_alias = run(&["unobfuscate", &token], Some("correct horse"));
    let by_name = run(&["decrypt", &token], Some("correct horse"));

    assert!(by_alias.status.success(), "{}", stderr(&by_alias));
    assert_eq!(stdout(&by_alias), "my p@ssw0rd");
    assert_eq!(stdout(&by_name), "my p@ssw0rd");
}
