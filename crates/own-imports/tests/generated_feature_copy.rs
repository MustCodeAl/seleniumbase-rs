//! The copy of the Gherkin feature that the doctest of `src/lib.rs` needs is the golden
//! file of the library.
//!
//! The scenario in `tests/golden/cli/mkdir_bdd/bdd_test.rs.golden` names its feature file
//! from the root of the crate that compiles it, and the macro reads the file at compile
//! time. This crate compiles the golden for its doctest, so the feature sits under the same
//! path here; a change of the golden that this copy misses fails the test.

use std::fs;
use std::path::Path;

#[test]
fn the_feature_copy_is_the_golden_feature() {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let name = "tests/golden/cli/mkdir_bdd/features/example.feature";
    let copy = fs::read_to_string(here.join(name)).expect("the copy of the feature");
    let golden = fs::read_to_string(here.join("../..").join(name)).expect("the golden feature");
    assert_eq!(copy, golden, "copy {name} again from the library's tests");
}
