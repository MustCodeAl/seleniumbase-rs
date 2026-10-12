// Firefox add-ons: `install_addon` and `uninstall_addon`, which send geckodriver's
// `moz/addon/install` and `moz/addon/uninstall` commands. Included at the end of
// `base_case.rs` like the files beside it, so it takes that file's imports and
// names everything else in full.

/// geckodriver's add-on commands, which the W3C WebDriver standard does not have.
///
/// `thirtyfour` has an install command, but it throws the add-on id away, and it
/// has no uninstall command, so the two are sent from here. Each is a `POST` to a
/// `moz/addon` endpoint below the session, with a JSON body.
#[derive(Debug)]
enum GeckoAddonCommand {
    /// `POST /session/{id}/moz/addon/install`: `{"path": ..., "temporary": ...}`.
    Install { path: String, temporary: bool },
    /// `POST /session/{id}/moz/addon/uninstall`: `{"id": ...}`.
    Uninstall { id: String },
}

impl thirtyfour::common::command::FormatRequestData for GeckoAddonCommand {
    fn format_request(&self, session_id: &thirtyfour::SessionId) -> thirtyfour::RequestData {
        let (action, body) = match self {
            Self::Install { path, temporary } => (
                "install",
                serde_json::json!({ "path": path, "temporary": temporary }),
            ),
            Self::Uninstall { id } => ("uninstall", serde_json::json!({ "id": id })),
        };
        thirtyfour::RequestData {
            method: "POST".parse().expect("POST is an HTTP method"),
            uri: format!("/session/{session_id}/moz/addon/{action}").into(),
            body: Some(body),
        }
    }
}

/// The absolute path geckodriver is asked to install, or why `path` is refused.
///
/// The add-on has to be an `.xpi` file or a directory, it has to exist, and the
/// argument may not go up through `..`: the add-on is named from where it is.
fn addon_path(path: &str) -> Result<String, SeleniumBaseError> {
    use std::path::{Component, Path};

    let refuse =
        |why: &str| SeleniumBaseError::InvalidConfig(format!("the add-on path {path:?} {why}"));
    if path.trim().is_empty() {
        return Err(refuse("is empty"));
    }
    if path.contains('\0') {
        return Err(refuse("has a NUL character in it"));
    }
    let given = Path::new(path);
    if given
        .components()
        .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(refuse(
            "goes up through `..`; name the add-on from where it is, without going up",
        ));
    }
    // Not `canonicalize`: on Windows that gives a `\\?\` path geckodriver may not read.
    // There is no `..` left to resolve, and a link is followed by `metadata`.
    let absolute = std::path::absolute(given).map_err(|error| refuse(&error.to_string()))?;
    let found = std::fs::metadata(&absolute)
        .map_err(|error| refuse(&format!("cannot be read ({error})")))?;
    let is_xpi = absolute
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("xpi"));
    if !(found.is_dir() || (found.is_file() && is_xpi)) {
        return Err(refuse(
            "is neither an .xpi file nor a directory that holds an unpacked add-on",
        ));
    }
    absolute
        .into_os_string()
        .into_string()
        .map_err(|_| refuse("is not valid UTF-8, which the JSON request needs"))
}

impl BaseCase {
    /// Installs a Firefox add-on through geckodriver and returns its id, which
    /// [`uninstall_addon`](Self::uninstall_addon) takes. Corresponds to Python's
    /// `install_addon(xpi_file)`, which installs a temporary add-on.
    ///
    /// `path` is an `.xpi` file or a directory that holds an unpacked add-on (a
    /// `manifest.json` and what it names). It is sent to geckodriver as an
    /// absolute path, so geckodriver has to run on the machine that holds the
    /// file. A `temporary` add-on lasts until the browser quits and may be
    /// unsigned; one that is not temporary has to be signed by Mozilla.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Unsupported`] unless the session is a
    /// Firefox one (Chromium browsers load their extensions at launch, through
    /// [`BrowserConfig::with_extension_dir`]), and
    /// [`SeleniumBaseError::InvalidConfig`] if `path` is empty, goes up through
    /// `..`, does not exist, or is neither an `.xpi` file nor a directory;
    /// nothing is sent to the driver in these cases. A refusal by geckodriver
    /// (a broken add-on, say) is a [`SeleniumBaseError::WebDriver`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn demo(sb: &seleniumbase_rs::BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// let id = sb.install_addon("addons/dark-mode.xpi", true).await?;
    /// // ... drive the page with the add-on in place ...
    /// sb.uninstall_addon(&id).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn install_addon(
        &self,
        path: &str,
        temporary: bool,
    ) -> Result<String, SeleniumBaseError> {
        self.require_firefox("install_addon")?;
        let path = addon_path(path)?;
        let reply = self
            .session
            .driver()
            .cmd(GeckoAddonCommand::Install { path, temporary })
            .await?;
        Ok(reply.value::<String>()?)
    }

    /// Removes a Firefox add-on that [`install_addon`](Self::install_addon)
    /// installed, given the id it returned.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Unsupported`] unless the session is a
    /// Firefox one, [`SeleniumBaseError::InvalidConfig`] if `id` is empty (nothing
    /// is sent), and [`SeleniumBaseError::WebDriver`] if geckodriver refuses.
    pub async fn uninstall_addon(&self, id: &str) -> Result<(), SeleniumBaseError> {
        self.require_firefox("uninstall_addon")?;
        if id.trim().is_empty() {
            return Err(SeleniumBaseError::InvalidConfig(
                "uninstall_addon needs the id that install_addon returned, not an empty one"
                    .to_owned(),
            ));
        }
        self.session
            .driver()
            .cmd(GeckoAddonCommand::Uninstall { id: id.to_owned() })
            .await?;
        Ok(())
    }

    /// Refuses, naming `what`, unless the session is a Firefox one.
    fn require_firefox(&self, what: &str) -> Result<(), SeleniumBaseError> {
        if self.config.browser == crate::browser::config::Browser::Firefox {
            return Ok(());
        }
        Err(SeleniumBaseError::Unsupported(format!(
            "{what} needs a Firefox session, because add-ons are installed through geckodriver; \
             this one is {}. A Chromium browser loads its extensions at launch, with \
             BrowserConfig::with_extension_dir or with_extension_archive.",
            self.config.browser
        )))
    }
}

#[cfg(test)]
mod tests_addon {
    use super::webdriver_support::{MockWebDriver, Reply};
    use super::*;
    use crate::browser::config::Browser;
    use serde_json::json;
    use thirtyfour::common::command::FormatRequestData;

    /// What the mock geckodriver answers an install with.
    const ADDON_ID: &str = "sb-test@example.org";

    /// A WebDriver server that answers the two add-on commands as geckodriver does:
    /// the id for an install, and nothing for an uninstall.
    fn gecko() -> MockWebDriver {
        MockWebDriver::start(json!({ "browserName": "firefox" }), |request| {
            match (request.method.as_str(), request.path.as_str()) {
                ("POST", "/moz/addon/install") => Reply::Value(json!(ADDON_ID)),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    /// A Firefox session on `driver`, which starts no driver of its own.
    async fn firefox(driver: &MockWebDriver) -> BaseCase {
        let mut config = BrowserConfig::default()
            .with_webdriver_url(driver.url())
            .with_browser(Browser::Firefox);
        config.auto_start_driver = false;
        BaseCase::new(config)
            .await
            .expect("the mock WebDriver accepts a Firefox session")
    }

    /// An add-on file called `name` in `dir`.
    fn xpi(dir: &tempfile::TempDir, name: &str) -> String {
        let path = dir.path().join(name);
        std::fs::write(&path, b"PK\x03\x04").expect("a file is written");
        path.to_str().expect("a UTF-8 path").to_owned()
    }

    /// The requests that reached geckodriver's add-on endpoints.
    fn addon_requests(driver: &MockWebDriver) -> Vec<String> {
        driver
            .all_requests()
            .into_iter()
            .filter(|request| request.path.starts_with("/moz/"))
            .map(|request| format!("{} {}", request.method, request.path))
            .collect()
    }

    #[tokio::test]
    async fn an_install_sends_the_path_and_the_temporary_flag_and_returns_the_id() {
        let driver = gecko();
        let sb = firefox(&driver).await;
        let dir = tempfile::tempdir().unwrap();
        let file = xpi(&dir, "my-addon.xpi");

        let id = sb.install_addon(&file, true).await.expect("installs");

        assert_eq!(id, ADDON_ID);
        let sent = driver.requests("/moz/addon/install");
        assert_eq!(sent.len(), 1, "{sent:?}");
        assert_eq!(sent[0].method, "POST");
        assert_eq!(sent[0].body, json!({ "path": file, "temporary": true }));
    }

    #[tokio::test]
    async fn a_permanent_install_says_so() {
        let driver = gecko();
        let sb = firefox(&driver).await;
        let dir = tempfile::tempdir().unwrap();
        let file = xpi(&dir, "signed.XPI");

        sb.install_addon(&file, false).await.expect("installs");

        let sent = driver.requests("/moz/addon/install");
        assert_eq!(sent[0].body, json!({ "path": file, "temporary": false }));
    }

    #[tokio::test]
    async fn an_unpacked_add_on_directory_is_installed_too() {
        let driver = gecko();
        let sb = firefox(&driver).await;
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("manifest.json"), b"{}").unwrap();
        let folder = dir.path().to_str().unwrap().to_owned();

        sb.install_addon(&folder, true).await.expect("installs");

        let sent = driver.requests("/moz/addon/install");
        assert_eq!(sent[0].body, json!({ "path": folder, "temporary": true }));
    }

    #[tokio::test]
    async fn an_uninstall_sends_the_id() {
        let driver = gecko();
        let sb = firefox(&driver).await;

        sb.uninstall_addon(ADDON_ID).await.expect("uninstalls");

        let sent = driver.requests("/moz/addon/uninstall");
        assert_eq!(sent.len(), 1, "{sent:?}");
        assert_eq!(sent[0].method, "POST");
        assert_eq!(sent[0].body, json!({ "id": ADDON_ID }));
    }

    #[tokio::test]
    async fn a_session_that_is_not_firefox_is_refused_and_sent_nothing() {
        let driver = gecko();
        // `base_case` starts a Chrome session.
        let sb = driver.base_case().await;
        let dir = tempfile::tempdir().unwrap();
        let file = xpi(&dir, "my-addon.xpi");

        let install = sb
            .install_addon(&file, true)
            .await
            .expect_err("not Firefox");
        let uninstall = sb.uninstall_addon(ADDON_ID).await.expect_err("not Firefox");

        for error in [install, uninstall] {
            let SeleniumBaseError::Unsupported(message) = &error else {
                panic!("expected Unsupported, got {error:?}");
            };
            assert!(
                message.contains("Firefox") && message.contains("chrome"),
                "{message}"
            );
        }
        assert!(addon_requests(&driver).is_empty());
    }

    #[tokio::test]
    async fn a_path_geckodriver_could_not_use_is_refused_before_anything_is_sent() {
        let driver = gecko();
        let sb = firefox(&driver).await;
        let dir = tempfile::tempdir().unwrap();
        let file = xpi(&dir, "my-addon.xpi");
        let notes = xpi(&dir, "notes.txt");
        let beside = format!("{}/sub/../my-addon.xpi", dir.path().to_str().unwrap());
        let missing = format!("{}/missing.xpi", dir.path().to_str().unwrap());

        let refused = [
            ("", "empty"),
            ("  ", "blank"),
            (missing.as_str(), "nothing there"),
            (notes.as_str(), "not an .xpi"),
            (beside.as_str(), ".."),
            ("../my-addon.xpi", ".."),
            ("my-addon\0.xpi", "NUL"),
        ];
        for (path, why) in refused {
            let error = sb
                .install_addon(path, true)
                .await
                .expect_err(&format!("{path:?} ({why}) is refused"));
            assert!(
                matches!(error, SeleniumBaseError::InvalidConfig(_)),
                "{path:?}: {error:?}"
            );
        }
        // The one that is fine is not refused for being near the others.
        sb.install_addon(&file, true).await.expect("installs");
        assert_eq!(addon_requests(&driver), ["POST /moz/addon/install"]);
    }

    #[tokio::test]
    async fn an_add_on_cannot_be_uninstalled_without_its_id() {
        let driver = gecko();
        let sb = firefox(&driver).await;

        for id in ["", "   "] {
            let error = sb.uninstall_addon(id).await.expect_err("needs an id");
            assert!(
                matches!(error, SeleniumBaseError::InvalidConfig(_)),
                "{error:?}"
            );
        }
        assert!(addon_requests(&driver).is_empty());
    }

    #[tokio::test]
    async fn a_refusal_by_geckodriver_is_a_webdriver_error() {
        let driver = MockWebDriver::start(json!({ "browserName": "firefox" }), |_| {
            Reply::Error(400, "invalid argument")
        });
        let sb = firefox(&driver).await;
        let dir = tempfile::tempdir().unwrap();
        let file = xpi(&dir, "broken.xpi");

        let install = sb.install_addon(&file, true).await.expect_err("refused");
        let uninstall = sb
            .uninstall_addon("gone@example.org")
            .await
            .expect_err("refused");

        assert!(
            matches!(install, SeleniumBaseError::WebDriver(_)),
            "{install:?}"
        );
        assert!(
            matches!(uninstall, SeleniumBaseError::WebDriver(_)),
            "{uninstall:?}"
        );
    }

    #[test]
    fn the_commands_are_posts_to_the_moz_addon_endpoints() {
        let session = thirtyfour::SessionId::from("abc");

        let install = GeckoAddonCommand::Install {
            path: "/tmp/a.xpi".to_owned(),
            temporary: true,
        }
        .format_request(&session);
        assert_eq!(install.method.as_str(), "POST");
        assert_eq!(&*install.uri, "/session/abc/moz/addon/install");
        assert_eq!(
            install.body,
            Some(json!({ "path": "/tmp/a.xpi", "temporary": true }))
        );

        let uninstall = GeckoAddonCommand::Uninstall {
            id: ADDON_ID.to_owned(),
        }
        .format_request(&session);
        assert_eq!(uninstall.method.as_str(), "POST");
        assert_eq!(&*uninstall.uri, "/session/abc/moz/addon/uninstall");
        assert_eq!(uninstall.body, Some(json!({ "id": ADDON_ID })));
    }

    #[test]
    fn a_relative_path_is_sent_as_an_absolute_one() {
        // A unit test runs in the package root, which has a `src` folder.
        let path = addon_path("src").expect("a folder is an add-on location");
        assert!(std::path::Path::new(&path).is_absolute(), "{path}");
        assert!(path.ends_with("src"), "{path}");
    }
}
