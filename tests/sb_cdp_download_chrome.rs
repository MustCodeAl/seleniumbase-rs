//! `Page::download_file`, `download_by_click`, `save_file_as` and
//! `assert_downloaded_file` against a real Chrome.
//!
//! A loopback server serves a page with a link to a file that the server marks
//! `Content-Disposition: attachment`, a plain text file, an export whose name
//! only the header gives, and a missing file. These tests need a Chrome or
//! Chromium on the machine, so they are ignored by default. Run them one test
//! binary at a time with:
//!
//! ```text
//! cargo test --test sb_cdp_download_chrome -- --ignored
//! ```

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, Page};
use seleniumbase_rs::SeleniumBaseError;

const HOME: &str = r##"<!doctype html><html><head><title>Downloads</title></head><body>
<a id="attach" href="/attachment">Invoice</a>
<a id="plain" href="/files/notes.txt">Notes</a>
</body></html>"##;

/// Answers one connection.
fn answer(mut stream: TcpStream) {
    let mut buffer = [0_u8; 8192];
    let read = stream.read(&mut buffer).unwrap_or(0);
    let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .unwrap_or("/");
    let (status, headers, body) = match path {
        "/" => ("200 OK", "Content-Type: text/html\r\n", HOME),
        "/attachment" => (
            "200 OK",
            "Content-Type: text/plain\r\nContent-Disposition: attachment; filename=\"invoice.txt\"\r\n",
            "invoice body",
        ),
        "/files/notes.txt" => ("200 OK", "Content-Type: text/plain\r\n", "plain notes"),
        "/export?id=3" => (
            "200 OK",
            "Content-Type: text/csv\r\nContent-Disposition: attachment; filename=\"export-3.csv\"\r\n",
            "id\n3\n",
        ),
        _ => ("404 Not Found", "Content-Type: text/plain\r\n", "no such file"),
    };
    let response = format!(
        "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
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

async fn open(base: &str) -> (Browser, Page) {
    let options = LaunchOptions::builder()
        .headless(true)
        .no_sandbox(std::env::var_os("CI").is_some())
        .window_size(1000, 800)
        .build()
        .unwrap();
    let browser = Browser::launch(options).await.expect("Chrome starts");
    let page = browser.default_page().await.unwrap();
    page.goto(format!("{base}/")).await.unwrap();
    (browser, page.with_timeout(Duration::from_secs(15)))
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn files_are_downloaded_by_address_by_click_and_under_a_chosen_name() {
    let base = serve();
    let (browser, page) = open(&base).await;
    let folder = tempfile::tempdir().unwrap();
    let dir = folder.path().canonicalize().unwrap();

    // By address: the name comes from the address when the server gives none.
    let notes = page
        .download_file(&format!("{base}/files/notes.txt"), &dir)
        .await
        .expect("the notes download");
    assert_eq!(notes, dir.join("notes.txt"));
    assert_eq!(fs::read_to_string(&notes).unwrap(), "plain notes");

    // ... and from the server's header when it does.
    let export = page
        .download_file(&format!("{base}/export?id=3"), &dir)
        .await
        .expect("the export downloads");
    assert_eq!(export, dir.join("export-3.csv"));
    assert_eq!(fs::read_to_string(&export).unwrap(), "id\n3\n");

    // A name already taken is replaced: Chrome does not number a download it
    // was told to allow.
    fs::write(&notes, "stale").unwrap();
    let again = page
        .download_file(&format!("{base}/files/notes.txt"), &dir)
        .await
        .expect("the notes download again");
    assert_eq!(again, notes);
    assert_eq!(fs::read_to_string(&notes).unwrap(), "plain notes");
    assert_eq!(
        fs::read_dir(&dir).unwrap().count(),
        2,
        "notes.txt and export-3.csv, nothing numbered"
    );

    // Under a name of your choosing, replacing what is there.
    let copy = dir.join("kept").join("copy.txt");
    let saved = page
        .save_file_as(&format!("{base}/files/notes.txt"), &copy)
        .await
        .expect("the copy is saved");
    assert_eq!(saved, copy);
    assert_eq!(fs::read_to_string(&copy).unwrap(), "plain notes");
    fs::write(&copy, "stale").unwrap();
    page.save_file_as(&format!("{base}/files/notes.txt"), &copy)
        .await
        .expect("the copy is saved over");
    assert_eq!(fs::read_to_string(&copy).unwrap(), "plain notes");
    assert_eq!(fs::read_dir(dir.join("kept")).unwrap().count(), 1);

    // By click: the server's header names the file.
    let invoice = page
        .download_by_click("#attach", &dir)
        .await
        .expect("the invoice downloads");
    assert_eq!(invoice, dir.join("invoice.txt"));
    assert_eq!(fs::read_to_string(&invoice).unwrap(), "invoice body");

    // The page is still the page it was.
    assert_eq!(page.title().await.unwrap(), "Downloads");

    // Files are found in the folder the browser saves to.
    let found = page
        .assert_downloaded_file("invoice.txt", Duration::from_secs(2))
        .await
        .expect("the invoice is in the folder");
    assert_eq!(found, invoice);
    let missing = page
        .assert_downloaded_file("nothing.txt", Duration::from_millis(300))
        .await
        .expect_err("there is no such file");
    assert!(
        matches!(missing, SeleniumBaseError::WaitTimeout(_)),
        "{missing:?}"
    );

    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn a_missing_file_and_a_click_that_downloads_nothing_are_reported() {
    let base = serve();
    let (browser, page) = open(&base).await;
    let folder = tempfile::tempdir().unwrap();

    let error = page
        .download_file(&format!("{base}/no/such/file.bin"), folder.path())
        .await
        .expect_err("the server answers 404");
    assert!(
        matches!(&error, SeleniumBaseError::Download { reason, .. } if reason.contains("404")),
        "{error:?}"
    );

    // The notes link is shown by the browser instead of downloaded.
    let quick = page.with_timeout(Duration::from_secs(2));
    let error = quick
        .download_by_click("#plain", folder.path())
        .await
        .expect_err("a link to a text file is not a download");
    assert!(
        matches!(error, SeleniumBaseError::WaitTimeout(_)),
        "{error:?}"
    );
    assert_eq!(fs::read_dir(folder.path()).unwrap().count(), 0);

    browser.close().await.unwrap();
}
