//! The Dockerfile's dependency-cache step must create every source file that
//! `Cargo.toml` names.
//!
//! The `Dockerfile` copies only the manifests first and builds a skeleton of
//! empty sources so Docker can cache the dependency build. Cargo refuses to read
//! a manifest whose `[lib]`, `[[bin]]`, `[[example]]`, `[[bench]]` or `[[test]]`
//! file is missing, even when the build asks for one binary. A target added to
//! `Cargo.toml` and not to that step therefore breaks the image build, but only
//! when somebody builds the image. This test finds it first.
//!
//! The step is read as plain text: the first `RUN` after `COPY Cargo.toml`, split
//! at `&&`, where `mkdir` makes folders, `touch` makes files and `echo ... > file`
//! writes one. Redirects must be written `> file` with a space; `>file` is not
//! understood, which fails the test rather than hiding a gap.

use std::collections::BTreeSet;
use std::path::Path;

const MANIFEST: &str = include_str!("../Cargo.toml");
const DOCKERFILE: &str = include_str!("../Dockerfile");

/// The sections of `Cargo.toml` that name a source file, and so need it to
/// exist.
const TARGET_SECTIONS: [&str; 4] = ["bin", "example", "bench", "test"];

/// A source file Cargo insists on finding before it reads the manifest.
#[derive(Debug)]
struct Needed {
    /// What asked for it, such as "bin `sbase`".
    wanted_by: String,
    file: String,
}

/// What the Dockerfile's skeleton step creates.
#[derive(Debug, Default)]
struct Skeleton {
    folders: BTreeSet<String>,
    files: BTreeSet<String>,
}

fn normalize(path: &str) -> String {
    path.trim_start_matches("./")
        .trim_end_matches('/')
        .to_owned()
}

/// Every file `manifest` makes Cargo look for: the `[lib]` and each `[[bin]]`,
/// `[[example]]`, `[[bench]]` and `[[test]]`, at their `path` or the place
/// Cargo defaults to.
fn needed_files(manifest: &str) -> Vec<Needed> {
    let manifest: toml::Table = manifest.parse().expect("Cargo.toml parses");
    let package = manifest
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(toml::Value::as_str)
        .expect("the package has a name");
    let mut needed = Vec::new();

    if let Some(lib) = manifest.get("lib") {
        let path = lib
            .get("path")
            .and_then(toml::Value::as_str)
            .unwrap_or("src/lib.rs");
        needed.push(Needed {
            wanted_by: "lib".to_owned(),
            file: normalize(path),
        });
    }
    for section in TARGET_SECTIONS {
        let Some(targets) = manifest.get(section).and_then(toml::Value::as_array) else {
            continue;
        };
        for target in targets {
            let name = target
                .get("name")
                .and_then(toml::Value::as_str)
                .expect("a target has a name");
            let default = match section {
                "bin" if name == package => "src/main.rs".to_owned(),
                "bin" => format!("src/bin/{name}.rs"),
                "example" => format!("examples/{name}.rs"),
                "bench" => format!("benches/{name}.rs"),
                _ => format!("tests/{name}.rs"),
            };
            let file = target
                .get("path")
                .and_then(toml::Value::as_str)
                .map_or(default, str::to_owned);
            needed.push(Needed {
                wanted_by: format!("{section} `{name}`"),
                file: normalize(&file),
            });
        }
    }
    needed
}

/// The instructions of a Dockerfile, each on one line: continuation lines are
/// joined and comment lines, which Docker also allows inside one, are dropped.
fn instructions(dockerfile: &str) -> Vec<String> {
    let mut all = Vec::new();
    let mut current = String::new();
    for line in dockerfile.lines().map(str::trim) {
        if line.starts_with('#') || (line.is_empty() && current.is_empty()) {
            continue;
        }
        if let Some(rest) = line.strip_suffix('\\') {
            current.push_str(rest.trim_end());
            current.push(' ');
        } else {
            current.push_str(line);
            all.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        all.push(current);
    }
    all
}

/// Splits a command into words at blanks, keeping quoted text together and
/// dropping the quotes.
fn words(command: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut started = false;
    for c in command.chars() {
        match quote {
            Some(open) if c == open => quote = None,
            Some(_) => word.push(c),
            None if c == '\'' || c == '"' => {
                quote = Some(c);
                started = true;
            }
            None if c.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            None => {
                word.push(c);
                started = true;
            }
        }
    }
    if started {
        words.push(word);
    }
    words
}

/// What the first `RUN` after `COPY Cargo.toml` creates.
fn skeleton(dockerfile: &str) -> Result<Skeleton, String> {
    let all = instructions(dockerfile);
    let copy = all
        .iter()
        .position(|instruction| instruction.starts_with("COPY Cargo.toml"))
        .ok_or("the Dockerfile has no `COPY Cargo.toml` step")?;
    let run = all[copy + 1..]
        .iter()
        .find_map(|instruction| instruction.strip_prefix("RUN "))
        .ok_or("no `RUN` step follows `COPY Cargo.toml`")?;

    let mut skeleton = Skeleton::default();
    for command in run.split("&&") {
        let parts = words(command);
        let Some((program, args)) = parts.split_first() else {
            continue;
        };
        let operands = || args.iter().filter(|arg| !arg.starts_with('-'));
        match program.as_str() {
            "mkdir" => {
                let with_parents = args.iter().any(|arg| arg == "-p");
                for dir in operands().map(|dir| normalize(dir)) {
                    if with_parents {
                        skeleton.folders.extend(
                            Path::new(&dir)
                                .ancestors()
                                .filter_map(Path::to_str)
                                .filter(|ancestor| !ancestor.is_empty())
                                .map(str::to_owned),
                        );
                    } else {
                        skeleton.folders.insert(dir);
                    }
                }
            }
            "touch" => skeleton
                .files
                .extend(operands().map(|file| normalize(file))),
            "echo" => {
                let redirect = args.iter().position(|arg| arg == ">" || arg == ">>");
                if let Some(target) = redirect.and_then(|at| args.get(at + 1)) {
                    skeleton.files.insert(normalize(target));
                }
            }
            _ => {}
        }
    }
    Ok(skeleton)
}

/// What is wrong with `dockerfile`'s skeleton step for `manifest`: one line for
/// each target file it does not create, or creates in a folder it never made.
fn problems(manifest: &str, dockerfile: &str) -> Vec<String> {
    let skeleton = skeleton(dockerfile).expect("the Dockerfile has a skeleton step");
    let mut problems = Vec::new();
    for need in needed_files(manifest) {
        if !skeleton.files.contains(&need.file) {
            problems.push(format!(
                "{} needs {}, which the skeleton step does not create",
                need.wanted_by, need.file
            ));
            continue;
        }
        let folder = Path::new(&need.file)
            .parent()
            .and_then(Path::to_str)
            .unwrap_or_default();
        if !folder.is_empty() && !skeleton.folders.contains(folder) {
            problems.push(format!(
                "{} needs {}, but the skeleton step never makes the folder {folder}",
                need.wanted_by, need.file
            ));
        }
    }
    problems
}

#[test]
fn the_skeleton_step_creates_every_file_cargo_toml_names() {
    let problems = problems(MANIFEST, DOCKERFILE);

    assert!(
        problems.is_empty(),
        "add these to the skeleton step of the Dockerfile, or the image build fails:\n{}",
        problems.join("\n")
    );
}

#[test]
fn every_target_header_in_cargo_toml_is_seen() {
    let needed = needed_files(MANIFEST);

    assert!(
        needed.iter().any(|need| need.wanted_by == "lib"),
        "the [lib] section is read: {needed:?}"
    );
    // Count the headers in the raw text, so a parser that read nothing could
    // not make the test above pass for no reason.
    for section in TARGET_SECTIONS {
        let header = format!("[[{section}]]");
        let in_text = MANIFEST
            .lines()
            .filter(|line| line.trim() == header)
            .count();
        let read = needed
            .iter()
            .filter(|need| need.wanted_by.starts_with(&format!("{section} `")))
            .count();
        assert_eq!(read, in_text, "{header} entries read from Cargo.toml");
    }
}

#[test]
fn removing_any_one_target_from_the_real_skeleton_is_caught() {
    let needed = needed_files(MANIFEST);
    assert!(!needed.is_empty());

    for need in needed {
        let without = DOCKERFILE.replace(&need.file, "");
        assert_ne!(
            without, DOCKERFILE,
            "{} is not in the Dockerfile at all",
            need.file
        );

        let found = problems(MANIFEST, &without);

        assert_eq!(found.len(), 1, "without {}: {found:?}", need.file);
        assert!(
            found[0].contains(&need.file) && found[0].starts_with(&need.wanted_by),
            "{found:?}"
        );
    }
}

#[test]
fn a_new_target_that_is_not_in_the_real_skeleton_is_caught() {
    for (section, file) in [
        ("bin", "src/bin/brand_new.rs"),
        ("example", "examples/brand_new.rs"),
        ("bench", "benches/brand_new.rs"),
        ("test", "tests/brand_new.rs"),
    ] {
        let manifest = format!("{MANIFEST}\n[[{section}]]\nname = \"brand_new\"\n");

        let found = problems(&manifest, DOCKERFILE);

        assert_eq!(found.len(), 1, "{section}: {found:?}");
        assert!(found[0].contains(file), "{found:?}");
    }
}

const SYNTHETIC_MANIFEST: &str = r#"
[package]
name = "demo"

[lib]
name = "demo_lib"

[[bin]]
name = "demo"

[[bin]]
name = "other"

[[bin]]
name = "tool"
path = "tools/tool_main.rs"

[[example]]
name = "sample"

[[bench]]
name = "speed"
harness = false

[[test]]
name = "slow"
"#;

const SYNTHETIC_FILES: [&str; 7] = [
    "src/lib.rs",
    "src/main.rs",
    "src/bin/other.rs",
    "tools/tool_main.rs",
    "examples/sample.rs",
    "benches/speed.rs",
    "tests/slow.rs",
];

#[test]
fn targets_without_a_path_are_found_where_cargo_looks_for_them() {
    let files: Vec<String> = needed_files(SYNTHETIC_MANIFEST)
        .into_iter()
        .map(|need| need.file)
        .collect();

    assert_eq!(files, SYNTHETIC_FILES);
}

#[test]
fn a_skeleton_written_in_the_usual_ways_is_read_in_full() {
    let dockerfile = "\
FROM rust:1.99 AS builder
# A comment before the step.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src/bin tools ./examples/ \\
    benches tests && \\
    # A comment inside the step.
    touch src/lib.rs \\
          benches/speed.rs examples/sample.rs tests/slow.rs && \\
    echo 'fn main() {}' > src/main.rs && \\
    echo \"fn main() {}\" >> src/bin/other.rs && \\
    echo 'fn main() {}' > tools/tool_main.rs
RUN touch src/never_counted.rs
";

    let skeleton = skeleton(dockerfile).unwrap();

    assert_eq!(
        skeleton
            .files
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [
            "benches/speed.rs",
            "examples/sample.rs",
            "src/bin/other.rs",
            "src/lib.rs",
            "src/main.rs",
            "tests/slow.rs",
            "tools/tool_main.rs",
        ],
        "only the first RUN after COPY Cargo.toml counts"
    );
    assert!(problems(SYNTHETIC_MANIFEST, dockerfile).is_empty());
}

#[test]
fn a_target_in_a_folder_that_is_never_made_is_caught() {
    let dockerfile = "COPY Cargo.toml ./\nRUN mkdir -p src benches tests tools && \\\n\
                      touch src/lib.rs src/main.rs src/bin/other.rs tools/tool_main.rs \\\n\
                      examples/sample.rs benches/speed.rs tests/slow.rs\n";

    let found = problems(SYNTHETIC_MANIFEST, dockerfile);

    assert_eq!(found.len(), 2, "{found:?}");
    assert!(
        found.iter().any(|line| line.contains("src/bin/other.rs")),
        "{found:?}"
    );
    assert!(
        found.iter().any(|line| line.contains("examples/sample.rs")),
        "{found:?}"
    );
    assert!(found
        .iter()
        .all(|line| line.contains("never makes the folder")));
}

#[test]
fn mkdir_without_p_makes_only_the_folder_it_names() {
    let dockerfile = "COPY Cargo.toml ./\nRUN mkdir src/bin && touch src/bin/other.rs\n";

    let skeleton = skeleton(dockerfile).unwrap();

    assert!(skeleton.folders.contains("src/bin"));
    assert!(!skeleton.folders.contains("src"));
}

#[test]
fn a_dockerfile_without_a_skeleton_step_is_an_error() {
    assert!(skeleton("FROM rust\nRUN touch src/lib.rs\n").is_err());
    assert!(skeleton("FROM rust\nCOPY Cargo.toml ./\nCOPY src ./src\n").is_err());
}
