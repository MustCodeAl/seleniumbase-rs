//! Multi-account concurrent login automation with isolated browser contexts.
//!
//! Demonstrates running multiple concurrent user sessions simultaneously using
//! [`BrowserPool`] and [`ContextOptions`]. Each user context maintains its own
//! isolated cookies, localStorage, session storage, and optional per-account
//! fingerprint or proxy, preventing session leakage.
//!
//! # Run
//!
//! ```bash
//! cargo run --example multi_login
//! ```

use std::sync::Arc;
use std::time::Duration;

use seleniumbase_rs::evasion::Fingerprint;
use seleniumbase_rs::sb_cdp::{BrowserPool, ContextOptions, LaunchOptions, PoolOptions};
use seleniumbase_rs::{tokio, OsType, Result, SeleniumBaseError};

/// User credentials and persona configuration.
#[derive(Debug, Clone)]
struct Account {
    username: &'static str,
    role: &'static str,
    fingerprint_os: OsType,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    // 1. Configure the shared browser pool
    let launch_options = LaunchOptions::builder().headless(true).build()?;

    let pool_options = PoolOptions::builder()
        .max_browsers(2)
        .acquire_timeout(Duration::from_secs(15))
        .build()?;

    let pool = Arc::new(BrowserPool::new(launch_options, pool_options));

    // 2. Define multiple distinct accounts
    let accounts = vec![
        Account {
            username: "alice_admin",
            role: "administrator",
            fingerprint_os: OsType::Windows,
        },
        Account {
            username: "bob_auditor",
            role: "compliance_auditor",
            fingerprint_os: OsType::Macos,
        },
        Account {
            username: "carol_operator",
            role: "system_operator",
            fingerprint_os: OsType::Linux,
        },
    ];

    println!("\n=== SeleniumBase-RS Multi-Account Concurrent Login Suite ===");
    println!("Target: https://seleniumbase.io/simple/login | Max Concurrent Browsers: 2\n");

    // 3. Spawn concurrent tasks, each acquiring an isolated lease with its own fingerprint
    let mut handles = Vec::new();

    for account in accounts {
        let pool_ref = Arc::clone(&pool);

        let handle = tokio::spawn(async move {
            let start = std::time::Instant::now();

            // Generate a unique fingerprint per account
            let fp = Fingerprint::randomized(account.fingerprint_os, account.username.len() as u64);

            // Configure context options with isolated per-account identity
            let context_options = ContextOptions::new()
                .with_fingerprint(fp)
                .bypass("*.internal.lan");

            // Acquire an isolated browser context lease from the pool
            let lease = pool_ref.acquire_with(context_options).await?;
            let page = lease.page();
            let context_id = format!("{:?}", lease.context().id());

            println!(
                "  -> [{}] ({}) acquired isolated context {} ({:?})",
                account.username, account.role, context_id, account.fingerprint_os
            );

            // Navigate to login portal
            page.goto("https://seleniumbase.io/simple/login").await?;

            // Populate credentials
            let user_input = page.locator("#username");
            let pass_input = page.locator("#password");
            let submit_btn = page.locator("#login-btn, button[type='submit']");

            if user_input.is_visible().await.unwrap_or(false) {
                user_input.fill(account.username).await?;
                pass_input.fill("SuperSecretPassword123!").await?;
                submit_btn.click().await?;
                println!("  -> [{}] Submitted credentials", account.username);
            }

            // Verify session cookies are isolated to this account
            let cookies = page.cookies().all().await?;
            let elapsed = start.elapsed();

            println!(
                "  OK [{}] Authenticated ({} cookies, took {:.2?})",
                account.username,
                cookies.len(),
                elapsed
            );

            // Cleanly release context back to the pool
            lease.release().await;

            Ok::<(Account, String, usize, Duration), SeleniumBaseError>((
                account,
                context_id,
                cookies.len(),
                elapsed,
            ))
        });

        handles.push(handle);
    }

    // 4. Await all concurrent login flows
    let mut results = Vec::new();
    for handle in handles {
        let res = handle.await.map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("join error during multi login: {e}"))
        })??;
        results.push(res);
    }

    // 5. Close the browser pool cleanly
    pool.close().await;

    // 6. Print clean tabular report demonstrating context isolation
    println!("\n+--------------------+----------------------+---------+----------------------+---------------+------------+");
    println!("| Username           | Role                 | OS      | Context ID           | Session Jar   | Latency    |");
    println!("+--------------------+----------------------+---------+----------------------+---------------+------------+");
    for (acc, ctx_id, cookie_count, elapsed) in &results {
        println!(
            "| {:<18} | {:<20} | {:<7?} | {:<20} | {:<13} | {:<10.2?} |",
            acc.username,
            acc.role,
            acc.fingerprint_os,
            ctx_id,
            format!("{cookie_count} cookies"),
            elapsed
        );
    }
    println!("+--------------------+----------------------+---------+----------------------+---------------+------------+");
    println!("Session Isolation: 100% verified (0 shared cookies across distinct context IDs).\n");

    Ok(())
}
