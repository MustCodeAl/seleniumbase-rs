//! Discovery of SeleniumBase-rs tests, examples, Behave features, case plans,
//! and failure evidence logs inside a workspace directory.

use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors returned by workspace discovery or artifact reading/writing.
#[derive(Debug, Error)]
pub enum DiscoveryError {
    /// Target workspace directory does not exist.
    #[error("workspace folder does not exist: {}", .0.display())]
    WorkspaceNotFound(PathBuf),
    /// Requested path is outside the workspace root or unsafe.
    #[error("unsafe path outside workspace: {0}")]
    UnsafePath(String),
    /// Underlying filesystem error.
    #[error("I/O error at {}: {source}", .path.display())]
    Io {
        /// Path that failed.
        path: PathBuf,
        /// Underlying OS error.
        #[source]
        source: std::io::Error,
    },
}

/// Kind of discovered Cargo target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    /// Integration test file in `tests/*.rs`.
    IntegrationTest,
    /// Runnable example in `examples/*.rs`.
    Example,
    /// Library unit test suite (`--lib`).
    UnitSuite,
}

/// Single test function found inside a Rust test file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredTestCase {
    /// Function name (e.g. `valid_login`).
    pub name: String,
    /// Line number in the source file (1-indexed).
    pub line: usize,
    /// Whether the test carries `#[ignore]`.
    pub ignored: bool,
}

/// Discovered Cargo test target or example file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredTarget {
    /// Target name passed to `--test` or `--example`.
    pub name: String,
    /// Target classification.
    pub kind: TargetKind,
    /// Relative POSIX path from the workspace root.
    pub path: String,
    /// Individual test functions detected in the file.
    pub cases: Vec<DiscoveredTestCase>,
}

/// One scenario inside a Gherkin `.feature` file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredScenario {
    /// Scenario or Scenario Outline title.
    pub name: String,
    /// Line number in the feature file (1-indexed).
    pub line: usize,
    /// Whether it is a `Scenario Outline:`.
    pub outline: bool,
    /// Combined feature + scenario tags (without `@`).
    pub tags: Vec<String>,
    /// Snake-case cargo test filter derived from the scenario name.
    pub filter: String,
}

/// Parsed `.feature` file under `features/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredFeature {
    /// Relative POSIX path.
    pub path: String,
    /// Feature title after `Feature:`.
    pub name: String,
    /// Feature-level tags without `@`.
    pub tags: Vec<String>,
    /// Scenarios in file order.
    pub scenarios: Vec<DiscoveredScenario>,
}

/// Discovered or generated case plan file (`case_plans/<id>.md`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CasePlanItem {
    /// Test identifier (`tests/foo.rs::bar`).
    pub id: String,
    /// Relative path to `case_plans/<name>.md`.
    pub plan_path: String,
    /// Whether the markdown file already exists on disk.
    pub exists: bool,
}

/// One failure log folder inside `latest_logs/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogFolderItem {
    /// Folder name (test run identifier).
    pub name: String,
    /// Relative POSIX path.
    pub path: String,
    /// Relative path to `screenshot.png` if present.
    pub screenshot_path: Option<String>,
    /// Relative path to `basic_test_info.txt` if present.
    pub info_path: Option<String>,
    /// Relative path to `page_source.html` if present.
    pub page_source_path: Option<String>,
}

/// Complete snapshot of a project workspace for the desktop UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceSnapshot {
    /// Canonical workspace path.
    pub root: String,
    /// Crate/project name parsed from `Cargo.toml` if present.
    pub package_name: Option<String>,
    /// Crate version parsed from `Cargo.toml` if present.
    pub package_version: Option<String>,
    /// Whether `sbrs` binary was found on PATH or inside `target/`.
    pub sbrs_binary: Option<String>,
    /// Discovered Cargo targets (`--lib`, `--test`, `--example`).
    pub targets: Vec<DiscoveredTarget>,
    /// Discovered `.feature` files under `features/`.
    pub features: Vec<DiscoveredFeature>,
    /// Discovered test case plans.
    pub case_plans: Vec<CasePlanItem>,
    /// Failure artifact folders inside `latest_logs/`.
    pub logs: Vec<LogFolderItem>,
}

/// Inspects `workspace_dir` and returns all discovered targets, features, case plans, and logs.
///
/// # Errors
///
/// Returns [`DiscoveryError::WorkspaceNotFound`] if the directory does not exist.
pub fn inspect_workspace(
    workspace_dir: &Path,
    sbrs_binary: Option<String>,
) -> Result<WorkspaceSnapshot, DiscoveryError> {
    if !workspace_dir.is_dir() {
        return Err(DiscoveryError::WorkspaceNotFound(
            workspace_dir.to_path_buf(),
        ));
    }
    let root = workspace_dir
        .canonicalize()
        .unwrap_or_else(|_| workspace_dir.to_path_buf());
    let (package_name, package_version) = parse_cargo_toml(&root.join("Cargo.toml"));

    let mut targets = Vec::new();
    if root.join("src").join("lib.rs").is_file() {
        targets.push(DiscoveredTarget {
            name: package_name.clone().unwrap_or_else(|| "lib".to_owned()),
            kind: TargetKind::UnitSuite,
            path: "src/lib.rs".to_owned(),
            cases: Vec::new(),
        });
    }
    discover_rs_targets(
        &root,
        &root.join("tests"),
        TargetKind::IntegrationTest,
        &mut targets,
    )?;
    discover_rs_targets(
        &root,
        &root.join("examples"),
        TargetKind::Example,
        &mut targets,
    )?;

    let mut features = Vec::new();
    discover_feature_files(&root, &root.join("features"), &mut features)?;

    let case_plans = build_case_plans(&root, &targets);
    let logs = discover_logs(&root)?;

    Ok(WorkspaceSnapshot {
        root: root.to_string_lossy().into_owned(),
        package_name,
        package_version,
        sbrs_binary,
        targets,
        features,
        case_plans,
        logs,
    })
}

/// Joins `relative` safely under `workspace_root`, rejecting traversal outside the root.
///
/// # Errors
///
/// Returns [`DiscoveryError::UnsafePath`] when the relative path tries to escape `workspace_root`.
pub fn safe_workspace_path(
    workspace_root: &Path,
    relative: &str,
) -> Result<PathBuf, DiscoveryError> {
    let rel = Path::new(relative);
    if rel.is_absolute() {
        return Err(DiscoveryError::UnsafePath(relative.to_owned()));
    }
    for comp in rel.components() {
        match comp {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(DiscoveryError::UnsafePath(relative.to_owned()));
            }
        }
    }
    Ok(workspace_root.join(rel))
}

fn parse_cargo_toml(path: &Path) -> (Option<String>, Option<String>) {
    let Ok(text) = fs::read_to_string(path) else {
        return (None, None);
    };
    let mut in_package = false;
    let mut name = None;
    let mut version = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("name")
            && let Some(val) = extract_toml_string(rest)
        {
            name = Some(val);
        } else if let Some(rest) = trimmed.strip_prefix("version")
            && let Some(val) = extract_toml_string(rest)
        {
            version = Some(val);
        }
    }
    (name, version)
}

fn extract_toml_string(after_key: &str) -> Option<String> {
    let trimmed = after_key.trim_start();
    let val = trimmed.strip_prefix('=')?.trim();
    let quoted = val.strip_prefix('"')?.split('"').next()?;
    if quoted.is_empty() {
        None
    } else {
        Some(quoted.to_owned())
    }
}

fn discover_rs_targets(
    root: &Path,
    dir: &Path,
    kind: TargetKind,
    out: &mut Vec<DiscoveredTarget>,
) -> Result<(), DiscoveryError> {
    if !dir.is_dir() {
        return Ok(());
    }
    let mut paths = Vec::new();
    let entries = fs::read_dir(dir).map_err(|source| DiscoveryError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| DiscoveryError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("rs") {
            paths.push(path);
        }
    }
    paths.sort();

    for path in paths {
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let cases = if kind == TargetKind::IntegrationTest {
            parse_test_functions(&path)
        } else {
            Vec::new()
        };
        out.push(DiscoveredTarget {
            name: stem.to_owned(),
            kind,
            path: rel,
            cases,
        });
    }
    Ok(())
}

fn parse_test_functions(path: &Path) -> Vec<DiscoveredTestCase> {
    let Ok(source) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut cases = Vec::new();
    let mut saw_test_attr = false;
    let mut saw_ignore_attr = false;
    for (idx, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("#[") {
            if trimmed.contains("test") || trimmed.contains("rstest") {
                saw_test_attr = true;
            }
            if trimmed.starts_with("#[ignore") {
                saw_ignore_attr = true;
            }
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        if saw_test_attr {
            if let Some(fn_name) = parse_fn_name(trimmed) {
                cases.push(DiscoveredTestCase {
                    name: fn_name,
                    line: idx + 1,
                    ignored: saw_ignore_attr,
                });
            }
            saw_test_attr = false;
            saw_ignore_attr = false;
        }
    }
    cases
}

fn parse_fn_name(line: &str) -> Option<String> {
    let rest = line.strip_prefix("pub ").unwrap_or(line).trim_start();
    let rest = rest.strip_prefix("async ").unwrap_or(rest).trim_start();
    let rest = rest.strip_prefix("fn ")?.trim_start();
    let name: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() { None } else { Some(name) }
}

fn discover_feature_files(
    root: &Path,
    dir: &Path,
    out: &mut Vec<DiscoveredFeature>,
) -> Result<(), DiscoveryError> {
    if !dir.is_dir() {
        return Ok(());
    }
    let mut files = Vec::new();
    collect_feature_paths(dir, &mut files)?;
    files.sort();
    for path in files {
        if let Some(feature) = parse_feature_file(root, &path) {
            out.push(feature);
        }
    }
    Ok(())
}

fn collect_feature_paths(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), DiscoveryError> {
    let entries = fs::read_dir(dir).map_err(|source| DiscoveryError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| DiscoveryError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if path.is_dir() {
            if !name_str.starts_with('.') && name_str != "target" && name_str != "node_modules" {
                collect_feature_paths(&path, out)?;
            }
        } else if path.extension().and_then(|s| s.to_str()) == Some("feature") {
            out.push(path);
        }
    }
    Ok(())
}

fn parse_feature_file(root: &Path, path: &Path) -> Option<DiscoveredFeature> {
    let text = fs::read_to_string(path).ok()?;
    let rel = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    let mut feature_name = String::new();
    let mut feature_tags = Vec::new();
    let mut pending_tags = Vec::new();
    let mut scenarios = Vec::new();

    for (idx, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with('@') {
            for tok in trimmed.split_whitespace() {
                if let Some(tag) = tok.strip_prefix('@')
                    && !tag.is_empty()
                {
                    pending_tags.push(tag.to_owned());
                }
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("Feature:") {
            feature_name = rest.trim().to_owned();
            feature_tags = std::mem::take(&mut pending_tags);
        } else if let Some(rest) = trimmed
            .strip_prefix("Scenario Outline:")
            .or_else(|| trimmed.strip_prefix("Scenario Template:"))
        {
            let name = rest.trim().to_owned();
            let mut tags = feature_tags.clone();
            tags.extend(std::mem::take(&mut pending_tags));
            scenarios.push(DiscoveredScenario {
                filter: scenario_filter(&name),
                name,
                line: idx + 1,
                outline: true,
                tags,
            });
        } else if let Some(rest) = trimmed.strip_prefix("Scenario:") {
            let name = rest.trim().to_owned();
            let mut tags = feature_tags.clone();
            tags.extend(std::mem::take(&mut pending_tags));
            scenarios.push(DiscoveredScenario {
                filter: scenario_filter(&name),
                name,
                line: idx + 1,
                outline: false,
                tags,
            });
        } else {
            pending_tags.clear();
        }
    }

    Some(DiscoveredFeature {
        path: rel,
        name: feature_name,
        tags: feature_tags,
        scenarios,
    })
}

/// Converts a scenario title to its snake_case `cargo test` filter name.
#[must_use]
pub fn scenario_filter(title: &str) -> String {
    let mut out = String::new();
    let mut prev_underscore = true;
    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_underscore = false;
        } else if !prev_underscore {
            out.push('_');
            prev_underscore = true;
        }
    }
    out.trim_end_matches('_').to_owned()
}

fn build_case_plans(root: &Path, targets: &[DiscoveredTarget]) -> Vec<CasePlanItem> {
    let mut plans = Vec::new();
    for target in targets {
        if target.kind != TargetKind::IntegrationTest {
            continue;
        }
        if target.cases.is_empty() {
            let id = target.path.clone();
            let plan_rel = case_plan_rel_path(&target.path, &target.name);
            plans.push(CasePlanItem {
                id,
                exists: root.join(&plan_rel).is_file(),
                plan_path: plan_rel,
            });
        } else {
            for case in &target.cases {
                let id = format!("{}::{}", target.path, case.name);
                let stem = format!("{}.{}", target.name, case.name);
                let plan_rel = case_plan_rel_path(&target.path, &stem);
                plans.push(CasePlanItem {
                    id,
                    exists: root.join(&plan_rel).is_file(),
                    plan_path: plan_rel,
                });
            }
        }
    }
    plans
}

fn case_plan_rel_path(source_rel: &str, plan_stem: &str) -> String {
    let parent = Path::new(source_rel)
        .parent()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    if parent.is_empty() {
        format!("case_plans/{plan_stem}.md")
    } else {
        format!("{parent}/case_plans/{plan_stem}.md")
    }
}

fn discover_logs(root: &Path) -> Result<Vec<LogFolderItem>, DiscoveryError> {
    let dir = root.join("latest_logs");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut items = Vec::new();
    let entries = fs::read_dir(&dir).map_err(|source| DiscoveryError::Io {
        path: dir.clone(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| DiscoveryError::Io {
            path: dir.clone(),
            source,
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let rel = format!("latest_logs/{name}");
        let screenshot = path.join("screenshot.png");
        let info = path.join("basic_test_info.txt");
        let page_source = path.join("page_source.html");
        items.push(LogFolderItem {
            name,
            screenshot_path: screenshot
                .is_file()
                .then(|| format!("{rel}/screenshot.png")),
            info_path: info.is_file().then(|| format!("{rel}/basic_test_info.txt")),
            page_source_path: page_source
                .is_file()
                .then(|| format!("{rel}/page_source.html")),
            path: rel,
        });
    }
    items.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenario_filter_matches_snake_case_rule() {
        assert_eq!(scenario_filter("Valid Login Flow!"), "valid_login_flow");
        assert_eq!(scenario_filter("  User   123 "), "user_123");
    }

    #[test]
    fn safe_workspace_path_rejects_traversal_and_absolute_paths() {
        let root = Path::new("/tmp/workspace");
        assert!(safe_workspace_path(root, "tests/case_plans/a.md").is_ok());
        assert!(safe_workspace_path(root, "../etc/passwd").is_err());
        assert!(safe_workspace_path(root, "/etc/passwd").is_err());
    }
}
