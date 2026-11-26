//! CPU micro-benchmarks for seleniumbase-rs core.
use std::hint::black_box;
use seleniumbase_rs::sb_cdp::human::Behavior;

fn main() {
    let behavior = black_box(Behavior::default());
    black_box(behavior);
}
