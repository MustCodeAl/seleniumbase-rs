//! Cost of the library's own work, with no browser involved.
//!
//! ```text
//! cargo bench --bench cpu
//! cargo bench --bench cpu --features turso     # adds the database figures
//! ```
//!
//! Each figure is the median of several batches, per operation.

use std::hint::black_box;
use std::time::{Duration, Instant};

use seleniumbase_rs::api::html_inspector::HtmlInspector;
use seleniumbase_rs::sb_cdp::Point;
use seleniumbase_rs::stealth::behavior::{Behavior, Humanizer};
use seleniumbase_rs::stealth::evasions::bootstrap_script;
use seleniumbase_rs::utils::selectors::Selector;
use seleniumbase_rs::{Fingerprint, OsType};

/// Times `op`, `iterations` calls per batch, and prints the median batch.
fn time<F: FnMut()>(label: &str, iterations: u32, mut op: F) {
    for _ in 0..iterations.min(50) {
        op();
    }
    let mut batches: Vec<Duration> = (0..9)
        .map(|_| {
            let started = Instant::now();
            for _ in 0..iterations {
                op();
            }
            started.elapsed()
        })
        .collect();
    batches.sort();
    let per = batches[4].as_secs_f64() / f64::from(iterations);
    let shown = if per >= 1e-3 {
        format!("{:.2} ms", per * 1e3)
    } else if per >= 1e-6 {
        format!("{:.2} µs", per * 1e6)
    } else {
        format!("{:.0} ns", per * 1e9)
    };
    println!("| {label} | {shown} | {:.0} |", 1.0 / per);
}

fn big_page(items: usize) -> String {
    let mut html = String::from("<!doctype html><html><head><title>t</title></head><body>");
    for i in 0..items {
        html.push_str(&format!(
            "<div class=\"row\" id=\"r{i}\"><a href=\"/p/{i}\">link {i}</a>\
             <img src=\"/i/{i}.png\" alt=\"image {i}\"><input id=\"in{i}\" name=\"n{i}\"></div>"
        ));
    }
    html.push_str("</body></html>");
    html
}

fn main() {
    println!("# CPU benchmark (no browser)\n");
    println!("| operation | time per call | calls / second |");
    println!("|---|---|---|");

    let mut seed = 0_u64;
    time("`Fingerprint::randomized` (Windows)", 2_000, || {
        seed = seed.wrapping_add(1);
        black_box(Fingerprint::randomized(OsType::Windows, seed));
    });
    let fingerprint = Fingerprint::randomized(OsType::Windows, 42);
    let script_bytes = bootstrap_script(&fingerprint).len();
    time(
        &format!(
            "build the stealth bootstrap script ({} KiB)",
            script_bytes / 1024
        ),
        200,
        || {
            black_box(bootstrap_script(black_box(&fingerprint)));
        },
    );
    time("`Fingerprint::validate`", 5_000, || {
        black_box(black_box(&fingerprint).validate());
    });

    let selectors = [
        "#login",
        ".btn.primary",
        "//div[@id='x']/a[2]",
        "(//li)[3]",
        "link=Sign in",
        "partial_link=Sign",
        "input[name='q']",
        "[data-test=\"go\"]",
    ];
    time("classify a CSS or XPath selector", 50_000, || {
        for s in &selectors {
            black_box(Selector::auto(black_box(s)));
        }
    });

    let mut person = Humanizer::new(Behavior::builder().seed(7).build().unwrap());
    time("plan a human mouse path (600 px)", 5_000, || {
        black_box(person.mouse_path(
            Point { x: 100.0, y: 100.0 },
            Point { x: 700.0, y: 400.0 },
            80.0,
        ));
    });
    time("plan human typing (100 characters)", 2_000, || {
        black_box(person.typing_plan(black_box(
            &"the quick brown fox jumps over the lazy dog. ".repeat(2),
        )));
    });

    let small = big_page(50);
    let large = big_page(2_000);
    time(
        &format!("inspect HTML ({} KiB)", small.len() / 1024),
        500,
        || {
            let _ = black_box(HtmlInspector::inspect(black_box(&small)));
        },
    );
    time(
        &format!("inspect HTML ({} KiB)", large.len() / 1024),
        20,
        || {
            let _ = black_box(HtmlInspector::inspect(black_box(&large)));
        },
    );

    #[cfg(feature = "turso")]
    database();
}

#[cfg(feature = "turso")]
fn database() {
    use seleniumbase_rs::core::report_helper::TestResult;
    use seleniumbase_rs::storage::{ProfileVault, ResultStore, RunInfo};

    let runtime = tokio::runtime::Runtime::new().unwrap();
    println!("\n## Embedded database (Turso)\n");
    println!("| operation | time per call | calls / second |");
    println!("|---|---|---|");

    runtime.block_on(async {
        let store = ResultStore::in_memory().await.unwrap();
        let run = store.start_run(&RunInfo::new("bench")).await.unwrap();
        let result = TestResult {
            name: "login_works".into(),
            passed: true,
            duration_secs: 1.25,
            message: "ok".into(),
        };
        let n = 2_000_u32;
        let started = Instant::now();
        for _ in 0..n {
            store.record(run, &result).await.unwrap();
        }
        let per = started.elapsed().as_secs_f64() / f64::from(n);
        println!(
            "| record a test result | {:.1} µs | {:.0} |",
            per * 1e6,
            1.0 / per
        );

        let started = Instant::now();
        let runs = 200_u32;
        for _ in 0..runs {
            black_box(store.flaky_tests(50).await.unwrap());
        }
        let per = started.elapsed().as_secs_f64() / f64::from(runs);
        println!(
            "| flaky-test query over {n} results | {:.1} µs | {:.0} |",
            per * 1e6,
            1.0 / per
        );

        // Opening a vault derives its key slowly on purpose.
        let started = Instant::now();
        let vault = ProfileVault::in_memory("benchmark passphrase")
            .await
            .unwrap();
        println!(
            "| open a vault (key derivation, on purpose) | {:.0} ms | - |",
            started.elapsed().as_secs_f64() * 1e3
        );
        let profile = serde_json::json!({ "name": "EU shop", "cookies": vec!["x"; 40] });
        let n = 2_000_u32;
        let started = Instant::now();
        for i in 0..n {
            vault
                .put("profiles", &format!("p{}", i % 50), &profile)
                .await
                .unwrap();
        }
        let per = started.elapsed().as_secs_f64() / f64::from(n);
        println!(
            "| seal and store a profile | {:.1} µs | {:.0} |",
            per * 1e6,
            1.0 / per
        );
        let started = Instant::now();
        for i in 0..n {
            black_box(
                vault
                    .get::<serde_json::Value>("profiles", &format!("p{}", i % 50))
                    .await
                    .unwrap(),
            );
        }
        let per = started.elapsed().as_secs_f64() / f64::from(n);
        println!(
            "| load and open a profile | {:.1} µs | {:.0} |",
            per * 1e6,
            1.0 / per
        );
    });
}
