//! Tauri application entrypoint and IPC commands for SeleniumBase Commander.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::Engine as _;
use serde::Serialize;
use tauri::{AppHandle, Runtime, State, command, generate_context, generate_handler};

mod discovery;
mod runner;

use discovery::{WorkspaceSnapshot, safe_workspace_path};
use runner::{RunRequest, RunnerState};

/// Resolves the best default workspace directory on launch.
#[command]
fn default_workspace() -> String {
    std::env::current_dir()
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| ".".to_owned())
}

/// Inspects a Rust / SeleniumBase project directory and returns all discovered
/// tests, examples, Behave features, case plans, and `latest_logs/` artifacts.
#[command]
async fn inspect_workspace(path: String) -> Result<WorkspaceSnapshot, String> {
    let workspace = PathBuf::from(&path);
    let bin = find_sbrs_binary(&workspace);
    discovery::inspect_workspace(&workspace, bin).map_err(|e| e.to_string())
}

/// Launches a Cargo test/example target or `sbrs` CLI command and streams output
/// back via `runner-log` and `runner-finished` events.
#[command]
async fn start_run<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<RunnerState>>,
    request: RunRequest,
) -> Result<u64, String> {
    let bin = find_sbrs_binary(Path::new(&request.workspace));
    RunnerState::start(state.inner().clone(), app, request, bin).await
}

/// Stops any active test or CLI process.
#[command]
async fn stop_run(state: State<'_, Arc<RunnerState>>) -> Result<bool, String> {
    state.stop().await
}

/// Reads or generates boilerplate content for a Case Plan markdown file.
#[command]
async fn read_case_plan(workspace: String, plan_path: String) -> Result<String, String> {
    let root = Path::new(&workspace);
    let full = safe_workspace_path(root, &plan_path).map_err(|e| e.to_string())?;
    if full.is_file() {
        fs::read_to_string(&full).map_err(|e| format!("failed to read {}: {e}", full.display()))
    } else {
        Ok(default_case_plan_template(&plan_path))
    }
}

/// Writes an edited Case Plan markdown file and updates `case_summary.md` in its folder.
#[command]
async fn write_case_plan(
    workspace: String,
    plan_path: String,
    content: String,
) -> Result<(), String> {
    let root = Path::new(&workspace);
    let full = safe_workspace_path(root, &plan_path).map_err(|e| e.to_string())?;
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    }
    fs::write(&full, content).map_err(|e| format!("failed to write {}: {e}", full.display()))?;
    Ok(())
}

/// Creates a new Gherkin `.feature` file inside `features/` without overwriting an existing one.
#[command]
async fn create_feature_file(
    workspace: String,
    name: String,
    title: String,
    scenarios: Vec<String>,
) -> Result<String, String> {
    let clean = name
        .trim()
        .trim_end_matches(".feature")
        .replace(['/', '\\', ' '], "_");
    if clean.is_empty() || clean.contains("..") {
        return Err("invalid feature file name".to_owned());
    }
    let rel = format!("features/{clean}.feature");
    let full = safe_workspace_path(Path::new(&workspace), &rel).map_err(|e| e.to_string())?;
    if full.exists() {
        return Err(format!("feature file already exists: {rel}"));
    }
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    }
    let feature_title = if title.trim().is_empty() {
        clean.replace('_', " ")
    } else {
        title.trim().to_owned()
    };
    let mut body = format!("Feature: {feature_title}\n");
    let items: Vec<String> = scenarios
        .into_iter()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect();
    if items.is_empty() {
        body.push_str(
            "\n  Scenario: Smoke check\n    Given I open the page\n    Then the page loads\n",
        );
    } else {
        for s in items {
            body.push_str(&format!(
                "\n  Scenario: {s}\n    Given I open the page\n    When I act on the page\n    Then I verify the result\n"
            ));
        }
    }
    fs::write(&full, body).map_err(|e| format!("failed to write {}: {e}", full.display()))?;
    Ok(rel)
}

/// Payload for viewing a failure artifact from `latest_logs/`.
#[derive(Debug, Serialize)]
pub struct LogArtifact {
    /// Text content (for `.txt` / `.html`), if text.
    pub text: Option<String>,
    /// Data URI (`data:image/png;base64,...`) when reading `screenshot.png`.
    pub image_data_url: Option<String>,
}

/// Reads an artifact inside `latest_logs/` within the current workspace.
#[command]
async fn read_log_artifact(
    workspace: String,
    artifact_path: String,
) -> Result<LogArtifact, String> {
    if !artifact_path.starts_with("latest_logs/") {
        return Err("artifact path must stay inside latest_logs/".to_owned());
    }
    let full =
        safe_workspace_path(Path::new(&workspace), &artifact_path).map_err(|e| e.to_string())?;
    if artifact_path.ends_with(".png") {
        let bytes =
            fs::read(&full).map_err(|e| format!("failed to read {}: {e}", full.display()))?;
        let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
        Ok(LogArtifact {
            text: None,
            image_data_url: Some(format!("data:image/png;base64,{encoded}")),
        })
    } else {
        let text = fs::read_to_string(&full)
            .map_err(|e| format!("failed to read {}: {e}", full.display()))?;
        Ok(LogArtifact {
            text: Some(text),
            image_data_url: None,
        })
    }
}

fn default_case_plan_template(plan_path: &str) -> String {
    let title = Path::new(plan_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("test_plan");
    format!(
        "### `{title}`\n\n\
         | Step | Action | Expected Result |\n\
         | :--- | :--- | :--- |\n\
         | 1 | Perform Action 1 | Verify Action 1 |\n"
    )
}

fn find_sbrs_binary(workspace: &Path) -> Option<String> {
    let exe_name = if cfg!(windows) { "sbrs.exe" } else { "sbrs" };
    for rel in [
        format!("target/debug/{exe_name}"),
        format!("target/release/{exe_name}"),
    ] {
        let candidate = workspace.join(rel);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(exe_name);
            if candidate.is_file() {
                return Some(candidate.to_string_lossy().into_owned());
            }
        }
    }
    None
}

/// Launches the SeleniumBase Commander Tauri desktop application.
pub fn run() {
    let state = Arc::new(RunnerState::default());

    tauri::Builder::default()
        .runtime(tauri_runtime_wry::Wry::default())
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(generate_handler![
            default_workspace,
            inspect_workspace,
            start_run,
            stop_run,
            read_case_plan,
            write_case_plan,
            create_feature_file,
            read_log_artifact,
        ])
        .run(generate_context!())
        .expect("error while running SeleniumBase Commander desktop GUI");
}
