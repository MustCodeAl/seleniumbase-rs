//! CPU micro-benchmarks for seleniumbase-rs core.
use seleniumbase_rs::sb_cdp::human::Behavior;
use std::hint::black_box;

fn main() {
    let behavior = black_box(Behavior::default());
    black_box(behavior);
}
