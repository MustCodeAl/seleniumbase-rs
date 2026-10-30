//! `#[rstest]` and `#[fixture]` written with imports from `seleniumbase_rs` alone.
//!
//! The manifest of this crate does not list `rstest`; if the re-export did not carry
//! everything the macros expand to, this file would not compile.

use seleniumbase_rs::testing::{fixture, rstest};
use seleniumbase_rs::tokio;

#[fixture]
fn answer() -> u32 {
    42
}

#[rstest]
#[case(1, 43)]
#[case(2, 44)]
fn a_case_test_with_a_fixture(answer: u32, #[case] extra: u32, #[case] expected: u32) {
    assert_eq!(answer + extra, expected);
}

#[rstest]
#[tokio::test]
async fn an_async_test_with_a_fixture(answer: u32) {
    assert_eq!(answer, 42);
}
