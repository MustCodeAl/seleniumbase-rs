//! `#[tokio::test]` and `tokio::time` written with imports from `seleniumbase_rs` alone.
//!
//! The manifest of this crate does not list `tokio`: the attribute macros expand to paths
//! that start with `tokio::`, and the import below makes them resolve.

use std::time::Duration;

use seleniumbase_rs::tokio;

/// What `#[tokio::main]` does to an `async fn main`, on a function a test can call.
#[tokio::main]
async fn run_to_completion() -> u32 {
    tokio::time::sleep(Duration::from_millis(1)).await;
    7
}

#[test]
fn tokio_main_builds_a_runtime_from_the_import() {
    assert_eq!(run_to_completion(), 7);
}

#[tokio::test]
async fn a_test_on_the_runtime_of_the_library() {
    tokio::time::sleep(Duration::from_millis(5)).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_multi_thread_test() {
    let task = tokio::spawn(async { 21 * 2 });
    assert_eq!(task.await.expect("the task ends"), 42);
}
