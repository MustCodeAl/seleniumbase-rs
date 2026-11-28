//! Async test and CLI subprocess runner with live output streaming and cancellation.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Runtime};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use crate::discovery::TargetKind;

/// Options submitted by the UI for launching a Cargo or `sbrs` run.
#[derive(Debug, Clone, Deserialize)]
pub struct RunRequest {
    /// Workspace root directory.
    pub workspace: String,
    /// Kind of execution (`cargo_target` or `sbrs_cli`).
    pub mode: RunMode,
    /// Target name (`tests/<name>` or `examples/<name>`) for `cargo_target`.
    pub target: Option<String>,
    /// Target classification (`unit_suite`, `integration_test`, `example`).
    pub target_kind: Option<TargetKind>,
    /// Optional `-k` / test filter expression.
    pub filter: Option<String>,
    /// Optional Cargo features list (comma-separated).
    pub features: Option<String>,
    /// Include `#[ignore]`d tests (real-browser runs).
    pub include_ignored: bool,
    /// Browser name (`chrome`, `edge`, `firefox`, `safari`, `chromium`).
    pub browser: Option<String>,
    /// Run headless (`SB_HEADLESS=true`).
    pub headless: bool,
    /// Demo mode (`SB_DEMO=true`).
    pub demo: bool,
    /// Slow mode (`SB_SLOW=true`).
    pub slow: bool,
    /// Stealth UC mode (`SB_UC=true`).
    pub uc: bool,
    /// Reuse browser session (`SB_REUSE_SESSION=true`).
    pub reuse_session: bool,
    /// Incognito browser window (`SB_INCOGNITO=true`).
    pub incognito: bool,
    /// Ad-block mode (`SB_AD_BLOCK=true`).
    pub ad_block: bool,
    /// Custom arguments passed directly to `sbrs` when `mode == sbrs_cli`.
    pub cli_args: Option<Vec<String>>,
    /// Extra environment variables (`KEY=VALUE` pairs or map).
    pub extra_env: Option<BTreeMap<String, String>>,
}

/// How a command is launched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunMode {
    /// Invoke `cargo test` or `cargo run --example`.
    CargoTarget,
    /// Invoke `sbrs <args...>`.
    SbrsCli,
}

/// Line streamed to the frontend while a command executes.
#[derive(Debug, Clone, Serialize)]
pub struct RunnerLogEvent {
    /// Run identifier.
    pub run_id: u64,
    /// `"stdout"` or `"stderr"` or `"meta"`.
    pub stream: String,
    /// Single log line.
    pub line: String,
}

/// Completion event emitted when the subprocess finishes or is stopped.
#[derive(Debug, Clone, Serialize)]
pub struct RunnerFinishedEvent {
    /// Run identifier.
    pub run_id: u64,
    /// OS exit code if available.
    pub exit_code: Option<i32>,
    /// Whether the process exited cleanly with status 0.
    pub success: bool,
    /// Whether the run was stopped by the user.
    pub stopped: bool,
    /// Total elapsed wall time in milliseconds.
    pub duration_ms: u64,
}

/// Tracks the currently running child process so `Stop` terminates it reliably.
#[derive(Default)]
pub struct RunnerState {
    active: Mutex<Option<ActiveRun>>,
    next_id: Mutex<u64>,
}

struct ActiveRun {
    run_id: u64,
    child: Child,
    stopped: Arc<std::sync::atomic::AtomicBool>,
}

impl RunnerState {
    /// Stops any active child process. Returns `true` if a process was killed.
    pub async fn stop(&self) -> Result<bool, String> {
        let mut guard = self.active.lock().await;
        if let Some(mut run) = guard.take() {
            run.stopped.store(true, std::sync::atomic::Ordering::SeqCst);
            run.child
                .kill()
                .await
                .map_err(|e| format!("failed to kill process: {e}"))?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Starts a new test/CLI run and streams logs back to the Tauri window.
    pub async fn start<R: Runtime>(
        state: Arc<Self>,
        app: AppHandle<R>,
        req: RunRequest,
        sbrs_bin: Option<String>,
    ) -> Result<u64, String> {
        {
            let guard = state.active.lock().await;
            if guard.is_some() {
                return Err(
                    "a run is already in progress; stop it before starting another".to_owned(),
                );
            }
        }

        let workspace = PathBuf::from(&req.workspace);
        if !workspace.is_dir() {
            return Err(format!(
                "workspace folder does not exist: {}",
                workspace.display()
            ));
        }

        let run_id = {
            let mut id_guard = state.next_id.lock().await;
            *id_guard += 1;
            *id_guard
        };

        let (program, args) = build_command(&req, sbrs_bin)?;
        let mut cmd = Command::new(&program);
        cmd.current_dir(&workspace)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        apply_env(&mut cmd, &req);

        let _ = app.emit(
            "runner-log",
            RunnerLogEvent {
                run_id,
                stream: "meta".to_owned(),
                line: format!("$ {} {}", program, args.join(" ")),
            },
        );

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("failed to spawn `{program}`: {e}"))?;

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let stopped_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));

        {
            let mut guard = state.active.lock().await;
            *guard = Some(ActiveRun {
                run_id,
                child,
                stopped: stopped_flag.clone(),
            });
        }

        let app_out = app.clone();
        let out_task = tauri::async_runtime::spawn(async move {
            if let Some(pipe) = stdout {
                let mut lines = BufReader::new(pipe).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = app_out.emit(
                        "runner-log",
                        RunnerLogEvent {
                            run_id,
                            stream: "stdout".to_owned(),
                            line,
                        },
                    );
                }
            }
        });

        let app_err = app.clone();
        let err_task = tauri::async_runtime::spawn(async move {
            if let Some(pipe) = stderr {
                let mut lines = BufReader::new(pipe).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = app_err.emit(
                        "runner-log",
                        RunnerLogEvent {
                            run_id,
                            stream: "stderr".to_owned(),
                            line,
                        },
                    );
                }
            }
        });

        let state_bg = state.clone();
        tauri::async_runtime::spawn(async move {
            let started = Instant::now();
            let status = loop {
                let poll = {
                    let mut guard = state_bg.active.lock().await;
                    match guard.as_mut() {
                        Some(active) if active.run_id == run_id => match active.child.try_wait() {
                            Ok(Some(status)) => {
                                let _ = guard.take();
                                Some(Ok(status))
                            }
                            Ok(None) => None,
                            Err(err) => {
                                let _ = guard.take();
                                Some(Err(err))
                            }
                        },
                        _ => break None,
                    }
                };
                if let Some(res) = poll {
                    break res.ok();
                }
                tokio::time::sleep(std::time::Duration::from_millis(80)).await;
            };

            let _ = out_task.await;
            let _ = err_task.await;

            let stopped = stopped_flag.load(std::sync::atomic::Ordering::SeqCst);
            let exit_code = status.and_then(|s| s.code());
            let success = status.is_some_and(|s| s.success()) && !stopped;
            let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

            let _ = app.emit(
                "runner-finished",
                RunnerFinishedEvent {
                    run_id,
                    exit_code,
                    success,
                    stopped,
                    duration_ms,
                },
            );
        });

        Ok(run_id)
    }
}

fn build_command(
    req: &RunRequest,
    sbrs_bin: Option<String>,
) -> Result<(String, Vec<String>), String> {
    match req.mode {
        RunMode::CargoTarget => {
            let mut args = Vec::new();
            let kind = req.target_kind.unwrap_or(TargetKind::IntegrationTest);
            match kind {
                TargetKind::Example => {
                    let name = req
                        .target
                        .as_deref()
                        .filter(|s| !s.is_empty())
                        .ok_or_else(|| "missing example target name".to_owned())?;
                    args.extend(["run".to_owned(), "--example".to_owned(), name.to_owned()]);
                    if let Some(feat) = req
                        .features
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                    {
                        args.push("--features".to_owned());
                        args.push(feat.to_owned());
                    }
                }
                TargetKind::UnitSuite => {
                    args.extend(["test".to_owned(), "--lib".to_owned()]);
                    if let Some(feat) = req
                        .features
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                    {
                        args.push("--features".to_owned());
                        args.push(feat.to_owned());
                    }
                    if let Some(filter) = req
                        .filter
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                    {
                        args.push(filter.to_owned());
                    }
                }
                TargetKind::IntegrationTest => {
                    let name = req
                        .target
                        .as_deref()
                        .filter(|s| !s.is_empty())
                        .ok_or_else(|| "missing test target name".to_owned())?;
                    args.extend(["test".to_owned(), "--test".to_owned(), name.to_owned()]);
                    if let Some(feat) = req
                        .features
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                    {
                        args.push("--features".to_owned());
                        args.push(feat.to_owned());
                    }
                    if let Some(filter) = req
                        .filter
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                    {
                        args.push(filter.to_owned());
                    }
                    if req.include_ignored {
                        args.push("--".to_owned());
                        args.push("--ignored".to_owned());
                        args.push("--nocapture".to_owned());
                    }
                }
            }
            Ok(("cargo".to_owned(), args))
        }
        RunMode::SbrsCli => {
            let cli_args = req
                .cli_args
                .clone()
                .filter(|v| !v.is_empty())
                .ok_or_else(|| "missing CLI arguments for sbrs".to_owned())?;
            if let Some(bin) = sbrs_bin {
                Ok((bin, cli_args))
            } else {
                let mut args = vec![
                    "run".to_owned(),
                    "--bin".to_owned(),
                    "sbrs".to_owned(),
                    "--".to_owned(),
                ];
                args.extend(cli_args);
                Ok(("cargo".to_owned(), args))
            }
        }
    }
}

fn apply_env(cmd: &mut Command, req: &RunRequest) {
    if let Some(browser) = req
        .browser
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        cmd.env("SB_BROWSER", browser);
    }
    if req.headless {
        cmd.env("SB_HEADLESS", "true");
    }
    if req.demo {
        cmd.env("SB_DEMO", "true");
    }
    if req.slow {
        cmd.env("SB_SLOW", "true");
    }
    if req.uc {
        cmd.env("SB_UC", "true");
    }
    if req.reuse_session {
        cmd.env("SB_REUSE_SESSION", "true");
    }
    if req.incognito {
        cmd.env("SB_INCOGNITO", "true");
    }
    if req.ad_block {
        cmd.env("SB_AD_BLOCK", "true");
    }
    if let Some(extra) = &req.extra_env {
        for (k, v) in extra {
            if !k.trim().is_empty() {
                cmd.env(k.trim(), v);
            }
        }
    }
}
