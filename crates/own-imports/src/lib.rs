//! No code: the tests of this crate are the point. See `Cargo.toml`.
//!
//! The doctest below compiles the files that `sbrs mkdir`, `sbrs mkfile` and the recorder
//! write, in a crate whose manifest lists `seleniumbase-rs` and nothing else a test names.
//! They are the golden files of the library (`tests/golden_rust/`, `tests/golden/cli/`), so
//! a generator that wrote a `use` of another crate would fail here. The block is `no_run`:
//! the tests would start a browser.

// The Gherkin golden names its feature file from the root of the crate that compiles it, so
// `tests/golden/cli/mkdir_bdd/features/example.feature` is copied here
// (`tests/generated_feature_copy.rs` keeps the copy equal to the golden).
#[cfg(doctest)]
#[doc = concat!(
    "```no_run,test_harness\n",
    "mod recorder_rstest {\n",
    include_str!("../../../tests/golden_rust/rstest_recording.rs"),
    "}\n",
    "mod recorder_cdp_script {\n",
    include_str!("../../../tests/golden_rust/cdp_script.rs"),
    "}\n",
    "mod mkdir_my_first_test {\n",
    include_str!("../../../tests/golden/cli/mkdir/my_first_test.rs.golden"),
    "}\n",
    "mod mkdir_parameterized_test {\n",
    include_str!("../../../tests/golden/cli/mkdir/parameterized_test.rs.golden"),
    "}\n",
    "mod mkdir_cdp_test {\n",
    include_str!("../../../tests/golden/cli/mkdir/cdp_test.rs.golden"),
    "}\n",
    "mod mkdir_bdd_test {\n",
    include_str!("../../../tests/golden/cli/mkdir_bdd/bdd_test.rs.golden"),
    "}\n",
    "mod mkfile_pf {\n",
    include_str!("../../../tests/golden/cli/mkfile/pf.rs.golden"),
    "}\n",
    "mod mkfile_pf_rec {\n",
    include_str!("../../../tests/golden/cli/mkfile/pf_rec.rs.golden"),
    "}\n",
    "mod mkfile_bc {\n",
    include_str!("../../../tests/golden/cli/mkfile/bc.rs.golden"),
    "}\n",
    "mod mkfile_cm {\n",
    include_str!("../../../tests/golden/cli/mkfile/cm.rs.golden"),
    "}\n",
    "mod mkfile_cm_uc {\n",
    include_str!("../../../tests/golden/cli/mkfile/cm_uc.rs.golden"),
    "}\n",
    "mod mkfile_dc {\n",
    include_str!("../../../tests/golden/cli/mkfile/dc.rs.golden"),
    "}\n",
    "mod mkfile_dm {\n",
    include_str!("../../../tests/golden/cli/mkfile/dm.rs.golden"),
    "}\n",
    "fn main() {}\n```"
)]
struct GeneratedCodeCompiles;
