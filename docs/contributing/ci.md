---
title: "CI on GitLab"
sidebarTitle: "CI on GitLab"
description: "The GitLab pipeline: its jobs, the security scanners and what they cover for Rust, the merge-request flow with GitLab Duo Code Review, and where coverage-guided fuzzing plugs in."
---

CI runs on GitLab.com (`gitlab.com/seleniumbaserust/seleniumbase-rs`), on its shared Linux
runners, and the definition is `.gitlab-ci.yml`. The repository is mirrored there by the lead;
the GitHub workflows in `.github/workflows` still exist, and only the release and Docker ones
run there. This page is the long form of the "CI on GitLab" part of
[Contributing](/CONTRIBUTING). The GitLab pages it relies on were read on 2026-11-06, when
GitLab.com ran 19.6; GitLab renames and retires CI features often, so check the page a section
names before you change that part of `.gitlab-ci.yml`.

## When a pipeline is created

The `workflow:` rules of `.gitlab-ci.yml` decide it:

| Event | Pipeline |
|---|---|
| A merge request is opened or its source branch is pushed to | A merge request pipeline. |
| A push to `main` | A branch pipeline. |
| A push to a `lane/` or `batch/` branch with no open merge request | A branch pipeline. |
| A push to a branch that has an open merge request | None. The merge request pipeline covers the push; without this rule every push would run the whole test suite twice. |
| Build, then Pipelines, then New pipeline | A pipeline by hand: our jobs are manual, the scanners run. |

A new push cancels a pipeline of the same kind that is still running.

## The jobs

| Job | What it runs | Stops the pipeline when it fails |
|---|---|---|
| `gate` | `cargo fmt --all -- --check` and clippy with `-D warnings` | Yes |
| `test` | `cargo test --no-fail-fast` and `cargo doc --no-deps` with `-D warnings` | Yes |
| `supply-chain` | `cargo deny --all-features --workspace check` and `cargo audit` | Yes |
| `docs` | `mise run mdx` and `mdbook build`, only when the docs or their tooling changed | Yes |
| `secret_detection` | GitLab secret detection | No |
| `dependency-scanning` | GitLab dependency scanning of `Cargo.lock` | No |
| `gitlab-advanced-sast-ext` | GitLab Advanced SAST (beta) for the Rust files | No |
| `gitlab-advanced-sast` | GitLab Advanced SAST for the JavaScript, Python, TypeScript and Ruby files | No |
| `msrv`, `chrome` | Manual: the declared Rust version, and the real-Chrome tests | Never start by themselves |

Every job takes its tools from [mise](https://mise.jdx.dev/continuous-integration.html), so a
job and a local `mise install` use the same versions: the Rust toolchain, cargo-deny, Python and
mdBook of `mise.toml`. The Rust jobs run on the `rust` image (its C toolchain, system libraries
and rustup) and install mise with its install script, at the release `MISE_VERSION` names; mise
then selects the toolchain of `mise.toml` through the image's rustup and sets
`RUSTUP_TOOLCHAIN`, so the jobs run cargo through `mise exec --` and add the clippy and rustfmt
components themselves (rustup ranks that variable above `rust-toolchain.toml`). `docs` runs on
mise's own image. Each job names the tools it needs in `MISE_ENABLE_TOOLS`, and the jobs that
install more than Rust (`supply-chain`, `docs`) keep mise's folders in their cache.

## The security scanners

The scanners are GitLab's own templates, included at the top of `.gitlab-ci.yml`:

```yaml
include:
  - template: Jobs/SAST.gitlab-ci.yml
  - template: Jobs/Secret-Detection.gitlab-ci.yml
  - template: Jobs/Dependency-Scanning.v2.gitlab-ci.yml
```

They are a GitLab Ultimate feature; the group has it through the Open Source plan. Each adds a
job to the `test` stage, and each template marks its job `allow_failure`, so a scanner that
breaks (an unreachable image, a timeout) turns its job orange and leaves the pipeline green. A
scanner that works and finds something also leaves the job green: the findings are reports, not
a failed exit code. Read them in the **Security** tab of the pipeline, in the security widget of
the merge request, and, for the default branch, in **Secure**, then **Vulnerability report**.
Each job also keeps its JSON report as an artifact (`gl-sast-report.json`,
`gl-secret-detection-report.json`, `gl-dependency-scanning-report.json`).

Two settings in the `variables:` of `.gitlab-ci.yml` matter:

- `AST_ENABLE_MR_PIPELINES: "true"`. Without it the templates run their jobs in branch
  pipelines only, and the merge request pipeline would hold no scanner. With it they run in the
  merge request pipeline and skip the branch pipeline of a branch that has an open merge request.
  The workflow rule that drops that branch pipeline makes the two agree.
- `GITLAB_ADVANCED_SAST_ENABLED: "true"`. See the next section.

The templates define no `workflow:` and no `stages:`, so they cannot change which pipelines are
created. Do not mix the stable and the `latest` edition of the security templates in one project;
GitLab warns that doing so can start both a merge request and a branch pipeline. The dependency
scanning template is the versioned `v2` (the SBOM-based scanning), which GitLab documents as
the one to use beside the stable SAST and secret detection templates.

### Rust and SAST

GitLab's Semgrep-based SAST analyzer has no Rust rules, and the SAST template does not even
route `.rs` files to its job (`semgrep-sast`). Rust is analysed only by **GitLab Advanced SAST**,
which supports it in beta since GitLab 19.5, as a separate job `gitlab-advanced-sast-ext`. The
job appears when `GITLAB_ADVANCED_SAST_ENABLED` is `"true"` and the repository holds `.rs` files.
Beta means rules and findings can change between GitLab releases, custom rulesets and
diff-based scanning do not apply to it, and its findings are not yet merged with Semgrep's. Treat
a finding as a lead to check, not as a verdict.

Its first scan analyses the whole repository; later scans re-analyse the changed code and keep
their state in the job cache (`GITLAB_ADVANCED_SAST_EXT_INCREMENTAL_ENABLED`, on by default).

`SAST_EXCLUDED_PATHS` keeps the template default (`spec, test, tests, tmp`), so the integration
tests under `tests/` are not scanned. Change it in `variables:` if you want them in.

### Dependency scanning and licences

The `dependency-scanning` job reads `Cargo.lock` (the only Rust lockfile of the workspace; GitLab's
list of supported files names it), writes a CycloneDX SBOM (`gl-sbom-cargo-cargo.cdx.json`),
and has GitLab match the crates against its advisory database. It needs no variable. By default
it searches the repository root and its direct subdirectories, so it also reads
`tools/mdx-check/bun.lock`, the lockfile of the docs check. The SBOM is kept as a job artifact.

GitLab's page on licence scanning lists Cargo only under the dependency scanning CI/CD component
and not under the template this repository includes, and the SBOM of the template carries no
licence fields, so do not count on the licence list or on a **licence approval policy**
(**Secure**, then **Policies**, a project setting and not a line of `.gitlab-ci.yml`) for crates
until the dependency list shows licences for them.

`cargo deny` and `cargo audit` in `supply-chain` stay the gate for advisories and licences: they
fail the pipeline and are configured in `deny.toml`. The GitLab scanner adds a second advisory
source and the vulnerability report; it does not replace them.

### Secret detection

`secret_detection` scans the files and the commits of the pipeline: on a merge request all the
commits of the branch, on `main` the commits since the previous push. It does not look at the
history that is already in `main`. To scan the whole history once, run the job with the variable
`SECRET_DETECTION_HISTORIC_SCAN` set to `true`, in a pipeline of a branch that has no open merge
request. The project refuses variables on a pipeline started by hand
(**Settings**, then **CI/CD**, then **Variables**, **Minimum role to use pipeline variables** is
"No one allowed"), so either an Owner relaxes that setting for the run, or a throwaway commit
adds `SECRET_DETECTION_HISTORIC_SCAN: "true"` to `variables:` and the next commit takes it out
again. A secret that a scan finds is leaked: rotate it first, then remove it from the code.

### Not added

- **Code Quality.** The template that runs the CodeClimate engine is deprecated since GitLab
  17.3 with removal planned for 19.0, and it needs Docker-in-Docker. GitLab's replacement is to
  import the report of a linter you already run, in the Code Quality JSON format, as a
  `codequality` report artifact. GitLab's list of tools with a documented integration has no
  Rust tool, so a clippy report would need a converter that is not part of GitLab. No job for
  it yet.
- **Coverage-guided fuzzing.** See the last section: there is no fuzz crate yet.

## The merge-request flow

A lane or batch branch is gated on GitLab before `main` moves to it:

1. Push the branch to the `gitlab` remote. Until a merge request exists this makes a branch
   pipeline.

   ```bash Terminal
   git push gitlab lane/my-change
   ```

2. Open the merge request. `--fill` takes the title and the description from the commits.

   ```bash Terminal
   glab mr create --fill --target-branch main --no-editor
   ```

3. **GitLab Duo Code Review** reviews it (next section). Read its comments, fix what is right,
   and answer the rest; `@GitLabDuo` in a reply asks it a follow-up question.
4. The merge request pipeline must be green: `gate`, `test`, `supply-chain`, and `docs` when
   the docs changed. Open the scanners' findings in the security widget of the merge request;
   a new finding is a reason to fix or to dismiss it with a stated reason.
5. The lead then fast-forwards `main` to the gated commit on GitHub and on GitLab and checks
   that local and remote agree. That fast-forward is the merge: the GitLab merge button and
   force-pushes are not used. Close the merge request if GitLab has not marked it merged by
   itself.

### GitLab Duo Code Review

Duo reviews a merge request when it is created (not as a draft, and with changes), or when you
assign it as a reviewer: assign `@GitLabDuo` in the sidebar, or write
`/assign_reviewer @GitLabDuo` in a comment. GitLab runs one of two features and picks by the seat
of the person who starts the review:

| Feature | Needs | Notes |
|---|---|---|
| GitLab Duo Code Review (non-agentic) | A GitLab Duo Enterprise seat | Single pass over the diff and the changed files. |
| Code Review Flow (agentic) | No add-on; it uses GitLab Credits and a runner | Follows the repository's structure, and starts a follow-up review on each push when **Start a new review on push** is on. |

The settings are in the project under **Settings**, then **Merge requests**, then **GitLab Duo
Code Review**: **Enable automatic reviews by GitLab Duo** and, for Code Review Flow,
**Start a new review on push**. A group Owner can set them for the whole group (**Settings**,
then **General**, then **Merge requests**), and a top-level group Owner can switch Code Review
Flow on or off under **Settings**, then **GitLab Duo**. On 2026-11-06 the project and the group
already had automatic reviews on. A repository may add its own review rules in
`.gitlab/duo/mr-review-instructions.yaml`; this repository has none yet.

A Duo review is a first pass. It does not replace the pipeline or a person who reads the diff.
It sends the merge request title, description and diff, and the contents of the changed files,
to a language model, so a merge request never carries a secret.

## Coverage-guided fuzzing

There is no `fuzz/` crate yet, so `.gitlab-ci.yml` has no fuzz job. This is how one plugs in.

<Warning>
GitLab deprecated coverage-guided fuzz testing in 18.0 and planned its removal in 19.0; its
documentation page is marked for removal on 2026-12-31. On 2026-11-06 the template and the
`gitlab-cov-fuzz` binary it downloads still resolved. Before building on it, read the current
page, `user/application_security/coverage_fuzzing`. If GitLab has removed it, use the plain
`cargo fuzz` job described at the end of this section.
</Warning>

**The crate.** `cargo fuzz init` creates `fuzz/` with `fuzz/Cargo.toml`, a `.gitignore` and the
first target in `fuzz/fuzz_targets/`. cargo-fuzz needs a nightly compiler and a C++ compiler,
and runs on x86-64 and Aarch64 Unix-like systems only (not on Windows). `cargo fuzz init
--fuzzing-workspace=true` makes `fuzz/` a workspace of its own, which keeps it out of the builds,
the `Cargo.lock` and the `cargo deny` run of the stable workspace; by default it joins the root
workspace, and its README says to add `fuzz` to `workspace.members`. Whichever way it is created,
a new package in the repository must also be added to the skeleton step of the `Dockerfile`
(see `CLAUDE.md`), and its dependencies reviewed like any other.

**The job.** The template is `Security/Coverage-Fuzzing.gitlab-ci.yml`. It defines the hidden job
`.fuzz_base`, which each fuzz target extends, one job per target. The template's job lives in a
stage `fuzz`, so `stages:` must then list every stage the file uses:

```yaml
stages:
  - test
  - fuzz

include:
  - template: Security/Coverage-Fuzzing.gitlab-ci.yml

fuzz_parse:
  extends: .fuzz_base
  image: rustlang/rust:nightly
  script:
    - apt-get update -qq && apt-get install -y -qq git make clang cmake
    - export CC=`which clang`
    - export CXX=`which clang++`
    - cargo install cargo-fuzz
    - cargo fuzz run parse -- -runs=0
    - ./gitlab-cov-fuzz run --regression=$REGRESSION -- ./fuzz/target/x86_64-unknown-linux-gnu/release/parse
```

This is the shape of GitLab's own Rust example (`rust-fuzzing-example`), with the target `parse`
as a placeholder. `cargo fuzz run parse -- -runs=0` builds the target without fuzzing it, and
`gitlab-cov-fuzz` then runs the built binary. Do not override the keys `before_script`,
`artifacts` or `rules` of `.fuzz_base` without copying their content: the template sets
`REGRESSION` there.

**What decides the run length.** `.fuzz_base` sets `REGRESSION=true` everywhere except on the
branch named by `COVFUZZ_BRANCH` (the default branch by default). A regression run replays the
stored corpus and the earlier crashes and stops; the default branch runs the fuzzer for real,
10 minutes by default. Pass `COVFUZZ_ADDITIONAL_ARGS` (for example `-max_total_time=300`) to
`gitlab-cov-fuzz` to change it.

**Merge requests need a rules override.** The template's `rules` start a fuzz job only when
`$CI_COMMIT_BRANCH` is set, and `CI_COMMIT_BRANCH` is not set in a merge request pipeline. Our
workflow creates no branch pipeline for a branch with an open merge request, so the unmodified job
would run on `main` and on branches without a merge request, and never in a merge request. To
fuzz in merge requests, copy the template's two `rules` entries into the job and allow the merge
request source as well:

```yaml
  rules:
    - if: $COVFUZZ_DISABLED == 'true' || $COVFUZZ_DISABLED == '1'
      when: never
    - if: ($CI_COMMIT_BRANCH || $CI_PIPELINE_SOURCE == "merge_request_event") && $GITLAB_FEATURES =~ /\bcoverage_fuzzing\b/
```

This follows from reading the template; it has not been run, so check it in the first merge
request that adds the job. Test every change to a security job in a merge request before it
reaches `main`, as GitLab's own page advises.

**Variables.**

| Variable | Use |
|---|---|
| `COVFUZZ_BRANCH` | The branch that runs the long fuzz; every other ref runs the regression. Default: the default branch. |
| `COVFUZZ_ADDITIONAL_ARGS` | Extra arguments for `gitlab-cov-fuzz` and the engine, such as `-max_total_time=300`. |
| `COVFUZZ_SEED_CORPUS` | A seed corpus directory to start from. |
| `COVFUZZ_USE_REGISTRY`, `COVFUZZ_CORPUS_NAME`, `COVFUZZ_GITLAB_TOKEN` | Keep the corpus in the project's corpus registry. The token is a personal or project access token with API access, so it is a masked CI/CD variable in the project settings and never a line of `.gitlab-ci.yml`. |
| `COVFUZZ_DISABLED` | `true` turns the jobs off. |

**Where the results are.** Each fuzz job keeps these artifacts, also when it fails:
`gl-coverage-fuzzing-report.json`, a `corpus` folder (every test case of this and the earlier
jobs) and a `crashes` folder (the crashes of this job and the ones not fixed yet). A crash shows
in the merge request's security widget and in the vulnerability report, with a button that
downloads the artifacts. Locally, cargo-fuzz writes its crash inputs to
`fuzz/artifacts/<target>/` and prints the commands to reproduce and to minimise one:

```bash Terminal
cargo fuzz run parse fuzz/artifacts/parse/crash-HASH
cargo fuzz tmin parse fuzz/artifacts/parse/crash-HASH
```

A crash that GitLab reports is a regression test waiting to be written: copy its input into
a unit test of the crate and fix the bug with that test failing first.

**Without GitLab's template.** If the template is gone, a plain job does the same work: a
manual or scheduled job on the nightly image that installs cargo-fuzz, runs
`cargo fuzz run <target> -- -max_total_time=600`, and keeps `fuzz/artifacts/` (crashes) and
`fuzz/corpus/` as job artifacts with `artifacts: when: always`. It has no security-widget entry,
so a failing run is the signal and the artifact is the evidence.
