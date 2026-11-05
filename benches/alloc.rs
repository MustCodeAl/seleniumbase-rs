//! Allocations per call, counted by divan's `AllocProfiler`: the pure CDP engine on its mocked
//! browser, the Python importer on the golden files, and MCP tool calls handled in process.
//!
//! ```text
//! cargo bench --bench alloc --features "mcp-server test-util"
//! ```
//!
//! No Chrome and no network. Each table row gives the time and, under it, the allocations and
//! bytes of one call (`alloc`, `dealloc`, `grow`). `alloc_sites` says where they come from.

use std::hint::black_box;

use divan::{AllocProfiler, Bencher};
use serde_json::json;

mod alloc_workloads;

use alloc_workloads as work;

#[global_allocator]
static ALLOC: AllocProfiler = AllocProfiler::system();

fn main() {
    divan::main();
}

/// The pure CDP engine, one command at a time, on the mocked browser.
mod cdp {
    use super::{work, Bencher};

    #[divan::bench]
    fn evaluate(bencher: Bencher) {
        let runtime = work::runtime();
        let (page, _mock) = runtime.block_on(work::mocked_page());
        bencher.bench_local(|| runtime.block_on(work::evaluate(&page)));
    }

    #[divan::bench]
    fn locator_count(bencher: Bencher) {
        let runtime = work::runtime();
        let (page, _mock) = runtime.block_on(work::mocked_page());
        bencher.bench_local(|| runtime.block_on(work::count(&page)));
    }

    #[divan::bench]
    fn locator_click(bencher: Bencher) {
        let runtime = work::runtime();
        let (page, _mock) = runtime.block_on(work::mocked_page());
        bencher.bench_local(|| runtime.block_on(work::click(&page)));
    }
}

/// `import_python` on every Python file of `tests/golden/import_python`.
mod importer {
    use super::{black_box, work, Bencher};

    #[divan::bench]
    fn all_golden_files(bencher: Bencher) {
        let sources = work::golden_sources();
        bencher.bench_local(|| work::import_all(black_box(&sources)));
    }
}

/// MCP requests handled in process, as `Host::call` and `tools/list` answer them.
mod mcp {
    use super::{json, work, Bencher};

    #[divan::bench]
    fn dev_find_api(bencher: Bencher) {
        let runtime = work::runtime();
        let host = work::dev_host();
        let arguments = work::arguments(json!({ "query": "click" }));
        bencher
            .with_inputs(|| arguments.clone())
            .bench_local_values(|arguments| {
                runtime
                    .block_on(host.call("find_api", arguments))
                    .expect("find_api answers")
            });
    }

    #[divan::bench]
    fn cdp_tools_list(bencher: Bencher) {
        let runtime = work::runtime();
        let (host, _mock) = runtime.block_on(work::cdp_host());
        bencher.bench_local(|| host.describe_tools());
    }
}
