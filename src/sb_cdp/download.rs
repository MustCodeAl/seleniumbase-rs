//! Downloads on the Pure CDP engine.
//!
//! A download is started in one of two ways, and both end the same way: the
//! browser is told to save into a folder and to report what it does
//! (`Browser.setDownloadBehavior` with `eventsEnabled`), something starts a
//! download, and the events that follow say what became of it.
//!
//! * [`Page::download_file`] and [`Page::save_file_as`] have the page fetch an
//!   address and save what comes back, as SeleniumBase's `sb.cdp.download_file`
//!   and `save_file_as` do. The page does the fetching, so it sends the page's
//!   cookies, and an address of another site has to allow it (CORS).
//! * [`Page::download_by_click`] clicks a link and follows the download the
//!   click starts. A link the server answers with `Content-Disposition:
//!   attachment` downloads; a link to a page or an image is shown instead,
//!   and the wait ends in a timeout.
//!
//! `Browser.downloadWillBegin` names the file and `Browser.downloadProgress`
//! ends in `completed`, with the path of the file, or `canceled`. (Chrome sends
//! the `Page.` twins of these events too, but its `completed` one carries no
//! path, so they are ignored.) The wait is bounded
//! by [`Page::timeout`], so a slow download wants
//! [`with_timeout`](Page::with_timeout). Downloads are followed one at a time
//! per browser: the first one to begin after the call starts is the one that
//! is waited for.
//!
//! [`Page::assert_downloaded_file`] looks for a file in the folder the browser
//! saves to, which is [`Browser::download_dir`](super::Browser::download_dir).

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::client::Events;
use super::page::Page;
use crate::artifacts::{bare_file_name, confined_path};
use crate::error::SeleniumBaseError;
use crate::js_code::quote;
use crate::utils::selectors::SelectorBuf;

/// How often [`Page::assert_downloaded_file`] looks in the folder.
const FILE_POLL: Duration = Duration::from_millis(50);

/// The page function behind [`Page::download_file`] and
/// [`Page::save_file_as`]: it fetches `url`, and saves what comes back by
/// clicking a link to it. `forced` is the name to save under, or `null`, when
/// the name is the server's (`Content-Disposition`), else the address's.
///
/// Fetching in the page, as SeleniumBase does, sends the page's cookies and
/// reports a status above 399 as an error instead of saving the error page.
const FETCH_AND_SAVE: &str = r#"async (url, forced) => {
    const response = await fetch(url);
    if (!response.ok) {
        throw new Error('HTTP ' + response.status + ' ' + response.statusText);
    }
    const decode = (text) => {
        try { return decodeURIComponent(text); } catch (error) { return text; }
    };
    const disposition = /filename\*?=(?:UTF-8'')?"?([^";]+)"?/i.exec(
        response.headers.get('content-disposition') || ''
    );
    const last = new URL(url, location.href).pathname.split('/').pop();
    const name = forced
        || (disposition && decode(disposition[1].trim()))
        || decode(last)
        || 'download';
    const href = URL.createObjectURL(await response.blob());
    const link = document.createElement('a');
    link.href = href;
    link.download = name;
    link.style.display = 'none';
    (document.body || document.documentElement).appendChild(link);
    link.click();
    setTimeout(() => { link.remove(); URL.revokeObjectURL(href); }, 1000);
    return name;
}"#;

/// The expression that has the page fetch `url` and save it under `name`, or
/// under the name the server or the address gives when `name` is `None`.
fn fetch_script(url: &str, name: Option<&str>) -> String {
    let name = name.map_or_else(|| "null".to_owned(), quote);
    format!("({FETCH_AND_SAVE})({}, {name})", quote(url))
}

/// A download the browser has begun.
#[derive(Debug)]
struct Started {
    /// The browser's identifier for the download.
    guid: String,
    /// The file name the browser would save it under.
    suggested: String,
    /// Where the data comes from: a `blob:` address for a fetched file.
    source: String,
}

/// Reads the browser's download events until the first download ends.
///
/// `started` is filled in when one begins, so a caller that gives up can say
/// whether it ever did.
async fn follow(
    events: &mut Events,
    dir: &Path,
    started: &mut Option<Started>,
) -> Result<PathBuf, SeleniumBaseError> {
    while let Some(event) = events.next().await {
        let text = |key: &str| event.params[key].as_str().unwrap_or_default().to_owned();
        match event.method.as_str() {
            "Browser.downloadWillBegin" if started.is_none() => {
                *started = Some(Started {
                    guid: text("guid"),
                    suggested: text("suggestedFilename"),
                    source: text("url"),
                });
            }
            "Browser.downloadProgress" => {
                if let Some(download) = started.as_ref().filter(|d| d.guid == text("guid")) {
                    match event.params["state"].as_str() {
                        Some("completed") => return saved_file(dir, download, &event.params),
                        Some("canceled") => {
                            return Err(SeleniumBaseError::Download {
                                url: download.source.clone(),
                                reason: "the browser cancelled the download".to_owned(),
                            })
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    Err(SeleniumBaseError::browser_disconnected(
        "the connection closed while a download was in progress",
    ))
}

/// The file a finished download left: where the browser says it is, else the
/// name it suggested in `dir`.
fn saved_file(
    dir: &Path,
    download: &Started,
    progress: &Value,
) -> Result<PathBuf, SeleniumBaseError> {
    let path = match progress["filePath"]
        .as_str()
        .filter(|path| !path.is_empty())
    {
        Some(reported) => PathBuf::from(reported),
        None => confined_path(dir, &download.suggested)?,
    };
    if path.is_file() {
        Ok(path)
    } else {
        Err(SeleniumBaseError::Download {
            url: download.source.clone(),
            reason: format!(
                "the browser reported the download as finished, but there is no file at {}",
                path.display()
            ),
        })
    }
}

/// Waits for the first download that begins on `events` to end, for up to
/// `timeout`. `what` names the download in a timeout message.
async fn finished_download(
    mut events: Events,
    dir: &Path,
    what: &str,
    timeout: Duration,
) -> Result<PathBuf, SeleniumBaseError> {
    let mut started = None;
    let outcome = tokio::time::timeout(timeout, follow(&mut events, dir, &mut started)).await;
    match outcome {
        Ok(result) => result,
        Err(_elapsed) => {
            let step = if started.is_some() { "finish" } else { "begin" };
            Err(SeleniumBaseError::wait_timeout(
                format!("{what} to {step}"),
                Some(timeout),
            ))
        }
    }
}

impl Page {
    /// Downloads the file at `url` into `dir` and returns where it was saved.
    ///
    /// The page fetches the address and saves the response, which is what
    /// SeleniumBase's `download_file` does, so the page's cookies go with the
    /// request and an address on another site needs CORS headers: open a page
    /// of the site first. The file is named by the server's
    /// `Content-Disposition` header if it sends one, else by the last part of
    /// the address, else `download`. A file of that name already in `dir` is
    /// replaced: Chrome does not number files whose downloads are controlled
    /// over DevTools. The path returned is the one the browser reports.
    ///
    /// `dir` is created if needed, and from now on is the folder the browser
    /// saves downloads in (see
    /// [`Browser::set_download_dir`](super::Browser::set_download_dir)). The
    /// wait for the download is bounded by [`timeout`](Self::timeout).
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Download`] if the page cannot fetch the
    /// address (a status above 399, a network error, CORS) or the browser
    /// cancels the download, [`SeleniumBaseError::WaitTimeout`] if the download
    /// does not begin or does not end in time, and [`SeleniumBaseError::Io`] if
    /// `dir` cannot be created.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn demo(page: seleniumbase_rs::sb_cdp::Page) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// page.goto("https://example.com").await?;
    /// let path = page.download_file("https://example.com/report.csv", "downloaded_files").await?;
    /// println!("saved {}", path.display());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_file(
        &self,
        url: &str,
        dir: impl AsRef<Path>,
    ) -> Result<PathBuf, SeleniumBaseError> {
        self.fetch_into(url, dir.as_ref(), None).await
    }

    /// Downloads the file at `url` and saves it at exactly `path`.
    ///
    /// Corresponds to SeleniumBase's `save_file_as`. The page fetches the
    /// address as for [`download_file`](Self::download_file), the file takes
    /// the name of the last part of `path`, and the folders in front of it are
    /// created. A file already at `path` is replaced. The returned path is
    /// absolute.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if `path` does not end in a
    /// plain file name, before the browser is asked for anything, and the
    /// errors of [`download_file`](Self::download_file) otherwise.
    pub async fn save_file_as(
        &self,
        url: &str,
        path: impl AsRef<Path>,
    ) -> Result<PathBuf, SeleniumBaseError> {
        let path = path.as_ref();
        let name = path.file_name().and_then(OsStr::to_str).ok_or_else(|| {
            SeleniumBaseError::InvalidConfig(format!(
                "{} does not end in a file name",
                path.display()
            ))
        })?;
        bare_file_name(name)?;
        let dir = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let saved = self.fetch_into(url, dir, Some(name)).await?;
        let target = fs::canonicalize(dir)?.join(name);
        if saved != target {
            // The browser saved the file under another name than it was asked to,
            // for example because it numbered it to keep one that was there.
            if target.exists() {
                fs::remove_file(&target)?;
            }
            fs::rename(&saved, &target)?;
        }
        Ok(target)
    }

    /// Clicks the element `selector` selects and returns the file the click
    /// downloads.
    ///
    /// Use it for a link or button that the server answers with a download,
    /// such as `Content-Disposition: attachment`. The file keeps the name the
    /// browser gives it. [`download_file`](Self::download_file) says what
    /// happens to a file of that name already in `dir`, what happens to `dir`
    /// and how long the wait lasts.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if the click starts no
    /// download in time (a link to a page or an image is displayed, not
    /// downloaded) or the download does not end, [`SeleniumBaseError::Download`]
    /// if the browser cancels it, and the errors of
    /// [`Locator::click`](super::Locator::click) if the element cannot be clicked.
    pub async fn download_by_click(
        &self,
        selector: impl Into<SelectorBuf>,
        dir: impl AsRef<Path>,
    ) -> Result<PathBuf, SeleniumBaseError> {
        let selector = selector.into();
        let what = format!("the download started by clicking {selector}");
        let dir = self.browser().start_downloads_in(dir.as_ref()).await?;
        // Listen first: the download can begin before the click returns.
        let events = self.browser().events();
        self.locator(selector).click().await?;
        finished_download(events, &dir, &what, self.timeout()).await
    }

    /// Waits for the file `name` to be in the folder downloads are saved in,
    /// and returns its path.
    ///
    /// Corresponds to SeleniumBase's `assert_downloaded_file`. The folder is
    /// [`Browser::download_dir`](super::Browser::download_dir). A file the
    /// browser has not finished (`name.crdownload`) does not count.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if `name` is not a plain
    /// file name (it cannot name a file outside the folder), and
    /// [`SeleniumBaseError::WaitTimeout`] naming the file and the folder if it
    /// is not there after `timeout`.
    pub async fn assert_downloaded_file(
        &self,
        name: &str,
        timeout: Duration,
    ) -> Result<PathBuf, SeleniumBaseError> {
        let dir = self.browser().download_dir();
        let path = confined_path(&dir, name)?;
        let deadline = Instant::now() + timeout;
        loop {
            if path.is_file() {
                return Ok(fs::canonicalize(&path).unwrap_or(path));
            }
            if Instant::now() >= deadline {
                return Err(SeleniumBaseError::wait_timeout(
                    format!(
                        "the file '{name}' to appear in the downloads folder {}, \
                         or its download to finish",
                        dir.display()
                    ),
                    Some(timeout),
                ));
            }
            tokio::time::sleep(FILE_POLL).await;
        }
    }

    /// Has the page fetch `url` into `dir`, and follows the download it
    /// starts. `name` fixes the file's name.
    async fn fetch_into(
        &self,
        url: &str,
        dir: &Path,
        name: Option<&str>,
    ) -> Result<PathBuf, SeleniumBaseError> {
        let dir = self.browser().start_downloads_in(dir).await?;
        // Listen first: the download can begin before the script returns.
        let events = self.browser().events();
        if let Err(error) = self.evaluate(fetch_script(url, name)).await {
            return Err(match error {
                SeleniumBaseError::CdpDriver(reason) => SeleniumBaseError::Download {
                    url: url.to_owned(),
                    reason,
                },
                other => other,
            });
        }
        finished_download(
            events,
            &dir,
            &format!("the download of {url}"),
            self.timeout(),
        )
        .await
    }
}
