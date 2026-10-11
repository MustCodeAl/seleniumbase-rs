//! Tests of the identity file: the round trip, the permissions rule, and every way a file can be wrong.

use std::fs;

use super::*;
use crate::scrape::passport::identity::fixtures::identity_with_clearance;
use crate::stealth::fingerprint::ProxyConfig;

/// The value of the clearance cookie in the sample identity. It must be in the file, and in no error.
const COOKIE_VALUE: &str = "SECRET-CLEARANCE-VALUE";

/// A proxy password injected into files the tests tamper with.
const PASSWORD: &str = "hunter2-proxy-password";

fn temp() -> tempfile::TempDir {
    tempfile::tempdir().expect("a temporary directory")
}

/// A saved sample identity: its directory, the file and the identity.
fn saved() -> (tempfile::TempDir, PathBuf, StealthIdentity) {
    let dir = temp();
    let path = dir.path().join("identity.json");
    let identity = identity_with_clearance();
    save_blocking(&path, &identity).expect("saves");
    (dir, path, identity)
}

/// Rewrites the JSON in `path` after `change`, keeping the file's mode.
fn edit(path: &Path, change: impl FnOnce(&mut serde_json::Value)) {
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(path).expect("reads")).expect("valid JSON");
    change(&mut value);
    fs::write(path, serde_json::to_vec_pretty(&value).expect("serializes")).expect("writes");
}

/// Writes `text` to a new file that only its owner can access, as `save_blocking` would.
fn write_private(path: &Path, text: &str) {
    fs::write(path, text).expect("writes");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).expect("chmod");
    }
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .expect("lists")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

/// Why an identity was refused, when `result` is the refusal of one.
fn refusal<T: std::fmt::Debug>(result: Result<T, IdentityFileError>) -> IdentityError {
    match result {
        Err(IdentityFileError::Identity { source, .. }) => *source,
        other => panic!("expected a refused identity, got {other:?}"),
    }
}

/// Asserts that no failure text holds the cookie value or the proxy password.
fn assert_clean(error: &IdentityFileError) {
    for text in [error.to_string(), format!("{error:?}")] {
        assert!(!text.contains(COOKIE_VALUE), "cookie value in {text}");
        assert!(!text.contains(PASSWORD), "proxy password in {text}");
    }
}

#[test]
fn an_identity_survives_a_save_and_a_load() {
    let (_dir, path, identity) = saved();
    assert_eq!(load_blocking(&path).expect("loads"), identity);
}

#[test]
fn the_file_holds_what_a_later_run_needs() {
    let (_dir, path, _identity) = saved();
    let text = fs::read_to_string(&path).expect("reads");
    assert!(text.contains(COOKIE_VALUE), "the cookie must be restorable");
    assert!(text.contains(&format!("\"schema\": {IDENTITY_SCHEMA}")));
    assert!(text.contains('\n'), "pretty printed, one field per line");
}

#[tokio::test]
async fn the_async_functions_do_the_same() {
    let dir = temp();
    let path = dir
        .path()
        .join("nested")
        .join("deeper")
        .join("identity.json");
    let identity = identity_with_clearance();
    save(&path, &identity)
        .await
        .expect("saves, creating the directories");
    assert_eq!(load(&path).await.expect("loads"), identity);
}

#[test]
fn saving_leaves_no_temporary_file_behind() {
    let (dir, _path, _identity) = saved();
    assert_eq!(names(dir.path()), vec!["identity.json".to_owned()]);
}

#[test]
fn saving_again_replaces_the_file() {
    let (_dir, path, mut identity) = saved();
    identity.created_at += 1;
    save_blocking(&path, &identity).expect("saves again");
    assert_eq!(
        load_blocking(&path).expect("loads").created_at,
        identity.created_at
    );
}

#[cfg(unix)]
mod unix {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    use super::*;

    fn mode(path: &Path) -> u32 {
        fs::metadata(path).expect("metadata").permissions().mode() & 0o777
    }

    fn set_mode(path: &Path, mode: u32) {
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).expect("chmod");
    }

    #[test]
    fn the_file_is_written_for_its_owner_alone() {
        let (_dir, path, _identity) = saved();
        assert_eq!(mode(&path), 0o600);
    }

    #[test]
    fn a_wider_file_is_replaced_by_an_owner_only_one_when_saving() {
        let dir = temp();
        let path = dir.path().join("identity.json");
        fs::write(&path, "old").expect("writes");
        set_mode(&path, 0o644);
        save_blocking(&path, &identity_with_clearance()).expect("saves");
        assert_eq!(mode(&path), 0o600);
        assert!(load_blocking(&path).is_ok());
    }

    #[test]
    fn a_file_that_others_can_access_is_rejected() {
        let (_dir, path, _identity) = saved();
        for wide in [0o644, 0o640, 0o604, 0o660, 0o666, 0o664, 0o601, 0o610] {
            set_mode(&path, wide);
            match load_blocking(&path) {
                Err(IdentityFileError::Permissions { mode, .. }) => assert_eq!(mode, wide),
                other => panic!("mode {wide:o} must be rejected, got {other:?}"),
            }
        }
    }

    #[test]
    fn a_stricter_or_owner_only_file_is_accepted() {
        let (_dir, path, identity) = saved();
        for narrow in [0o600, 0o400, 0o700, 0o500] {
            set_mode(&path, narrow);
            assert_eq!(load_blocking(&path).expect("loads"), identity, "{narrow:o}");
        }
    }

    #[test]
    fn a_rejected_file_is_reported_and_not_corrected() {
        let (_dir, path, _identity) = saved();
        set_mode(&path, 0o644);
        let before = fs::read(&path).expect("reads");
        let error = load_blocking(&path).expect_err("rejected");
        assert_eq!(mode(&path), 0o644, "loading must not change permissions");
        assert_eq!(fs::read(&path).expect("reads"), before);
        let text = error.to_string();
        assert!(text.contains("644") && text.contains("chmod 600"), "{text}");
        assert_clean(&error);
    }

    #[tokio::test]
    async fn the_async_load_applies_the_same_rule() {
        let (_dir, path, _identity) = saved();
        set_mode(&path, 0o666);
        assert!(matches!(
            load(&path).await,
            Err(IdentityFileError::Permissions { .. })
        ));
    }

    #[test]
    fn the_user_is_the_owner_of_the_files_the_process_makes() {
        let dir = temp();
        let made = dir.path().join("made");
        fs::write(&made, "x").expect("writes");
        let owner = fs::metadata(&made).expect("metadata").uid();
        assert_eq!(current_user().expect("a user"), owner);
        assert_eq!(current_user().expect("a user"), owner, "asked again");
    }

    #[test]
    fn a_file_owned_by_another_user_is_refused_by_the_check_of_the_handle() {
        let (_dir, path, _identity) = saved();
        let file = fs::File::open(&path).expect("opens");
        let owner = fs::metadata(&path).expect("metadata").uid();

        assert!(check_handle_as(&path, &file, owner).is_ok());
        for other in [owner.wrapping_add(1), owner.wrapping_sub(1)] {
            let error = check_handle_as(&path, &file, other).expect_err("another user");
            assert!(
                matches!(error, IdentityFileError::Owner { .. }),
                "{error:?}"
            );
            assert_clean(&error);
        }
    }

    #[test]
    fn the_owner_is_checked_on_the_open_file_before_anything_is_read() {
        let (_dir, path, _identity) = saved();
        let file = open_for_reading(&path).expect("opens");
        let owner = fs::metadata(&path).expect("metadata").uid();

        // A file that is neither the user's nor private is reported for what it is first.
        set_mode(&path, 0o644);
        let error = check_handle_as(&path, &file, owner.wrapping_add(1)).expect_err("refused");

        assert!(
            matches!(error, IdentityFileError::Owner { .. }),
            "{error:?}"
        );
        assert!(matches!(
            check_handle_as(&path, &file, owner),
            Err(IdentityFileError::Permissions { mode: 0o644, .. })
        ));
    }

    /// Loads `path` on another thread, so that a load that waits for a writer fails the test
    /// instead of stopping it.
    fn load_without_waiting(path: &Path) -> Result<StealthIdentity, IdentityFileError> {
        let (sender, receiver) = std::sync::mpsc::channel();
        let target = path.to_path_buf();
        std::thread::spawn(move || {
            let _sent = sender.send(load_blocking(&target));
        });
        receiver
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("the load waited for a writer")
    }

    #[test]
    fn a_named_pipe_in_place_of_the_file_is_refused_and_not_waited_for() {
        if OPEN_NONBLOCK == 0 {
            return; // This platform opens without the flag; see the constant.
        }
        let dir = temp();
        let path = dir.path().join("identity.json");
        let made = std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .expect("mkfifo runs");
        assert!(made.success());

        match load_without_waiting(&path) {
            Err(IdentityFileError::Io { source, .. }) => {
                assert_eq!(source.kind(), io::ErrorKind::InvalidInput);
            }
            other => panic!("a named pipe must be refused, got {other:?}"),
        }
    }

    #[test]
    fn a_directory_is_not_an_identity_file() {
        let dir = temp();
        match load_without_waiting(dir.path()) {
            Err(IdentityFileError::Io { source, .. }) => {
                assert_eq!(source.kind(), io::ErrorKind::InvalidInput);
            }
            other => panic!("a directory must be refused, got {other:?}"),
        }
    }

    #[test]
    fn a_symlink_to_a_private_file_of_the_user_is_read_like_the_file() {
        let (dir, path, identity) = saved();
        let link = dir.path().join("link.json");
        std::os::unix::fs::symlink(&path, &link).expect("symlinks");

        assert_eq!(load_blocking(&link).expect("loads"), identity);
        set_mode(&path, 0o644);
        assert!(
            matches!(
                load_blocking(&link),
                Err(IdentityFileError::Permissions { .. })
            ),
            "the file the link leads to is the one checked"
        );
    }
}

#[test]
fn a_file_with_another_schema_is_refused_as_that() {
    let (_dir, path, _identity) = saved();
    edit(&path, |value| {
        value["schema"] = serde_json::json!(IDENTITY_SCHEMA + 1);
    });
    assert_eq!(
        refusal(load_blocking(&path)),
        IdentityError::Schema(IDENTITY_SCHEMA + 1)
    );
}

#[test]
fn another_schema_is_reported_even_when_the_rest_would_not_parse() {
    let (_dir, path, _identity) = saved();
    fs::write(&path, r#"{"schema": 7, "something": "else entirely"}"#).expect("writes");
    assert_eq!(refusal(load_blocking(&path)), IdentityError::Schema(7));
}

#[test]
fn a_file_without_a_schema_is_corrupt() {
    let (_dir, path, _identity) = saved();
    edit(&path, |value| {
        value.as_object_mut().expect("an object").remove("schema");
    });
    assert!(matches!(
        load_blocking(&path),
        Err(IdentityFileError::Corrupt { .. })
    ));
}

#[test]
fn text_that_is_not_json_is_corrupt_and_left_alone() {
    let dir = temp();
    let path = dir.path().join("identity.json");
    write_private(&path, "{ this is not json");
    match load_blocking(&path) {
        Err(IdentityFileError::Corrupt { line, column, .. }) => {
            assert!(line >= 1 && column >= 1);
        }
        other => panic!("expected Corrupt, got {other:?}"),
    }
    assert_eq!(
        fs::read_to_string(&path).expect("reads"),
        "{ this is not json"
    );
}

#[test]
fn a_file_that_ends_early_is_corrupt_and_says_so() {
    let dir = temp();
    let path = dir.path().join("identity.json");
    write_private(&path, r#"{"schema": 1, "fingerprint": "#);
    match load_blocking(&path) {
        Err(IdentityFileError::Corrupt { reason, .. }) => {
            assert!(reason.contains("ends too early"), "{reason}");
        }
        other => panic!("expected Corrupt, got {other:?}"),
    }
}

#[test]
fn a_cookie_the_jar_would_refuse_makes_the_file_corrupt() {
    let (_dir, path, _identity) = saved();
    edit(&path, |value| {
        value["cookies"]["cookies"][0]["path"] = serde_json::json!("no-leading-slash");
    });
    let error = load_blocking(&path).expect_err("a tampered jar");
    assert!(
        matches!(error, IdentityFileError::Corrupt { .. }),
        "{error:?}"
    );
    assert_clean(&error);
}

#[test]
fn a_secure_prefix_without_secure_makes_the_file_corrupt() {
    let (_dir, path, _identity) = saved();
    edit(&path, |value| {
        let cookie = &mut value["cookies"]["cookies"][0];
        cookie["name"] = serde_json::json!("__Secure-cf_clearance");
        cookie["secure"] = serde_json::json!(false);
    });
    assert!(matches!(
        load_blocking(&path),
        Err(IdentityFileError::Corrupt { .. })
    ));
}

#[test]
fn a_duplicated_cookie_makes_the_file_corrupt() {
    let (_dir, path, _identity) = saved();
    edit(&path, |value| {
        let cookies = value["cookies"]["cookies"]
            .as_array_mut()
            .expect("an array");
        let again = cookies[0].clone();
        cookies.push(again);
    });
    assert!(matches!(
        load_blocking(&path),
        Err(IdentityFileError::Corrupt { .. })
    ));
}

#[test]
fn a_type_error_does_not_echo_the_value_it_met() {
    let (_dir, path, _identity) = saved();
    edit(&path, |value| {
        value["cookies"]["cookies"][0]["expires"] = serde_json::json!("SECRET-EXPIRES-TEXT");
        value["created_at"] = serde_json::json!("SECRET-CREATED-TEXT");
    });
    let error = load_blocking(&path).expect_err("wrong types");
    assert!(matches!(error, IdentityFileError::Corrupt { .. }));
    for text in [error.to_string(), format!("{error:?}")] {
        assert!(!text.contains("SECRET"), "a value leaked into {text}");
    }
}

#[test]
fn a_cookie_value_of_the_wrong_type_is_not_echoed() {
    let (_dir, path, _identity) = saved();
    edit(&path, |value| {
        value["cookies"]["cookies"][0]["value"] = serde_json::json!(918_273_645);
    });
    let error = load_blocking(&path).expect_err("wrong type");
    for text in [error.to_string(), format!("{error:?}")] {
        assert!(!text.contains("918273645"), "{text}");
    }
}

#[test]
fn a_fingerprint_that_no_longer_fits_its_os_is_refused() {
    let (_dir, path, _identity) = saved();
    edit(&path, |value| {
        value["fingerprint"]["platform"] = serde_json::json!("Win32");
    });
    match refusal(load_blocking(&path)) {
        IdentityError::Incoherent(reasons) => assert_ne!(reasons, Vec::<String>::new()),
        other => panic!("expected Incoherent, got {other:?}"),
    }
}

#[test]
fn a_proxy_smuggled_into_a_file_is_refused_without_echoing_its_password() {
    let (_dir, path, _identity) = saved();
    edit(&path, |value| {
        value["fingerprint"]["proxy"] = serde_json::json!({
            "type": "http", "host": "proxy.example", "port": 8080,
            "username": "alice", "password": PASSWORD, "save_traffic": false
        });
    });
    let result = load_blocking(&path);
    assert_clean(result.as_ref().expect_err("a proxy is refused"));
    assert!(matches!(refusal(result), IdentityError::Incoherent(_)));
}

#[test]
fn an_identity_with_a_proxy_is_not_written() {
    let dir = temp();
    let path = dir.path().join("identity.json");
    let mut identity = identity_with_clearance();
    identity.fingerprint.proxy = Some(ProxyConfig {
        r#type: "http".to_owned(),
        host: "proxy.example".to_owned(),
        port: 8080,
        username: Some("alice".to_owned()),
        password: Some(PASSWORD.to_owned()),
        save_traffic: false,
    });
    let error = save_blocking(&path, &identity).expect_err("refused");
    assert!(matches!(error, IdentityFileError::Identity { .. }));
    assert_clean(&error);
    assert!(!path.exists(), "nothing is written for a refused identity");
}

#[test]
fn an_identity_of_another_schema_is_not_written() {
    let dir = temp();
    let path = dir.path().join("identity.json");
    let mut identity = identity_with_clearance();
    identity.schema = IDENTITY_SCHEMA + 1;
    assert_eq!(
        refusal(save_blocking(&path, &identity)),
        IdentityError::Schema(IDENTITY_SCHEMA + 1)
    );
    assert!(!path.exists());
}

#[test]
fn a_missing_file_is_an_io_error_that_says_so() {
    let dir = temp();
    let error = load_blocking(&dir.path().join("absent.json")).expect_err("missing");
    assert!(error.is_not_found(), "{error:?}");
    let other = IdentityFileError::Corrupt {
        path: PathBuf::from("x"),
        reason: "r",
        line: 1,
        column: 1,
    };
    assert!(!other.is_not_found());
}

#[test]
fn a_directory_is_not_an_identity_file() {
    let dir = temp();
    let error = load_blocking(dir.path()).expect_err("a directory");
    assert!(matches!(error, IdentityFileError::Io { .. }), "{error:?}");
}

#[test]
fn a_file_over_the_limit_is_too_large() {
    let (_dir, path, _identity) = saved();
    match load_with_limit(&path, 10) {
        Err(IdentityFileError::TooLarge { limit, .. }) => assert_eq!(limit, 10),
        other => panic!("expected TooLarge, got {other:?}"),
    }
    let size = fs::metadata(&path).expect("metadata").len();
    assert!(
        load_with_limit(&path, size).is_ok(),
        "exactly the limit is allowed"
    );
    assert!(matches!(
        load_with_limit(&path, size - 1),
        Err(IdentityFileError::TooLarge { .. })
    ));
}

#[test]
fn a_save_that_cannot_happen_is_an_io_error() {
    let dir = temp();
    let blocker = dir.path().join("blocker");
    fs::write(&blocker, "a file where a directory is needed").expect("writes");
    let error = save_blocking(&blocker.join("identity.json"), &identity_with_clearance())
        .expect_err("cannot create a directory under a file");
    assert!(matches!(error, IdentityFileError::Io { .. }), "{error:?}");
    assert_clean(&error);
}
