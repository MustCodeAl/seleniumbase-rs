//! `sbase encrypt` and `sbase decrypt`, run as the real binary.

use std::io::Write;
use std::process::{Command, Output, Stdio};

const SBASE: &str = env!("CARGO_BIN_EXE_sbase");

fn run(args: &[&str], passphrase: Option<&str>, stdin: Option<&str>) -> Output {
    let mut command = Command::new(SBASE);
    command
        .args(args)
        .env_remove("SB_ENCRYPTION_KEY")
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(passphrase) = passphrase {
        command.env("SB_ENCRYPTION_KEY", passphrase);
    }
    let mut child = command.spawn().expect("sbase starts");
    if let Some(text) = stdin {
        child
            .stdin
            .take()
            .expect("piped stdin")
            .write_all(text.as_bytes())
            .expect("write stdin");
    }
    child.wait_with_output().expect("sbase finishes")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn a_secret_survives_an_encrypt_and_decrypt_round_trip() {
    let encrypted = run(&["encrypt", "my p@ssw0rd"], Some("correct horse"), None);
    assert!(encrypted.status.success(), "{}", stderr(&encrypted));
    let token = stdout(&encrypted);
    assert!(token.starts_with("sbenc1:"), "{token}");
    assert!(
        !token.contains("p@ssw0rd"),
        "the token must not contain the secret"
    );

    let decrypted = run(&["decrypt", &token], Some("correct horse"), None);
    assert!(decrypted.status.success(), "{}", stderr(&decrypted));
    assert_eq!(stdout(&decrypted), "my p@ssw0rd");
}

#[test]
fn text_can_come_from_standard_input_so_it_stays_out_of_shell_history() {
    let encrypted = run(&["encrypt"], Some("pass"), Some("piped secret\n"));
    assert!(encrypted.status.success(), "{}", stderr(&encrypted));

    let decrypted = run(
        &["decrypt"],
        Some("pass"),
        Some(&format!("{}\n", stdout(&encrypted))),
    );
    assert!(decrypted.status.success(), "{}", stderr(&decrypted));
    assert_eq!(
        stdout(&decrypted),
        "piped secret",
        "the trailing newline is not part of the text"
    );
}

#[test]
fn without_a_passphrase_the_command_fails_and_says_what_to_set() {
    let output = run(&["encrypt", "x"], None, None);

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("SB_ENCRYPTION_KEY"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn the_wrong_passphrase_fails_rather_than_printing_garbage() {
    let token = stdout(&run(&["encrypt", "secret"], Some("right"), None));

    let output = run(&["decrypt", &token], Some("wrong"), None);

    assert!(!output.status.success());
    assert!(
        stdout(&output).is_empty(),
        "nothing may be printed: {:?}",
        stdout(&output)
    );
}

#[test]
fn a_string_that_is_not_a_token_is_rejected() {
    let output = run(&["decrypt", "just some text"], Some("pass"), None);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("token"), "{}", stderr(&output));
}
