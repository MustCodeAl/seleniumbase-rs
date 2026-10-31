#!/usr/bin/env bash
# Checks the contracts between this crate and the repositories that build on it.
#
# Every repository that uses this crate has a contract file that lists, as Rust, everything it
# imports from here and pins the signatures it calls: tests/contract_stealth.rs,
# contract_companion.rs, contract_plugins.rs and contract_tauri.rs, and
# crates/cli/tests/contract_homebrew.rs for the Homebrew tap. A change to a listed item breaks
# the contract test, which names the consumer. This script is the other half: it reads the
# sibling checkouts and fails, naming the path, when a consumer uses something its contract file
# does not list, so a contract cannot lag behind the repository it describes.
#
# Usage:
#
#   scripts/check-consumers.sh [--check-build] [--sync] [--list CONSUMER] [CONSUMER...]
#
#   CONSUMER      one of stealth companion plugins tauri homebrew (default: all of them)
#   --list NAME   print the `seleniumbase_rs::...` paths NAME uses, one per line, and stop
#   --sync        copy the companion's golden programs into tests/contract_companion/ and stop
#   --check-build also run `cargo check --tests` in each consumer against this checkout (a
#                 `[patch]` on this crate's git URL); heavy (stealth builds BoringSSL with cmake),
#                 so it is for a release, not for every change
#
# The checkouts are found beside this one (../seleniumbase-rs-stealth, ../seleniumbase-rs-companion,
# ../seleniumbase-rs-plugins, ../tauri-plugin-sbrs-automation, ../homebrew-tap) or in
# SBRS_STEALTH_DIR, SBRS_COMPANION_DIR, SBRS_PLUGINS_DIR, SBRS_TAURI_DIR and SBRS_TAP_DIR.
# SBRS_CONTRACTS_DIR and SBRS_CLI_CONTRACTS_DIR move the folders that hold the contract files
# (tests/ and crates/cli/tests/); the tests of this script use them to run it on a fixture.
#
# A path is what a `use` line names, with the braces written out: `use a::{b, c::{d, e}};` is
# the four paths a::b, a::c::d and a::c::e, and a path in an expression (`a::b::c(..)`) or a
# rustdoc link counts too. Only `.rs` files are read (`.ts` too for the companion, whose
# generators write Rust as text). The contract file is read the same way, without its comment
# lines, so a path has to be in code there.
#
# Exit status: 0 when every check passes, 1 when one fails, 2 for a wrong command line.

set -euo pipefail

core=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
beside=$(dirname "$core")
contracts=${SBRS_CONTRACTS_DIR:-$core/tests}
cli_contracts=${SBRS_CLI_CONTRACTS_DIR:-$core/crates/cli/tests}
git_url='https://github.com/MustCodeAl/seleniumbase-rs'

all_consumers='stealth companion plugins tauri homebrew'

# Reads source text and prints every path that starts with the awk variable `prefix`. With
# `comments=skip`, lines that start with `//` are left out.
read -r -d '' awk_program <<'AWK' || true
function skipws() {
    while (pos <= n && substr(text, pos, 1) ~ /[ \t\r\n]/) pos++
}
function skipgroup(    depth, c) {
    depth = 1
    while (pos <= n && depth > 0) {
        c = substr(text, pos, 1)
        if (c == "{") depth++
        else if (c == "}") depth--
        pos++
    }
}
function emit(path) {
    sub(/::self$/, "", path)
    sub(/::$/, "", path)
    print path
}
function tree(base,    seg, c) {
    skipws()
    seg = ""
    while (pos <= n && substr(text, pos, 1) ~ /[A-Za-z0-9_:*]/) {
        seg = seg substr(text, pos, 1)
        pos++
    }
    if (seg == "") return
    skipws()
    if (seg ~ /::$/ && substr(text, pos, 1) == "{") {
        pos++
        skipws()
        # `{${names}}`: a list that code writes at run time. Its names are not here.
        if (substr(text, pos, 1) == "$") { skipgroup(); return }
        while (pos <= n) {
            tree(base seg)
            skipws()
            c = substr(text, pos, 1)
            while (pos <= n && c != "," && c != "}") { pos++; c = substr(text, pos, 1) }
            pos++
            if (c == "}") return
            skipws()
            if (substr(text, pos, 1) == "}") { pos++; return }
        }
        return
    }
    emit(base seg)
}
function flush(    from, hit, start, before) {
    n = length(text)
    from = 1
    while ((hit = index(substr(text, from), prefix)) > 0) {
        start = from + hit - 1
        before = (start > 1) ? substr(text, start - 1, 1) : ""
        if (before ~ /[A-Za-z0-9_]/) { from = start + length(prefix); continue }
        pos = start
        tree("")
        from = (pos > start) ? pos : start + length(prefix)
    }
    text = ""
}
FNR == 1 && NR > 1 { flush() }
{
    if (comments == "skip" && $0 ~ /^[ \t]*\/\//) next
    text = text $0 "\n"
}
END { flush() }
AWK

fail=0

# A message for a failed check; the script goes on to the next one and fails at the end.
bad() {
    printf 'FAIL %s\n' "$*" >&2
    fail=1
}

# The folder of a consumer's checkout.
consumer_dir() {
    case $1 in
        stealth) echo "${SBRS_STEALTH_DIR:-$beside/seleniumbase-rs-stealth}" ;;
        companion) echo "${SBRS_COMPANION_DIR:-$beside/seleniumbase-rs-companion}" ;;
        plugins) echo "${SBRS_PLUGINS_DIR:-$beside/seleniumbase-rs-plugins}" ;;
        tauri) echo "${SBRS_TAURI_DIR:-$beside/tauri-plugin-sbrs-automation}" ;;
        homebrew) echo "${SBRS_TAP_DIR:-$beside/homebrew-tap}" ;;
    esac
}

# The contract file of a consumer.
contract_file() {
    case $1 in
        homebrew) echo "$cli_contracts/contract_homebrew.rs" ;;
        *) echo "$contracts/contract_$1.rs" ;;
    esac
}

# scan DIR PREFIX COMMENTS EXTENSION...: the paths under DIR that start with PREFIX, in the
# files with one of the extensions (build output, dependencies and other checkouts are skipped).
scan() {
    local dir=$1 prefix=$2 comments=$3 extension
    shift 3
    local names=()
    for extension in "$@"; do
        names+=(-o -name "*.$extension")
    done
    names=("${names[@]:1}")
    find "$dir" \
        \( -name target -o -name node_modules -o -name .git -o -name .tokensave -o -name .claude -o -name dist \) -prune \
        -o -type f \( "${names[@]}" \) \
        -exec awk -v prefix="$prefix" -v comments="$comments" "$awk_program" {} +
}

# The paths a consumer uses, sorted, once each. For `plugins` the consumer is this crate: the
# paths it uses of the plugin crates it depends on.
consumer_paths() {
    local dir
    dir=$(consumer_dir "$1")
    {
        case $1 in
            stealth | tauri) scan "$dir" 'seleniumbase_rs::' keep rs ;;
            companion) scan "$dir" 'seleniumbase_rs::' keep rs ts ;;
            plugins)
                scan "$core/src" 'seleniumbase_rs_plugin::' keep rs
                scan "$core/crates/cli/src" 'seleniumbase_rs_plugin::' keep rs
                scan "$core/src" 'seleniumbase_rs_plugins_builtin::' keep rs
                scan "$core/crates/cli/src" 'seleniumbase_rs_plugins_builtin::' keep rs
                ;;
        esac
    } | LC_ALL=C sort -u
}

# The paths a contract file lists (comment lines left out).
contract_paths() {
    {
        case $1 in
            plugins)
                scan "$2" 'seleniumbase_rs_plugin::' skip rs
                scan "$2" 'seleniumbase_rs_plugins_builtin::' skip rs
                ;;
            *) scan "$2" 'seleniumbase_rs::' skip rs ;;
        esac
    } | LC_ALL=C sort -u
}

# check_paths CONSUMER: every path the consumer uses is in its contract file.
check_paths() {
    local name=$1 dir contract wanted listed missing
    dir=$(consumer_dir "$name")
    contract=$(contract_file "$name")
    if [ ! -f "$contract" ]; then
        bad "$name: the contract file $contract does not exist"
        return
    fi
    if [ "$name" != plugins ] && [ ! -d "$dir" ]; then
        bad "$name: no checkout at $dir (set the variable named in --help)"
        return
    fi
    wanted=$(consumer_paths "$name")
    listed=$(contract_paths "$name" "$contract")
    missing=$(LC_ALL=C comm -23 <(printf '%s\n' "$wanted") <(printf '%s\n' "$listed"))
    if [ -n "$missing" ]; then
        while IFS= read -r path; do
            bad "$name: $path is used but $contract does not list it"
        done <<<"$missing"
        return
    fi
    printf 'ok   %s: %s paths, all in %s\n' "$name" "$(printf '%s\n' "$wanted" | wc -l | tr -d ' ')" "${contract#"$core"/}"
}

# check_golden: the companion's golden programs are the copies in tests/contract_companion/.
check_golden() {
    local dir golden copy name
    dir=$(consumer_dir companion)
    [ -d "$dir/tests/golden" ] || { bad "companion: no $dir/tests/golden"; return; }
    for golden in "$dir"/tests/golden/*; do
        name=$(basename "$golden")
        copy=$contracts/contract_companion/$name
        if ! cmp -s "$golden" "$copy"; then
            bad "companion: tests/golden/$name differs from ${copy#"$core"/} (run scripts/check-consumers.sh --sync)"
        fi
    done
    printf 'ok   companion: golden programs equal the copies in tests/contract_companion/\n'
}

# check_plugins_rev: both lines of Cargo.toml name the same plugins repository and commit, and
# the commit is on the main branch of the plugins checkout.
check_plugins_rev() {
    local dir manifest lines revs urls rev
    dir=$(consumer_dir plugins)
    manifest=$core/Cargo.toml
    lines=$(grep -E '^seleniumbase-rs-plugin(s-builtin)? = ' "$manifest" || true)
    if [ "$(printf '%s\n' "$lines" | wc -l | tr -d ' ')" != 2 ]; then
        bad "plugins: expected two plugin lines in Cargo.toml (seleniumbase-rs-plugin and seleniumbase-rs-plugins-builtin)"
        return
    fi
    revs=$(printf '%s\n' "$lines" | sed -E 's/.*rev = "([0-9a-f]+)".*/\1/' | sort -u)
    urls=$(printf '%s\n' "$lines" | sed -E 's/.*git = "([^"]+)".*/\1/' | sort -u)
    if [ "$(printf '%s\n' "$revs" | wc -l | tr -d ' ')" != 1 ] || [ "$(printf '%s\n' "$urls" | wc -l | tr -d ' ')" != 1 ]; then
        bad "plugins: the two plugin lines of Cargo.toml name different sources: $(printf '%s ' "$revs" "$urls")"
        return
    fi
    rev=$revs
    if [ ! -d "$dir/.git" ] && [ ! -f "$dir/.git" ]; then
        bad "plugins: no checkout at $dir (SBRS_PLUGINS_DIR)"
        return
    fi
    if ! git -C "$dir" merge-base --is-ancestor "$rev" main 2>/dev/null; then
        bad "plugins: rev $rev of Cargo.toml is not an ancestor of main in $dir (push it, or fetch)"
        return
    fi
    printf 'ok   plugins: both lines pin %s, an ancestor of main\n' "$rev"
}

# check_formula: what the formula's `test do` and `install` rely on is in contract_homebrew.rs.
check_formula() {
    local dir formula contract code literals missing
    dir=$(consumer_dir homebrew)
    formula=$dir/Formula/sbrs.rb
    contract=$(contract_file homebrew)
    if [ ! -f "$formula" ]; then
        bad "homebrew: no formula at $formula (SBRS_TAP_DIR)"
        return
    fi
    if [ ! -f "$contract" ]; then
        bad "homebrew: the contract file $contract does not exist"
        return
    fi
    literals=$(
        {
            sed -n -E 's/^[[:space:]]*assert_match "([^"]+)".*/\1/p' "$formula"
            sed -n -E 's/.*shells: \[([^]]*)\].*/\1/p' "$formula" | tr ',' '\n' | sed -E 's/[[:space:]:]//g'
            sed -n -E 's/.*shell_output\("#\{bin\}\/sbrs ([^"]*)"\).*/\1/p' "$formula" | tr ' ' '\n'
            sed -n -E 's/.*Dir\["([^"*]+)\/\*\.yaml"\].*/\1/p' "$formula"
        } | LC_ALL=C sort -u
    )
    # The strings of the code of the contract file: one in a comment does not count.
    code=$(grep -v '^[[:space:]]*//' "$contract" || true)
    missing=
    while IFS= read -r literal; do
        [ -n "$literal" ] || continue
        if ! printf '%s\n' "$code" | grep -F -q -- "\"$literal\""; then
            bad "homebrew: the formula relies on \"$literal\" but ${contract#"$core"/} has no such string"
            missing=1
        fi
    done <<<"$literals"
    [ -n "$missing" ] || printf 'ok   homebrew: %s strings of the formula are in %s\n' "$(printf '%s\n' "$literals" | wc -l | tr -d ' ')" "${contract#"$core"/}"
}

# check_build CONSUMER: `cargo check --tests` in the consumer, with this checkout in place of the
# git dependency on this crate.
check_build() {
    local name=$1 dir patch
    dir=$(consumer_dir "$name")
    patch="patch.\"$git_url\".seleniumbase-rs.path=\"$core\""
    case $name in
        stealth) (cd "$dir" && cargo check --tests --workspace --config "$patch") ;;
        companion) (cd "$dir" && cargo check --tests --manifest-path e2e/Cargo.toml --config "$patch") ;;
        tauri) (cd "$dir" && cargo check --tests --features stealth --config "$patch") ;;
        *)
            printf 'skip %s: nothing to build\n' "$name"
            return
            ;;
    esac || bad "$name: cargo check --tests against this checkout failed"
    printf 'ok   %s: builds against this checkout\n' "$name"
}

sync_golden() {
    local dir
    dir=$(consumer_dir companion)
    [ -d "$dir/tests/golden" ] || { echo "no $dir/tests/golden" >&2; exit 1; }
    mkdir -p "$contracts/contract_companion"
    cp "$dir"/tests/golden/* "$contracts/contract_companion/"
    printf 'copied the golden programs of %s into %s\n' "$dir" "${contracts#"$core"/}/contract_companion"
}

usage() {
    sed -n '2,/^$/p' "${BASH_SOURCE[0]}" | sed -E 's/^# ?//'
}

run_check_build=
chosen=
while [ $# -gt 0 ]; do
    case $1 in
        --check-build) run_check_build=1 ;;
        --sync)
            sync_golden
            exit 0
            ;;
        --list)
            [ $# -ge 2 ] || { echo "--list needs a consumer name" >&2; exit 2; }
            consumer_paths "$2"
            exit 0
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        stealth | companion | plugins | tauri | homebrew) chosen="$chosen $1" ;;
        *)
            echo "unknown argument: $1 (see --help)" >&2
            exit 2
            ;;
    esac
    shift
done
[ -n "$chosen" ] || chosen=$all_consumers

for name in $chosen; do
    case $name in
        homebrew) check_formula ;;
        *) check_paths "$name" ;;
    esac
    case $name in
        companion) check_golden ;;
        plugins) check_plugins_rev ;;
    esac
    if [ -n "$run_check_build" ]; then
        check_build "$name"
    fi
done

exit "$fail"
