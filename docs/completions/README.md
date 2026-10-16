# Fig completion spec for `sbrs`

`fig-sbrs.ts` is a completion spec for the `sbrs` command in the Fig format, the
format of the [withfig/autocomplete](https://github.com/withfig/autocomplete)
repository. Warp's built-in completions, inshellisense and Amazon Q Developer
CLI read specs from it. The file is **prepared here, not submitted**: nothing in
this folder has been sent to upstream, and whether to send it is for the
maintainer of this repository to decide.

## What the file is

- The output of `sbrs completions fig` (sbrs 0.3.0, which writes it with
  `clap_complete_fig`), formatted with the prettier settings of withfig/autocomplete
  (`trailingComma: "es5"`, `printWidth: 80`). Upstream's `pnpm lint` runs
  `prettier --check`, which the raw output fails; the formatting only wraps long
  descriptions.
- The CLI definition has not changed since 0.3.0 (the commits after the tag change
  no command, option or description; one changes how `sbrs` ends when its output
  pipe closes), so the spec matches the current source.
- It is the CLI of a build **without the `turso` feature**, which is how the
  Homebrew tap and a plain `cargo install --path .` build `sbrs`. `sbrs report`
  exists only with `turso`, so the spec has no `report` subcommand. The `tui`
  feature changes what some commands do, not the command line, so it does not
  matter here.
- 112 top-level subcommands (225 command entries with the nested ones) and 24
  top-level options. It uses `template: "filepaths"` in five places, and has no
  `generators` and no `script`, so it runs nothing in a user's shell. Two
  descriptions contain a URL (`https://example.com` and `http://localhost:4444`),
  which upstream's review bot lists.

To regenerate it after the CLI changes, from an `sbrs` built without `turso`
(it reproduces the saved file byte for byte today):

```bash
sbrs completions fig \
  | prettier --no-config --trailing-comma es5 --print-width 80 --parser typescript \
  > docs/completions/fig-sbrs.ts
```

### Where the spec and the CLI differ

`clap_complete_fig` writes two attributes that do not match how `sbrs` parses
its command line:

- Every option that takes a value is marked `isRepeatable: true` (138 of them),
  although most may be given only once: a second `--browser`, for example, is an
  error. Only the few options that collect a list (such as `--features` and
  `--assert`) may repeat.
- When such an option is optional, its value is marked `isOptional: true`
  (top-level `--browser`, `--webdriver`, `--user-agent` and others), although
  `sbrs` needs the value. Fig may then offer subcommands and options where the
  value should go.

Both were checked with `sbrs` 0.3.0: `--browser chrome --browser edge` and a
`--browser` with no value each end with an error and exit status 2.

Completion still offers the right names. A reviewer upstream may ask for these
to be corrected by hand; if so, the saved file stops being the generator's
exact output.

## Before you decide

Checked on 2026-10-15:

- withfig/autocomplete is not archived, but its last commit and last merged pull
  request are from 2025-05-05, and pull requests opened since (the newest from
  2026-10-04) have not been merged. A new spec may wait a long time for a review,
  or never get one.
- The README says Amazon Q Developer CLI is the former Fig. The "How to
  Contribute" page it links (`fig.io/docs/getting-started/contributing`) answered
  with HTTP 503. The steps below come from the repository's own README,
  CONTRIBUTING.md, `package.json` and workflows instead.
- There is no spec named `sbrs`, `seleniumbase` or similar in the repository.
- Contributors must sign a CLA (`cla/cla.md`; the bot asks for a pull request
  comment). Among other things it has you state that the work is yours.
- Without the upstream spec, `sbrs completions bash|zsh|fish` still gives
  shell-native completion, and Warp uses that by default (see
  [CLI Usage](../tutorials/cli_usage.md#install-shell-completions)).

What was checked on the file, and what was not:

- Checked: it loads as a module (Node 24 type stripping) and has the shape above;
  `prettier --check` passes with upstream's settings (prettier 3.2.5 here, upstream
  asks for ^3.4.2).
- Not checked: upstream's `pnpm test` (`tsc --noEmit` against
  `@withfig/autocomplete-types`) and `pnpm lint` (eslint with
  `@fig/eslint-config-autocomplete`). They need a clone of withfig/autocomplete and
  `pnpm install`, which was not done. Run them in your fork before you open the
  pull request. `pnpm dev` (trying the spec live) needs the Amazon Q app.

## Steps to submit

Requirements from upstream: Node 20 or newer and pnpm 9 or newer.

```bash
# 1. Fork https://github.com/withfig/autocomplete on GitHub, then:
git clone https://github.com/YOUR_GITHUB_USERNAME/autocomplete.git
cd autocomplete
git remote add upstream https://github.com/withfig/autocomplete.git
git fetch upstream
git switch -c feat/sbrs-spec upstream/master

# 2. Install, and add the spec as src/sbrs.ts
pnpm install
cp /path/to/seleniumbase-rs/docs/completions/fig-sbrs.ts src/sbrs.ts

# 3. Check it the way upstream's CI does
pnpm lint:fix     # eslint --fix and prettier --write
pnpm test         # tsc --noEmit
pnpm build        # compiles src/ to build/

# 4. Commit (conventional commit, the CLI's name as the scope, imperative,
#    72 characters or fewer on the first line) and push
git add src/sbrs.ts
git commit -m "feat(sbrs): add completion spec"
git push -u origin feat/sbrs-spec
```

Then open the pull request against `withfig/autocomplete` `master`, with the text
below. The CI jobs are a build, a linter (`pnpm lint`), a typecheck, a bot that
comments with the spec's URLs and scripts, and the CLA check: sign it by commenting
`I have read the CLA Document and I hereby sign the CLA` on the pull request when
the bot asks. If `pnpm lint:fix` changes the file, send the changed file, not the
one in this folder.

## Pull request text

Title:

```text
feat(sbrs): add completion spec
```

Description (tick the boxes that are true after you ran the checks):

```markdown
Adds a completion spec for `sbrs`, the command-line tool of
[seleniumbase-rs](https://github.com/MustCodeAl/seleniumbase-rs), a Rust port of
SeleniumBase (browser automation and testing).

How it was made: `sbrs completions fig` (clap_complete_fig) writes the spec from
the CLI's own definition, so the subcommands, aliases, options and descriptions
match `sbrs --help` of the default build (the optional `turso` feature adds a
`report` subcommand, which the spec leaves out). The file was then formatted
with this repository's prettier settings. To regenerate it:
`sbrs completions fig`.

Known differences from the CLI, both from the generator: every option that
takes a value is marked `isRepeatable`, although most may be given only once,
and the value of an optional option is marked `isOptional`, although `sbrs`
needs it. I can correct them by hand if you prefer.

- 112 top-level subcommands, 225 command entries with the nested ones.
- `template: "filepaths"` only; no generators and no `script`.
- Short and long forms share one `name` array; there are no combined short flags.

Checklist from the greeting comment:

- [ ] Do all subcommands / options which take arguments include the `args` property?
      The generator writes `args` for every option and argument the CLI defines.
- [ ] Are all options modular (`-a -u -x`, not `-aux`)?
- [ ] Have all other checks passed (`pnpm lint`, `pnpm test`, `pnpm build`)?
```

Install `sbrs` with `cargo install --path .` in this repository, or from the
Homebrew tap, to run the generator yourself.
