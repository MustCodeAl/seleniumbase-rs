//! How fast the Pure CDP engine drives a real Chrome.
//!
//! Run it in release mode, on a quiet machine, with Chrome installed:
//!
//! ```text
//! cargo bench --bench cdp_latency
//! ```
//!
//! It prints Markdown tables. Every figure is measured here, against a local
//! page, so network time is not in it; what it shows is the cost the library
//! itself adds to a browser that is already running. Timings from a loaded
//! machine mean little: close other work first.

use std::future::Future;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, Page, State};

const FIXTURE: &str = r##"<!doctype html><html><head><title>Bench</title></head><body>
<h1 id="title">Bench page</h1>
<p id="para">some text to read</p>
<button id="btn" onclick="window.__clicks=(window.__clicks||0)+1">Go</button>
<input id="field">
<div style="height:3000px"></div>
</body></html>"##;

/// Serves [`FIXTURE`] for every request on a loopback port.
fn serve() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let address = listener.local_addr().expect("bound address");
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            // One thread per connection: Chrome opens spare connections that
            // never send a request, and a server that waits on each in turn
            // would starve the real ones.
            thread::spawn(move || {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let mut buffer = [0_u8; 1024];
                if stream.read(&mut buffer).unwrap_or(0) == 0 {
                    return;
                }
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{FIXTURE}",
                    FIXTURE.len()
                );
                let _ = stream.write_all(response.as_bytes());
            });
        }
    });
    format!("http://{address}")
}

fn options() -> LaunchOptions {
    LaunchOptions::builder()
        .headless(true)
        .no_sandbox(std::env::var_os("CI").is_some())
        .build()
        .expect("valid launch options")
}

/// Median, 95th percentile and fastest of a set of timings.
struct Spread {
    median: Duration,
    p95: Duration,
    min: Duration,
}

fn spread(mut samples: Vec<Duration>) -> Spread {
    samples.sort();
    let at = |q: f64| samples[((samples.len() - 1) as f64 * q).round() as usize];
    Spread {
        median: at(0.5),
        p95: at(0.95),
        min: samples[0],
    }
}

fn micros(d: Duration) -> String {
    if d >= Duration::from_millis(10) {
        format!("{:.1} ms", d.as_secs_f64() * 1e3)
    } else {
        format!("{:.0} µs", d.as_secs_f64() * 1e6)
    }
}

fn row(label: &str, n: usize, s: &Spread) {
    println!(
        "| {label} | {n} | {} | {} | {} |",
        micros(s.median),
        micros(s.p95),
        micros(s.min)
    );
}

/// Times `op` `n` times, after a short warm-up.
async fn bench<F, Fut>(label: &str, n: usize, mut op: F)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = ()>,
{
    for _ in 0..10 {
        op().await;
    }
    let mut samples = Vec::with_capacity(n);
    for _ in 0..n {
        let started = Instant::now();
        op().await;
        samples.push(started.elapsed());
    }
    row(label, n, &spread(samples));
}

/// How long after an element appears does a wait notice it?
///
/// The page inserts an element 100 ms after a script runs and records its own
/// clock when it does. The wait returns, and the page's clock is read again, so
/// the difference is the detection delay, measured on one clock. It includes
/// one protocol round trip for the final read.
async fn wait_delay(page: &Page, n: usize) -> Spread {
    let mut samples = Vec::with_capacity(n);
    for _ in 0..n {
        page.evaluate(
            "document.getElementById('late') && document.getElementById('late').remove()",
        )
        .await
        .unwrap();
        page.evaluate(
            "(() => { setTimeout(() => { const i = document.createElement('i'); \
             i.id = 'late'; i.textContent = 'x'; document.body.append(i); \
             window.__shown = performance.now(); }, 100); })()",
        )
        .await
        .unwrap();
        page.locator("#late")
            .wait_for(State::Visible)
            .await
            .unwrap();
        let seen: f64 = page.evaluate_as("performance.now()").await.unwrap();
        let shown: f64 = page.evaluate_as("window.__shown").await.unwrap();
        samples.push(Duration::from_secs_f64(((seen - shown) / 1000.0).max(0.0)));
    }
    spread(samples)
}

/// Resident memory of this process, in KiB.
fn rss_kib() -> u64 {
    let pid = std::process::id().to_string();
    Command::new("ps")
        .args(["-o", "rss=", "-p", &pid])
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or(0)
}

#[tokio::main]
async fn main() {
    let url = serve();
    println!("# Pure CDP benchmark\n");
    println!(
        "Rust {}, {} cores, release build.\n",
        option_env!("RUSTC_VERSION").unwrap_or("stable"),
        thread::available_parallelism().map_or(0, usize::from)
    );

    // ----- start-up -----------------------------------------------------
    println!("## Start-up\n");
    println!("| step | runs | median | p95 | fastest |");
    println!("|---|---|---|---|---|");
    let mut launches = Vec::new();
    let mut first_pages = Vec::new();
    for _ in 0..5 {
        let started = Instant::now();
        let browser = Browser::launch(options()).await.expect("Chrome starts");
        launches.push(started.elapsed());
        let started = Instant::now();
        let page = browser.default_page().await.unwrap();
        page.goto(format!("{url}/")).await.unwrap();
        first_pages.push(started.elapsed());
        browser.close().await.unwrap();
    }
    row(
        "launch Chrome to a DevTools connection",
        5,
        &spread(launches),
    );
    row("first page attached and loaded", 5, &spread(first_pages));
    println!();

    // ----- single-command latency --------------------------------------
    let browser = Browser::launch(options()).await.expect("Chrome starts");
    let page = browser.default_page().await.unwrap();
    page.goto(format!("{url}/")).await.unwrap();
    let p = &page;

    println!("## One tab, one command at a time\n");
    println!("| operation | runs | median | p95 | fastest |");
    println!("|---|---|---|---|---|");
    bench("evaluate `1 + 1`", 500, || async move {
        p.evaluate("1 + 1").await.unwrap();
    })
    .await;
    bench("locator count", 500, || async move {
        p.locator("#btn").count().await.unwrap();
    })
    .await;
    bench("read text", 300, || async move {
        p.locator("#para").text().await.unwrap();
    })
    .await;
    bench("click a button", 300, || async move {
        p.locator("#btn").click().await.unwrap();
    })
    .await;
    bench("fill a field (11 chars)", 300, || async move {
        p.locator("#field").fill("hello world").await.unwrap();
    })
    .await;
    bench("type a field key by key (11 chars)", 100, || async move {
        let field = p.locator("#field");
        field.clear().await.unwrap();
        field.type_text("hello world").await.unwrap();
    })
    .await;
    bench("screenshot (viewport PNG)", 30, || async move {
        p.screenshot().await.unwrap();
    })
    .await;
    let navigate = format!("{url}/");
    let nav = &navigate;
    bench("goto a local page", 50, || async move {
        p.goto(nav).await.unwrap();
    })
    .await;
    println!();

    println!("## How long a wait takes to notice an element\n");
    println!("The element appears 100 ms after the script runs; this is the extra delay.\n");
    println!("| operation | runs | median | p95 | fastest |");
    println!("|---|---|---|---|---|");
    row(
        "wait for an element to become visible",
        40,
        &wait_delay(&page, 40).await,
    );
    println!();
    browser.close().await.unwrap();

    // ----- concurrency ----------------------------------------------------
    println!("## Many tabs at once (one Chrome)\n");
    println!("Each tab clicks a button 150 times; tabs run concurrently.\n");
    println!("| tabs | clicks | wall time | clicks / second |");
    println!("|---|---|---|---|");
    let browser = Browser::launch(options()).await.expect("Chrome starts");
    for tabs in [1_usize, 2, 4, 8] {
        let mut pages = Vec::new();
        for _ in 0..tabs {
            pages.push(browser.new_page(Some(format!("{url}/"))).await.unwrap());
        }
        let started = Instant::now();
        let mut tasks = Vec::new();
        for page in pages.clone() {
            tasks.push(tokio::spawn(async move {
                for _ in 0..150 {
                    page.locator("#btn").click().await.unwrap();
                }
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
        let wall = started.elapsed();
        let clicks = tabs * 150;
        println!(
            "| {tabs} | {clicks} | {} | {:.0} |",
            micros(wall),
            clicks as f64 / wall.as_secs_f64()
        );
        for page in pages {
            page.close().await.unwrap();
        }
    }
    browser.close().await.unwrap();
    println!();

    // ----- memory ----------------------------------------------------------
    println!("## Memory of the driving process\n");
    println!("Resident memory of this process (not Chrome's) as browsers are added.\n");
    println!("| Chrome processes driven | resident memory |");
    println!("|---|---|");
    let baseline = rss_kib();
    println!("| 0 | {:.1} MiB |", baseline as f64 / 1024.0);
    let mut browsers = Vec::new();
    for count in 1..=8_usize {
        let browser = Browser::launch(options()).await.expect("Chrome starts");
        let page = browser.default_page().await.unwrap();
        page.goto(format!("{url}/")).await.unwrap();
        page.locator("#btn").click().await.unwrap();
        browsers.push(browser);
        if count == 1 || count == 4 || count == 8 {
            println!("| {count} | {:.1} MiB |", rss_kib() as f64 / 1024.0);
        }
    }
    for browser in browsers {
        browser.close().await.unwrap();
    }
}
