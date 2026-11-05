//! Where the allocations of the `alloc` workloads come from, found with dhat.
//!
//! ```text
//! CARGO_PROFILE_BENCH_DEBUG=line-tables-only \
//!     cargo bench --bench alloc_sites --features "mcp-server test-util"
//! ```
//!
//! Each workload is set up first, then run under a heap profiler a fixed number of times. The
//! profile is written to `target/tmp/dhat/<workload>.json`, and the largest allocation sites in
//! this crate's code are printed as Markdown tables. Open a JSON file in
//! <https://nnethercote.github.io/dh_view/dh_view.html> for the whole tree. Line tables (the
//! variable above) give each site its file and line; without them the table names functions.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

mod alloc_workloads;

use alloc_workloads as work;

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

/// How many times each workload runs under the profiler.
const RUNS: u64 = 200;

/// How many sites each table lists.
const TOP: usize = 8;

/// The frames of a backtrace dhat keeps; deep enough to get past the allocator and the
/// standard library into this crate's code.
const FRAMES: usize = 40;

/// One allocation site: where in this crate the allocations start, and their totals.
#[derive(Debug, Default)]
struct Site {
    bytes: u64,
    blocks: u64,
}

/// The part of a dhat frame after its address: `function (file:line:column)`.
fn without_address(frame: &str) -> &str {
    frame.split_once(": ").map_or(frame, |(_, rest)| rest)
}

/// The first frame of `frames` in this crate's code, as `file:line function`, else the first
/// frame of another crate. Symbol names alone mislead: the linker folds identical generic
/// functions, so a `Vec` growth may carry the name of an unrelated element type.
fn place(frames: &[&str], files: &[String]) -> String {
    for frame in frames.iter().map(|frame| without_address(frame)) {
        let Some(path) = location(frame) else {
            continue;
        };
        let Some(own) = own_file(path, files) else {
            continue;
        };
        let line = path.split(':').nth(1).unwrap_or("?");
        let name = frame.split(" (").next().unwrap_or_default();
        let name = name.split('<').next().unwrap_or_default();
        return if name.is_empty() || name.starts_with('{') {
            format!("src/{own}:{line}")
        } else {
            format!("src/{own}:{line} {name}")
        };
    }
    let outside = frames
        .iter()
        .map(|frame| without_address(frame))
        .find(|frame| {
            location(frame).is_some_and(|path| {
                !["alloc/", "core/", "std/", "src/", "dhat-", "???"]
                    .iter()
                    .any(|prefix| path.starts_with(prefix))
            })
        });
    format!("(outside this crate) {}", outside.unwrap_or("unknown"))
}

/// The `file:line:column` of a dhat frame, the part in brackets at its end.
fn location(frame: &str) -> Option<&str> {
    frame
        .rsplit_once(" (")
        .map(|(_, rest)| rest.trim_end_matches(')'))
}

/// Every `.rs` file under this crate's `src/`, as a path relative to it.
fn own_files() -> Vec<String> {
    fn walk(folder: &Path, prefix: &str, files: &mut Vec<String>) {
        for entry in std::fs::read_dir(folder).into_iter().flatten().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            if path.is_dir() {
                walk(&path, &format!("{prefix}{name}/"), files);
            } else if name.ends_with(".rs") {
                files.push(format!("{prefix}{name}"));
            }
        }
    }
    let mut files = Vec::new();
    walk(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        "",
        &mut files,
    );
    files
}

/// This crate's file that a frame's path names, as a path under `src/`. dhat writes some of
/// them from `src/` and others from deeper down (`importer/pyconv/parse.rs`), and the standard
/// library's as `src/vec/mod.rs`, so a path is matched against the crate's own files.
fn own_file<'a>(path: &str, files: &'a [String]) -> Option<&'a str> {
    let file = path.split(':').next().unwrap_or(path);
    let found = match file.strip_prefix("src/") {
        Some(under_src) => files.iter().find(|own| own.as_str() == under_src),
        None => files
            .iter()
            .find(|own| own.as_str() == file || own.ends_with(&format!("/{file}"))),
    };
    found.map(String::as_str)
}

/// The sites of a dhat heap profile, largest first, merged by place.
fn sites(profile: &Value, files: &[String]) -> Vec<(String, Site)> {
    let table: Vec<&str> = profile["ftbl"]
        .as_array()
        .map(|frames| frames.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let mut merged: HashMap<String, Site> = HashMap::new();
    for point in profile["pps"].as_array().into_iter().flatten() {
        let frames: Vec<&str> = point["fs"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_u64)
            .filter_map(|index| usize::try_from(index).ok())
            .filter_map(|index| table.get(index).copied())
            .collect();
        let site = merged.entry(place(&frames, files)).or_default();
        site.bytes += point["tb"].as_u64().unwrap_or(0);
        site.blocks += point["tbk"].as_u64().unwrap_or(0);
    }
    let mut sites: Vec<(String, Site)> = merged.into_iter().collect();
    sites.sort_by(|a, b| b.1.bytes.cmp(&a.1.bytes).then_with(|| a.0.cmp(&b.0)));
    sites
}

/// Runs `work` `RUNS` times under a heap profiler and prints its totals and largest sites.
fn profile(name: &str, folder: &Path, files: &[String], mut work: impl FnMut()) {
    let file = folder.join(format!("{name}.json"));
    let profiler = dhat::Profiler::builder()
        .file_name(&file)
        .trim_backtraces(Some(FRAMES))
        .build();
    for _ in 0..RUNS {
        work();
    }
    let stats = dhat::HeapStats::get();
    drop(profiler);

    println!("## {name}\n");
    println!(
        "{RUNS} runs: {} allocations, {} bytes; per run {} allocations, {} bytes. Profile: `{}`\n",
        stats.total_blocks,
        stats.total_bytes,
        stats.total_blocks / RUNS,
        stats.total_bytes / RUNS,
        file.display()
    );
    let profile: Value = std::fs::read_to_string(&file)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| json!({}));
    println!("| site | bytes per run | allocations per run |");
    println!("|---|---|---|");
    for (place, site) in sites(&profile, files).into_iter().take(TOP) {
        println!(
            "| `{}` | {} | {} |",
            place.replace('|', "\\|"),
            site.bytes / RUNS,
            site.blocks / RUNS
        );
    }
    println!();
}

fn main() {
    let folder: PathBuf = Path::new(env!("CARGO_TARGET_TMPDIR")).join("dhat");
    std::fs::create_dir_all(&folder).expect("the profile folder");
    let runtime = work::runtime();
    let files = own_files();
    println!("# Allocation sites (dhat)\n");

    let (page, _mock) = runtime.block_on(work::mocked_page());
    profile("cdp_evaluate", &folder, &files, || {
        runtime.block_on(work::evaluate(&page));
    });
    profile("cdp_locator_count", &folder, &files, || {
        runtime.block_on(work::count(&page));
    });
    profile("cdp_locator_click", &folder, &files, || {
        runtime.block_on(work::click(&page));
    });

    let sources = work::golden_sources();
    profile("importer_all_golden_files", &folder, &files, || {
        std::hint::black_box(work::import_all(&sources));
    });

    let host = work::dev_host();
    let arguments = work::arguments(json!({ "query": "click" }));
    profile("mcp_dev_find_api", &folder, &files, || {
        runtime
            .block_on(host.call("find_api", arguments.clone()))
            .expect("find_api answers");
    });
    let (cdp_host, _cdp_mock) = runtime.block_on(work::cdp_host());
    profile("mcp_cdp_tools_list", &folder, &files, || {
        std::hint::black_box(cdp_host.describe_tools());
    });
}
