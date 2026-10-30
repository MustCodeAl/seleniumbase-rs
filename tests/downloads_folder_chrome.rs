//! Downloads land in `./downloaded_files` on both engines, against a real Chrome.
//!
//! Python SeleniumBase saves what a browser downloads in `downloaded_files/`,
//! in the folder the tests run from. The two tests start a Chrome of each
//! engine (WebDriver through `BaseCase`, and the pure CDP engine), download a
//! file that a loopback server marks `Content-Disposition: attachment`, and
//! look for it in that folder without having chosen any folder. The tests
//! change the working directory of the process to a fresh temporary folder
//! first, so nothing is left in the project.
//!
//! They need a Chrome or Chromium on the machine (the WebDriver one also a
//! chromedriver), so they are ignored by default. Run them one test binary at a
//! time with:
//!
//! ```text
//! cargo test --test downloads_folder_chrome -- --ignored
//! ```

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::{BaseCase, BrowserConfig};

const HOME: &str = r##"<!doctype html><html><head><title>Downloads</title></head><body>
<a id="file" href="/files/from-cdp.csv" download>Export</a>
</body></html>"##;

/// Moves the working directory of the process to a fresh folder, once, and
/// returns it (canonical, as the browsers report their paths).
fn work_dir() -> &'static PathBuf {
    static DIR: OnceLock<(tempfile::TempDir, PathBuf)> = OnceLock::new();
    &DIR.get_or_init(|| {
        let dir = tempfile::tempdir().expect("a temporary folder");
        let path = dir.path().canonicalize().expect("its canonical path");
        std::env::set_current_dir(&path).expect("work in the temporary folder");
        (dir, path)
    })
    .1
}

/// Answers one connection: the page, or a file named by the path that follows
/// `/files/`, marked as an attachment.
fn answer(mut stream: TcpStream) {
    let mut buffer = [0_u8; 8192];
    let read = stream.read(&mut buffer).unwrap_or(0);
    let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .unwrap_or("/")
        .to_owned();
    let response = if let Some(name) = path.strip_prefix("/files/") {
        let body = "id,name\n1,Ada\n";
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/csv\r\n\
             Content-Disposition: attachment; filename=\"{name}\"\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    } else {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\
             Connection: close\r\n\r\n{HOME}",
            HOME.len()
        )
    };
    let _ = stream.write_all(response.as_bytes());
}

/// Starts the server and returns its `http://127.0.0.1:port` base.
fn serve() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let base = format!("http://{}", listener.local_addr().expect("bound address"));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            // A thread each: Chrome opens idle connections ahead of time.
            std::thread::spawn(move || answer(stream));
        }
    });
    base
}

#[tokio::test]
#[ignore = "launches a real Chrome and chromedriver"]
async fn a_webdriver_download_lands_in_downloaded_files() {
    let folder = work_dir().join("downloaded_files");
    let base = serve();
    let config = BrowserConfig {
        headless: true,
        auto_start_driver: true,
        ..BrowserConfig::default()
    };
    let mut sb = BaseCase::new(config).await.expect("start Chrome");
    assert_eq!(sb.get_downloads_folder(), folder);

    let saved = sb
        .download_file_in_browser(
            &format!("{base}/files/from-webdriver.csv"),
            "from-webdriver.csv",
            20,
        )
        .await
        .expect("the file downloads");

    assert_eq!(saved, folder.join("from-webdriver.csv"));
    assert_eq!(fs::read_to_string(&saved).unwrap(), "id,name\n1,Ada\n");
    assert!(sb.is_downloaded_file_present("from-webdriver.csv"));
    sb.assert_downloaded_file("from-webdriver.csv")
        .await
        .expect("the file is where assert_downloaded_file looks");
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn a_cdp_download_lands_in_downloaded_files_before_a_folder_is_chosen() {
    let folder = work_dir().join("downloaded_files");
    let base = serve();
    let options = LaunchOptions::builder()
        .headless(true)
        .no_sandbox(std::env::var_os("CI").is_some())
        .window_size(1000, 800)
        .build()
        .unwrap();
    let browser = Browser::launch(options).await.expect("Chrome starts");
    let page = browser
        .default_page()
        .await
        .unwrap()
        .with_timeout(Duration::from_secs(15));
    page.goto(format!("{base}/")).await.unwrap();

    // No folder has been chosen: a click on a download link must still save
    // the file in ./downloaded_files.
    page.locator("#file").click().await.expect("click the link");
    let saved = page
        .assert_downloaded_file("from-cdp.csv", Duration::from_secs(15))
        .await
        .expect("the file is in downloaded_files");

    assert_eq!(saved, folder.join("from-cdp.csv"));
    assert_eq!(fs::read_to_string(&saved).unwrap(), "id,name\n1,Ada\n");
    assert_eq!(browser.download_dir(), folder);
    browser.close().await.expect("close");
}
