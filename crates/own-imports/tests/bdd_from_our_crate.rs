//! A Gherkin scenario written with imports from `seleniumbase_rs` alone.
//!
//! The manifest of this crate lists neither `rstest-bdd` nor `rstest-bdd-macros` nor `rstest`.
//! If the macro re-exports did not carry everything the macros expand to, this file would
//! not compile. The steps use a plain counter, not a browser, so the test runs anywhere.

use seleniumbase_rs::testing::bdd::{given, scenario, skip, then, when};
use seleniumbase_rs::testing::{fixture, rstest};

/// What the steps of a scenario share.
#[derive(Default)]
struct Counter {
    value: u32,
}

#[fixture]
fn counter() -> Counter {
    Counter::default()
}

#[given("the counter holds {n:u32}")]
fn the_counter_holds_to_begin_with(counter: &mut Counter, n: u32) {
    counter.value = n;
}

#[when("I add {n:u32}")]
fn i_add(counter: &mut Counter, n: u32) {
    counter.value += n;
}

#[when("I add {n:u32} twice")]
fn i_add_twice(counter: &mut Counter, n: u32) {
    counter.value += 2 * n;
}

#[then("the counter holds {n:u32}")]
fn the_counter_holds(counter: &Counter, n: u32) {
    assert_eq!(counter.value, n);
}

/// A step that ends its scenario early with `skip!`, which the module re-exports too. No
/// scenario of this file uses it; the file only has to compile.
#[then("the counter is left unchecked")]
fn the_counter_is_left_unchecked() {
    skip!("on purpose");
}

#[scenario(
    path = "tests/features/counter.feature",
    name = "Adding to the counter"
)]
fn adding_to_the_counter(counter: Counter) {
    let _ = counter;
}

#[scenario(path = "tests/features/counter.feature", name = "Adding twice")]
fn adding_twice(counter: Counter) {
    let _ = counter;
}
