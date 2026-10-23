//! JUnit XML for a run (`--junit-xml`): the format CI systems read.
//!
//! [`render_junit_xml`] makes the document, shaped like the one pytest writes:
//! a `<testsuites>` root with one `<testsuite>` per suite (the test binary, or
//! the feature file of a BDD scenario) and one `<testcase>` per test.
//!
//! | Outcome | Element |
//! |---|---|
//! | passed | none |
//! | failed | `<failure message="first line" type="kind">detail</failure>` |
//! | skipped | `<skipped message="reason"/>` |
//! | unreported | `<error message="the test did not finish" type="unreported"/>` |
//! | untested | left out while the run is live; unreported once it is finished |
//!
//! A failed test also gets a `<system-out>` with the page it ended on
//! (`Last Page: <url>`) and one `[[ATTACHMENT|<absolute path>]]` line per
//! evidence file, the convention the Jenkins JUnit attachments plugin reads.
//!
//! Test names and messages come from the code under test, so every text and
//! attribute is escaped (`& < > " '`), ANSI escape sequences are removed, and
//! so are the characters XML 1.0 does not allow at all. An attachment is
//! only named for a plain relative path inside the run's folder.
//!
//! The file is written whole to a temporary neighbour and renamed into place,
//! as the other reports are, so a CI step reading it never sees half a file.

use std::fmt::Write as _;
use std::io;
use std::path::Path;

use crate::core::dashboard::is_plain_relative;
use crate::core::report_helper::{write_atomically, RunSnapshot, TestResult, TestStatus};

/// The name of the `<testsuites>` root, and the suite of a test whose name has
/// no suite in front of it.
const ROOT_NAME: &str = "sbrs";

/// The JUnit XML of `run`, with the name of this machine as the `hostname` of
/// every suite.
///
/// See the [module](self) documentation for what is written.
#[must_use]
pub fn render_junit_xml(run: &RunSnapshot) -> String {
    render_junit_xml_for_host(run, &hostname())
}

/// Like [`render_junit_xml`], with `hostname` as the `hostname` of every suite,
/// so the output does not depend on the machine it was made on.
#[must_use]
pub fn render_junit_xml_for_host(run: &RunSnapshot, hostname: &str) -> String {
    let cases = cases(run);
    let mut suites: Vec<(&str, Vec<&Case<'_>>)> = Vec::new();
    for case in &cases {
        match suites.iter_mut().find(|(name, _)| *name == case.suite) {
            Some((_, members)) => members.push(case),
            None => suites.push((&case.suite, vec![case])),
        }
    }
    let timestamp = run.started_at.format("%Y-%m-%dT%H:%M:%S%.6f").to_string();
    let hostname = escape(hostname);
    let total = Tally::of(cases.iter());

    let mut xml = String::with_capacity(512 + cases.len() * 256);
    xml.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    // Writing to a `String` cannot fail.
    let _ = writeln!(
        xml,
        "<testsuites name=\"{ROOT_NAME}\" tests=\"{}\" failures=\"{}\" errors=\"{}\" \
         skipped=\"{}\" time=\"{:.3}\" timestamp=\"{timestamp}\">",
        total.tests, total.failures, total.errors, total.skipped, total.time
    );
    for (name, members) in &suites {
        let tally = Tally::of(members.iter().copied());
        let _ = writeln!(
            xml,
            "  <testsuite name=\"{}\" errors=\"{}\" failures=\"{}\" skipped=\"{}\" \
             tests=\"{}\" time=\"{:.3}\" timestamp=\"{timestamp}\" hostname=\"{hostname}\">",
            escape(name),
            tally.errors,
            tally.failures,
            tally.skipped,
            tally.tests,
            tally.time
        );
        for case in members {
            write_case(&mut xml, run, case);
        }
        xml.push_str("  </testsuite>\n");
    }
    xml.push_str("</testsuites>\n");
    xml
}

/// Writes the JUnit XML of `run` to `path`, replacing any file there. The
/// parent directory must exist.
///
/// # Errors
///
/// Returns the I/O error if the file cannot be written.
pub fn write_junit_xml(path: impl AsRef<Path>, run: &RunSnapshot) -> io::Result<()> {
    write_atomically(path.as_ref(), render_junit_xml(run).as_bytes())
}

/// One test as the XML sees it.
struct Case<'a> {
    suite: String,
    classname: String,
    name: String,
    /// The status to write: an untested test of a finished run is unreported.
    status: TestStatus,
    result: &'a TestResult,
}

/// The counts of a group of cases, as the attributes of a suite or the root.
#[derive(Default)]
struct Tally {
    tests: usize,
    failures: usize,
    errors: usize,
    skipped: usize,
    time: f64,
}

impl Tally {
    fn of<'a, 'b: 'a>(cases: impl Iterator<Item = &'a Case<'b>>) -> Self {
        cases.fold(Self::default(), |mut tally, case| {
            tally.tests += 1;
            tally.time += case.result.duration_secs;
            match case.status {
                TestStatus::Failed => tally.failures += 1,
                TestStatus::Unreported | TestStatus::Untested => tally.errors += 1,
                TestStatus::Skipped => tally.skipped += 1,
                TestStatus::Passed => {}
            }
            tally
        })
    }
}

/// The tests of `run` that belong in the XML, in the order they started.
fn cases(run: &RunSnapshot) -> Vec<Case<'_>> {
    run.results
        .iter()
        .filter_map(|result| {
            let status = match result.status {
                TestStatus::Untested if run.finished => TestStatus::Unreported,
                TestStatus::Untested => return None,
                other => other,
            };
            let (suite, classname, name) = split_id(&result.name);
            Some(Case {
                suite,
                classname,
                name,
                status,
                result,
            })
        })
        .collect()
}

/// Splits a display id into the suite, the class name and the test name.
///
/// - `login_chrome::tests::test_login` is the suite `login_chrome`, the class
///   `login_chrome.tests` and the test `test_login`.
/// - `tests/features/login.feature:12 => Successful login` is the suite
///   `tests/features/login.feature`, the class `tests.features.login.feature`
///   and the test `Successful login`.
/// - A name with no `::` is a test of the suite `sbrs`.
fn split_id(display: &str) -> (String, String, String) {
    if let Some((feature, scenario)) = display.split_once(" => ") {
        let path = match feature.rsplit_once(':') {
            Some((path, line)) if !line.is_empty() && line.bytes().all(|b| b.is_ascii_digit()) => {
                path
            }
            _ => feature,
        };
        return (
            path.to_owned(),
            path.replace(['/', '\\'], "."),
            scenario.to_owned(),
        );
    }
    let parts: Vec<&str> = display.split("::").collect();
    match parts.as_slice() {
        [.., name] if parts.len() > 1 && !name.is_empty() => (
            parts[0].to_owned(),
            parts[..parts.len() - 1].join("."),
            (*name).to_owned(),
        ),
        _ => (
            ROOT_NAME.to_owned(),
            ROOT_NAME.to_owned(),
            display.to_owned(),
        ),
    }
}

fn write_case(xml: &mut String, run: &RunSnapshot, case: &Case<'_>) {
    let _ = write!(
        xml,
        "    <testcase classname=\"{}\" name=\"{}\" time=\"{:.3}\"",
        escape(&case.classname),
        escape(&case.name),
        case.result.duration_secs
    );
    let outcome = outcome(case);
    let output = system_out(run, case.result);
    if outcome.is_none() && output.is_none() {
        xml.push_str("/>\n");
        return;
    }
    xml.push_str(">\n");
    if let Some(outcome) = outcome {
        let _ = writeln!(xml, "      {outcome}");
    }
    if let Some(output) = output {
        let _ = writeln!(xml, "      <system-out>{}</system-out>", escape(&output));
    }
    xml.push_str("    </testcase>\n");
}

/// The `<failure>`, `<skipped>` or `<error>` element of a test that did not
/// pass.
fn outcome(case: &Case<'_>) -> Option<String> {
    let result = case.result;
    match case.status {
        TestStatus::Passed => None,
        TestStatus::Failed => {
            let failure = result.failure.as_ref();
            let kind = failure
                .map(|failure| failure.kind.as_str())
                .filter(|kind| !kind.is_empty())
                .unwrap_or("failure");
            let detail = failure
                .and_then(|failure| failure.detail.as_deref())
                .filter(|detail| !detail.is_empty())
                .unwrap_or(&result.message);
            let first_line = result.message.lines().next().unwrap_or_default();
            Some(format!(
                "<failure message=\"{}\" type=\"{}\">{}</failure>",
                escape(first_line),
                escape(kind),
                escape(detail)
            ))
        }
        TestStatus::Skipped => Some(format!(
            "<skipped message=\"{}\"/>",
            escape(&result.message)
        )),
        TestStatus::Untested | TestStatus::Unreported => {
            Some("<error message=\"the test did not finish\" type=\"unreported\"/>".to_owned())
        }
    }
}

/// The page a failed test ended on and its evidence files, one per line.
fn system_out(run: &RunSnapshot, result: &TestResult) -> Option<String> {
    let failure = result.failure.as_ref()?;
    let mut lines = Vec::new();
    if let Some(url) = failure.url.as_deref().filter(|url| !url.is_empty()) {
        lines.push(format!("Last Page: {url}"));
    }
    for evidence in [&failure.screenshot, &failure.page_source]
        .into_iter()
        .flatten()
    {
        if is_plain_relative(evidence) {
            let joined = run.root.join(evidence);
            let absolute = std::path::absolute(&joined).unwrap_or(joined);
            lines.push(format!("[[ATTACHMENT|{}]]", absolute.display()));
        }
    }
    (!lines.is_empty()).then(|| lines.join("\n"))
}

/// The name of this machine as the environment gives it, or `localhost`.
fn hostname() -> String {
    ["HOSTNAME", "COMPUTERNAME", "HOST"]
        .into_iter()
        .find_map(|name| std::env::var(name).ok().filter(|v| !v.trim().is_empty()))
        .unwrap_or_else(|| "localhost".to_owned())
}

/// `text` safe to put in an XML text or in a quoted attribute: ANSI escape
/// sequences and the characters XML 1.0 forbids are removed, `& < > " '` and
/// the carriage return become references, and so do a tab and a line feed
/// (so an attribute keeps them).
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in strip_ansi(text).chars().filter(|&c| allowed_in_xml(c)) {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            '\r' => escaped.push_str("&#13;"),
            c => escaped.push(c),
        }
    }
    escaped
}

/// Whether XML 1.0 allows `c` in a document (its `Char` production).
fn allowed_in_xml(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r')
        || ('\u{20}'..='\u{d7ff}').contains(&c)
        || ('\u{e000}'..='\u{fffd}').contains(&c)
        || c >= '\u{10000}'
}

/// `text` without the ANSI escape sequences a terminal colours its output
/// with: `ESC [ ... final byte`, `ESC ] ... BEL` and a lone `ESC x`.
fn strip_ansi(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            plain.push(c);
            continue;
        }
        match chars.next() {
            // Control sequence: parameters, then one byte from `@` to `~`.
            Some('[') => {
                for next in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&next) {
                        break;
                    }
                }
            }
            // Operating system command: up to BEL or `ESC \`.
            Some(']') => {
                while let Some(next) = chars.next() {
                    if next == '\u{7}' {
                        break;
                    }
                    if next == '\u{1b}' {
                        if chars.peek() == Some(&'\\') {
                            chars.next();
                        }
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    plain
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::report_helper::FailureDetails;
    use chrono::{Local, TimeZone};
    use std::path::PathBuf;

    fn result(name: &str, status: TestStatus, secs: f64, message: &str) -> TestResult {
        TestResult::new(name, status, secs, message)
    }

    fn run_of(results: Vec<TestResult>) -> RunSnapshot {
        let mut run = RunSnapshot::from_results("T", results);
        run.started_at = Local.with_ymd_and_hms(2026, 10, 22, 15, 24, 1).unwrap();
        run.root = PathBuf::from("/run");
        run
    }

    fn xml_of(run: &RunSnapshot) -> String {
        render_junit_xml_for_host(run, "ci-box")
    }

    fn parse(xml: &str) -> roxmltree::Document<'_> {
        roxmltree::Document::parse(xml).unwrap_or_else(|error| panic!("{error}\n{xml}"))
    }

    fn three() -> RunSnapshot {
        run_of(vec![
            result("login::tests::test_login", TestStatus::Passed, 0.5, "ok"),
            result(
                "login::tests::test_logout",
                TestStatus::Failed,
                1.25,
                "boom",
            ),
            result("cart::test_add", TestStatus::Skipped, 0.0, "no browser"),
        ])
    }

    #[test]
    fn the_counts_of_the_root_and_the_suites_add_up() {
        let xml = xml_of(&three());
        let doc = parse(&xml);
        let root = doc.root_element();

        assert_eq!(root.tag_name().name(), "testsuites");
        assert_eq!(root.attribute("name"), Some("sbrs"));
        assert_eq!(root.attribute("tests"), Some("3"));
        assert_eq!(root.attribute("failures"), Some("1"));
        assert_eq!(root.attribute("errors"), Some("0"));
        assert_eq!(root.attribute("skipped"), Some("1"));
        assert_eq!(root.attribute("time"), Some("1.750"));
        assert!(root
            .attribute("timestamp")
            .is_some_and(|t| t.starts_with("2026-10-22T15:24:01.000000")));

        let suites: Vec<_> = root
            .children()
            .filter(roxmltree::Node::is_element)
            .collect();
        assert_eq!(suites.len(), 2);
        assert_eq!(suites[0].attribute("name"), Some("login"));
        assert_eq!(suites[0].attribute("tests"), Some("2"));
        assert_eq!(suites[0].attribute("failures"), Some("1"));
        assert_eq!(suites[0].attribute("time"), Some("1.750"));
        assert_eq!(suites[0].attribute("hostname"), Some("ci-box"));
        assert_eq!(suites[1].attribute("name"), Some("cart"));
        assert_eq!(suites[1].attribute("skipped"), Some("1"));
    }

    #[test]
    fn a_test_is_a_case_with_a_class_a_name_and_a_time() {
        let xml = xml_of(&three());
        let doc = parse(&xml);
        let case = doc
            .descendants()
            .find(|n| n.attribute("name") == Some("test_logout"))
            .unwrap();

        assert_eq!(case.attribute("classname"), Some("login.tests"));
        assert_eq!(case.attribute("time"), Some("1.250"));
        let nested = doc
            .descendants()
            .find(|n| n.attribute("name") == Some("test_add"))
            .unwrap();
        assert_eq!(nested.attribute("classname"), Some("cart"));
        assert!(xml
            .contains("<testcase classname=\"login.tests\" name=\"test_login\" time=\"0.500\"/>"));
    }

    #[test]
    fn a_failure_has_the_first_line_the_kind_and_the_detail() {
        let mut failed = result(
            "ui::test_buy",
            TestStatus::Failed,
            2.0,
            "element not found: #buy\nmore lines",
        );
        failed.failure = Some(FailureDetails {
            kind: "element_not_found".to_owned(),
            detail: Some("element not found: #buy\ncaused by: timeout".to_owned()),
            url: Some("https://shop.test/cart".to_owned()),
            screenshot: Some("latest_logs/ui.test_buy/screenshot.png".to_owned()),
            page_source: Some("latest_logs/ui.test_buy/page_source.html".to_owned()),
            ..FailureDetails::default()
        });
        let xml = xml_of(&run_of(vec![failed]));
        let doc = parse(&xml);

        let failure = doc
            .descendants()
            .find(|n| n.has_tag_name("failure"))
            .unwrap();
        assert_eq!(
            failure.attribute("message"),
            Some("element not found: #buy")
        );
        assert_eq!(failure.attribute("type"), Some("element_not_found"));
        assert_eq!(
            failure.text(),
            Some("element not found: #buy\ncaused by: timeout")
        );
        let out = doc
            .descendants()
            .find(|n| n.has_tag_name("system-out"))
            .and_then(|n| n.text())
            .unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "Last Page: https://shop.test/cart");
        assert!(lines[1].starts_with("[[ATTACHMENT|") && lines[1].ends_with("screenshot.png]]"));
        assert!(lines[2].ends_with("page_source.html]]"));
        assert!(
            Path::new(
                lines[1]
                    .trim_start_matches("[[ATTACHMENT|")
                    .trim_end_matches("]]")
            )
            .is_absolute(),
            "{}",
            lines[1]
        );
    }

    #[test]
    fn a_failure_without_details_is_a_plain_failure() {
        let xml = xml_of(&run_of(vec![result(
            "t::test_a",
            TestStatus::Failed,
            0.0,
            "the test panicked",
        )]));
        let doc = parse(&xml);
        let failure = doc
            .descendants()
            .find(|n| n.has_tag_name("failure"))
            .unwrap();

        assert_eq!(failure.attribute("type"), Some("failure"));
        assert_eq!(failure.text(), Some("the test panicked"));
        assert!(!xml.contains("system-out"));
    }

    #[test]
    fn a_skipped_test_has_its_reason() {
        let xml = xml_of(&run_of(vec![result(
            "t::test_a",
            TestStatus::Skipped,
            0.0,
            "needs a browser",
        )]));
        assert!(
            xml.contains("<skipped message=\"needs a browser\"/>"),
            "{xml}"
        );
    }

    #[test]
    fn unreported_is_an_error_and_untested_is_left_out_while_the_run_is_live() {
        let mut run = run_of(vec![
            result("t::done", TestStatus::Passed, 0.1, "ok"),
            result("t::killed", TestStatus::Unreported, 0.0, ""),
            result("t::running", TestStatus::Untested, 0.0, ""),
        ]);
        run.finished = false;
        let live = xml_of(&run);
        let doc = parse(&live);
        let root = doc.root_element();
        assert_eq!(root.attribute("tests"), Some("2"));
        assert_eq!(root.attribute("errors"), Some("1"));
        assert!(live.contains("<error message=\"the test did not finish\" type=\"unreported\"/>"));
        assert!(!live.contains("running"), "{live}");

        run.finished = true;
        let done = xml_of(&run);
        let doc = parse(&done);
        assert_eq!(doc.root_element().attribute("tests"), Some("3"));
        assert_eq!(doc.root_element().attribute("errors"), Some("2"));
        assert!(done.contains("name=\"running\""));
    }

    #[test]
    fn a_bdd_scenario_uses_the_feature_path_and_the_scenario_name() {
        let xml = xml_of(&run_of(vec![result(
            "tests/features/login.feature:12 => Successful login",
            TestStatus::Passed,
            0.2,
            "ok",
        )]));
        let doc = parse(&xml);
        let suite = doc
            .descendants()
            .find(|n| n.has_tag_name("testsuite"))
            .unwrap();
        let case = doc
            .descendants()
            .find(|n| n.has_tag_name("testcase"))
            .unwrap();

        assert_eq!(
            suite.attribute("name"),
            Some("tests/features/login.feature")
        );
        assert_eq!(
            case.attribute("classname"),
            Some("tests.features.login.feature")
        );
        assert_eq!(case.attribute("name"), Some("Successful login"));
    }

    #[test]
    fn display_ids_split_into_suite_class_and_name() {
        let split = |id: &str| split_id(id);
        let owned = |a: &str, b: &str, c: &str| (a.to_owned(), b.to_owned(), c.to_owned());

        assert_eq!(split("a::b"), owned("a", "a", "b"));
        assert_eq!(
            split("login_chrome::test_login::case_1_firefox"),
            owned("login_chrome", "login_chrome.test_login", "case_1_firefox")
        );
        assert_eq!(split("lonely"), owned("sbrs", "sbrs", "lonely"));
        assert_eq!(split("trailing::"), owned("sbrs", "sbrs", "trailing::"));
        assert_eq!(
            split("f.feature => No line"),
            owned("f.feature", "f.feature", "No line")
        );
        assert_eq!(
            split("C:\\w\\f.feature:3 => Win"),
            owned("C:\\w\\f.feature", "C:.w.f.feature", "Win")
        );
    }

    #[test]
    fn hostile_text_is_escaped_and_control_characters_are_removed() {
        let nasty = "<b>\"q\" & 'a'</b>\u{1b}[31mred\u{1b}[0m\u{0}\u{8}\u{fffe}]]>";
        let mut failed = result(
            "s::<script>alert(1)</script>",
            TestStatus::Failed,
            0.0,
            nasty,
        );
        failed.failure = Some(FailureDetails {
            kind: "k\"><x".to_owned(),
            detail: Some(format!("{nasty}\r\ntab\there")),
            url: Some("javascript:alert(1)".to_owned()),
            ..FailureDetails::default()
        });
        let xml = xml_of(&run_of(vec![failed]));
        let doc = parse(&xml);

        assert!(!xml.contains('\u{1b}') && !xml.contains('\u{0}'), "{xml:?}");
        assert!(!xml.contains("<script>") && !xml.contains("<x"), "{xml}");
        let case = doc
            .descendants()
            .find(|n| n.has_tag_name("testcase"))
            .unwrap();
        assert_eq!(case.attribute("name"), Some("<script>alert(1)</script>"));
        let failure = doc
            .descendants()
            .find(|n| n.has_tag_name("failure"))
            .unwrap();
        assert_eq!(
            failure.attribute("message"),
            Some("<b>\"q\" & 'a'</b>red]]>")
        );
        assert_eq!(failure.attribute("type"), Some("k\"><x"));
        assert!(failure.text().unwrap().contains("red]]>\r\ntab\there"));
    }

    #[test]
    fn an_attachment_is_named_only_for_a_plain_relative_path() {
        let mut failed = result("s::t", TestStatus::Failed, 0.0, "x");
        failed.failure = Some(FailureDetails {
            screenshot: Some("../../etc/passwd".to_owned()),
            page_source: Some("/etc/shadow".to_owned()),
            ..FailureDetails::default()
        });
        let xml = xml_of(&run_of(vec![failed]));

        assert!(!xml.contains("ATTACHMENT"), "{xml}");
        assert!(!xml.contains("system-out"), "{xml}");
    }

    #[test]
    fn an_empty_run_is_a_valid_document_with_no_suites() {
        let xml = xml_of(&run_of(Vec::new()));
        let doc = parse(&xml);

        assert_eq!(doc.root_element().attribute("tests"), Some("0"));
        assert_eq!(doc.root_element().attribute("time"), Some("0.000"));
        assert_eq!(
            doc.descendants()
                .filter(|n| n.has_tag_name("testsuite"))
                .count(),
            0
        );
    }

    #[test]
    fn the_written_file_replaces_the_old_one_and_leaves_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("report.xml");
        std::fs::write(&file, "old").unwrap();

        write_junit_xml(&file, &three()).unwrap();

        let xml = std::fs::read_to_string(&file).unwrap();
        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<testsuites"));
        parse(&xml);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        assert!(write_junit_xml(dir.path().join("no-such-dir").join("r.xml"), &three()).is_err());
    }

    #[test]
    fn the_machine_name_comes_from_the_environment_or_is_localhost() {
        assert!(!hostname().is_empty());
        assert!(render_junit_xml(&three()).contains(" hostname=\""));
    }
}
