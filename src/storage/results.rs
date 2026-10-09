//! A database of test runs, for trends no single report can show.
//!
//! A [`ResultStore`] keeps every run and every result in one embedded Turso
//! file, so a team can ask which tests are flaky, how a run compares with the
//! last one, or which tests fail only against staging, without parsing a pile of
//! JSON reports. It is the Rust counterpart of Python SeleniumBase's database
//! reporting; `sbase report` reads it from the command line.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::core::report_helper::TestResult;
//! use seleniumbase_rs::storage::{ResultStore, RunInfo};
//!
//! # async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! let store = ResultStore::open("reports/results.db").await?;
//! let run = store.start_run(&RunInfo::new("nightly").environment("staging")).await?;
//!
//! store
//!     .record(run, &TestResult {
//!         name: "login_works".into(),
//!         passed: true,
//!         duration_secs: 1.8,
//!         message: "ok".into(),
//!     })
//!     .await?;
//!
//! for run in store.runs(10).await? {
//!     println!("{} {}: {} passed, {} failed", run.id, run.label, run.passed, run.failed);
//! }
//! # Ok(())
//! # }
//! ```

use std::fmt;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use chrono::{DateTime, Utc};
use turso::{Connection, Database};

use super::db;
use crate::core::report_helper::TestResult;
use crate::error::{Result, SeleniumBaseError};

const KIND: &str = "results";
const SCHEMA_VERSION: i64 = 1;
const SCHEMA: &str = "
CREATE TABLE runs (
    id INTEGER PRIMARY KEY,
    label TEXT NOT NULL,
    environment TEXT NOT NULL,
    started_ms INTEGER NOT NULL
);
CREATE TABLE results (
    id INTEGER PRIMARY KEY,
    run_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    passed INTEGER NOT NULL,
    duration_secs REAL NOT NULL,
    message TEXT NOT NULL,
    recorded_ms INTEGER NOT NULL
);
CREATE INDEX results_by_run ON results (run_id);
CREATE INDEX results_by_name ON results (name);
";

/// Identifies one run of a test suite in a [`ResultStore`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RunId(i64);

impl RunId {
    /// The run's number, as `sbase report` prints it.
    #[must_use]
    pub fn get(self) -> i64 {
        self.0
    }
}

impl fmt::Display for RunId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for RunId {
    type Err = SeleniumBaseError;

    /// Parses the number `sbase report` prints for a run.
    fn from_str(text: &str) -> Result<Self> {
        match text.trim().parse::<i64>() {
            Ok(id) if id > 0 => Ok(Self(id)),
            _ => Err(SeleniumBaseError::parse(text, "a run number such as 12")),
        }
    }
}

/// What a run is called and where it ran.
///
/// The environment plays the part of Python SeleniumBase's `--database_env`:
/// results from `staging` and `production` share a database but stay apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunInfo {
    label: String,
    environment: String,
}

impl RunInfo {
    /// Describes a run called `label`, in the `local` environment.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            environment: "local".to_owned(),
        }
    }

    /// Sets the environment the run targets, such as `staging`.
    #[must_use]
    pub fn environment(mut self, environment: impl Into<String>) -> Self {
        self.environment = environment.into();
        self
    }

    /// The run's label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The environment the run targets.
    #[must_use]
    pub fn target(&self) -> &str {
        &self.environment
    }
}

/// How a run stands: who ran it and how many tests passed.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct RunSummary {
    /// The run's number.
    pub id: RunId,
    /// The label it was started with.
    pub label: String,
    /// The environment it targeted.
    pub environment: String,
    /// When it started.
    pub started_at: DateTime<Utc>,
    /// When its latest result was recorded; `None` before the first one.
    pub last_result_at: Option<DateTime<Utc>>,
    /// Results that passed.
    pub passed: u32,
    /// Results that failed.
    pub failed: u32,
}

impl RunSummary {
    /// Results recorded so far.
    #[must_use]
    pub fn total(&self) -> u32 {
        self.passed + self.failed
    }

    /// Whether the run has results and none of them failed.
    #[must_use]
    pub fn all_passed(&self) -> bool {
        self.failed == 0 && self.passed > 0
    }
}

/// A test that has both passed and failed across recent runs.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct FlakyTest {
    /// The test's name.
    pub name: String,
    /// How many times it ran in the runs examined.
    pub attempts: u32,
    /// How many of those attempts failed.
    pub failures: u32,
}

/// Test runs and their results, in an embedded database.
///
/// Cloning a store is cheap and every clone shares the same database.
#[derive(Debug, Clone)]
pub struct ResultStore {
    conn: Connection,
    // Keeps the database open for as long as any clone of the store lives.
    _db: Database,
}

impl ResultStore {
    /// Opens the database at `path`, creating the file and its directory if
    /// they do not exist.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Database`] if the file cannot be opened,
    /// is not a results database, or was written by a newer release, and
    /// [`SeleniumBaseError::Io`] if its directory cannot be created.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::on(db::open_file(path.as_ref()).await?).await
    }

    /// A store that is discarded when the last handle to it is dropped.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Database`] if the database cannot start.
    pub async fn in_memory() -> Result<Self> {
        Self::on(db::open_memory().await?).await
    }

    async fn on(database: Database) -> Result<Self> {
        let conn = database.connect()?;
        db::prepare(&conn, KIND, SCHEMA_VERSION, SCHEMA).await?;
        Ok(Self {
            conn,
            _db: database,
        })
    }

    /// Begins a run and returns its number.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Database`] if the run cannot be stored.
    pub async fn start_run(&self, info: &RunInfo) -> Result<RunId> {
        self.conn
            .execute(
                "INSERT INTO runs (label, environment, started_ms) VALUES (?1, ?2, ?3)",
                (info.label.as_str(), info.environment.as_str(), now_ms()),
            )
            .await?;
        Ok(RunId(self.conn.last_insert_rowid()))
    }

    /// Adds one test's result to `run`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Database`] if `run` does not exist or the
    /// result cannot be stored.
    pub async fn record(&self, run: RunId, result: &TestResult) -> Result<()> {
        if self.run(run).await?.is_none() {
            return Err(SeleniumBaseError::Database(format!(
                "there is no run {run}"
            )));
        }
        // A NaN or infinite duration would not survive the database.
        let duration = if result.duration_secs.is_finite() {
            result.duration_secs
        } else {
            0.0
        };
        self.conn
            .execute(
                "INSERT INTO results (run_id, name, passed, duration_secs, message, recorded_ms) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                (
                    run.0,
                    result.name.as_str(),
                    i64::from(result.passed),
                    duration,
                    result.message.as_str(),
                    now_ms(),
                ),
            )
            .await?;
        Ok(())
    }

    /// Records how a test went: a pass if `outcome` is `Ok`, otherwise a
    /// failure whose message is the error.
    ///
    /// This is the call to put after a test body, with the time it took:
    ///
    /// ```
    /// # use seleniumbase_rs::storage::{ResultStore, RunInfo};
    /// # async fn demo(store: ResultStore) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// # let run = store.start_run(&RunInfo::new("ci")).await?;
    /// let started = std::time::Instant::now();
    /// let outcome: Result<(), seleniumbase_rs::SeleniumBaseError> = Ok(());
    /// store.record_outcome(run, "login_works", started.elapsed(), &outcome).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// As [`record`](Self::record).
    pub async fn record_outcome<E: fmt::Display>(
        &self,
        run: RunId,
        name: &str,
        elapsed: Duration,
        outcome: &std::result::Result<(), E>,
    ) -> Result<()> {
        self.record(run, &outcome_to_result(name, elapsed, outcome))
            .await
    }

    /// The run numbered `id`, or `None` if there is no such run.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Database`] if the query fails.
    pub async fn run(&self, id: RunId) -> Result<Option<RunSummary>> {
        let mut found = self.summaries(Some(id), 1).await?;
        Ok(found.pop())
    }

    /// The latest `limit` runs, newest first.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Database`] if the query fails.
    pub async fn runs(&self, limit: usize) -> Result<Vec<RunSummary>> {
        self.summaries(None, limit).await
    }

    async fn summaries(&self, only: Option<RunId>, limit: usize) -> Result<Vec<RunSummary>> {
        let mut rows = self
            .conn
            .query(
                "SELECT r.id, r.label, r.environment, r.started_ms, MAX(x.recorded_ms), \
                        COALESCE(SUM(CASE WHEN x.passed = 1 THEN 1 ELSE 0 END), 0), \
                        COALESCE(SUM(CASE WHEN x.passed = 0 THEN 1 ELSE 0 END), 0) \
                 FROM runs r LEFT JOIN results x ON x.run_id = r.id \
                 WHERE (?1 = 0 OR r.id = ?1) \
                 GROUP BY r.id ORDER BY r.id DESC LIMIT ?2",
                (only.map_or(0, |id| id.0), saturating_i64(limit)),
            )
            .await?;
        let mut runs = Vec::new();
        while let Some(row) = rows.next().await? {
            runs.push(RunSummary {
                id: RunId(db::integer(&row, 0)?),
                label: db::text(&row, 1)?,
                environment: db::text(&row, 2)?,
                started_at: from_ms(db::integer(&row, 3)?)?,
                last_result_at: db::optional_integer(&row, 4)?.map(from_ms).transpose()?,
                passed: count(&row, 5)?,
                failed: count(&row, 6)?,
            });
        }
        Ok(runs)
    }

    /// Every result recorded in `run`, in the order they were recorded.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Database`] if the query fails.
    pub async fn results(&self, run: RunId) -> Result<Vec<TestResult>> {
        let mut rows = self
            .conn
            .query(
                "SELECT name, passed, duration_secs, message FROM results \
                 WHERE run_id = ?1 ORDER BY id",
                [run.0],
            )
            .await?;
        let mut results = Vec::new();
        while let Some(row) = rows.next().await? {
            results.push(TestResult {
                name: db::text(&row, 0)?,
                passed: db::integer(&row, 1)? != 0,
                duration_secs: db::real(&row, 2)?,
                message: db::text(&row, 3)?,
            });
        }
        Ok(results)
    }

    /// Tests that both passed and failed within the latest `runs` runs, the
    /// most often failing first.
    ///
    /// A test that always fails is broken, not flaky, and is left out.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Database`] if the query fails.
    pub async fn flaky_tests(&self, runs: usize) -> Result<Vec<FlakyTest>> {
        let mut rows = self
            .conn
            .query(
                "SELECT name, COUNT(*), SUM(CASE WHEN passed = 0 THEN 1 ELSE 0 END) \
                 FROM results \
                 WHERE run_id IN (SELECT id FROM runs ORDER BY id DESC LIMIT ?1) \
                 GROUP BY name \
                 HAVING SUM(passed) > 0 AND SUM(passed) < COUNT(*) \
                 ORDER BY 3 DESC, name",
                [saturating_i64(runs)],
            )
            .await?;
        let mut flaky = Vec::new();
        while let Some(row) = rows.next().await? {
            flaky.push(FlakyTest {
                name: db::text(&row, 0)?,
                attempts: count(&row, 1)?,
                failures: count(&row, 2)?,
            });
        }
        Ok(flaky)
    }
}

/// Turns how a test went into the result to store.
fn outcome_to_result<E: fmt::Display>(
    name: &str,
    elapsed: Duration,
    outcome: &std::result::Result<(), E>,
) -> TestResult {
    TestResult {
        name: name.to_owned(),
        passed: outcome.is_ok(),
        duration_secs: elapsed.as_secs_f64(),
        message: match outcome {
            Ok(()) => "ok".to_owned(),
            Err(error) => error.to_string(),
        },
    }
}

fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

fn from_ms(ms: i64) -> Result<DateTime<Utc>> {
    DateTime::from_timestamp_millis(ms)
        .ok_or_else(|| SeleniumBaseError::Database(format!("{ms} is not a valid time")))
}

fn saturating_i64(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

fn count(row: &turso::Row, idx: usize) -> Result<u32> {
    u32::try_from(db::integer(row, idx)?)
        .ok()
        .ok_or_else(|| SeleniumBaseError::Database(format!("column {idx} is not a valid count")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(name: &str, passed: bool) -> TestResult {
        TestResult {
            name: name.to_owned(),
            passed,
            duration_secs: 0.5,
            message: if passed { "ok" } else { "boom" }.to_owned(),
        }
    }

    #[test]
    fn a_run_number_parses_only_when_it_is_positive() {
        assert_eq!("12".parse::<RunId>().unwrap().to_string(), "12");
        assert_eq!(" 7 ".parse::<RunId>().unwrap().to_string(), "7");
        for bad in ["0", "-3", "abc", "", "1.5"] {
            assert!(bad.parse::<RunId>().is_err(), "{bad:?}");
        }
    }

    #[test]
    fn an_outcome_becomes_a_pass_or_a_failure_with_the_error_as_message() {
        let ok: std::result::Result<(), String> = Ok(());
        let passed = outcome_to_result("a", Duration::from_millis(1500), &ok);
        assert!(passed.passed);
        assert_eq!(passed.message, "ok");
        assert!((passed.duration_secs - 1.5).abs() < 1e-9);

        let bad: std::result::Result<(), String> = Err("no such element".into());
        let failed = outcome_to_result("a", Duration::ZERO, &bad);
        assert!(!failed.passed);
        assert_eq!(failed.message, "no such element");
    }

    #[tokio::test]
    async fn results_come_back_as_they_were_recorded() {
        let store = ResultStore::in_memory().await.unwrap();
        let run = store.start_run(&RunInfo::new("ci")).await.unwrap();
        store.record(run, &result("first", true)).await.unwrap();
        store.record(run, &result("second", false)).await.unwrap();

        let back = store.results(run).await.unwrap();

        assert_eq!(
            back.iter()
                .map(|r| (r.name.as_str(), r.passed))
                .collect::<Vec<_>>(),
            [("first", true), ("second", false)]
        );
        assert_eq!(back[1].message, "boom");
        assert!((back[0].duration_secs - 0.5).abs() < 1e-9);
    }

    #[tokio::test]
    async fn a_run_summary_counts_passes_and_failures() {
        let store = ResultStore::in_memory().await.unwrap();
        let run = store
            .start_run(&RunInfo::new("nightly").environment("staging"))
            .await
            .unwrap();
        assert_eq!(store.run(run).await.unwrap().unwrap().total(), 0);
        for (name, passed) in [("a", true), ("b", true), ("c", false)] {
            store.record(run, &result(name, passed)).await.unwrap();
        }

        let summary = store.run(run).await.unwrap().unwrap();

        assert_eq!((summary.passed, summary.failed), (2, 1));
        assert_eq!(summary.total(), 3);
        assert!(!summary.all_passed());
        assert_eq!(summary.label, "nightly");
        assert_eq!(summary.environment, "staging");
        assert!(summary.last_result_at.unwrap() >= summary.started_at);
    }

    #[tokio::test]
    async fn an_empty_run_has_no_last_result_and_is_not_green() {
        let store = ResultStore::in_memory().await.unwrap();
        let run = store.start_run(&RunInfo::new("empty")).await.unwrap();

        let summary = store.run(run).await.unwrap().unwrap();

        assert_eq!(summary.last_result_at, None);
        assert!(!summary.all_passed(), "nothing ran, so nothing passed");
    }

    #[tokio::test]
    async fn runs_are_listed_newest_first_and_limited() {
        let store = ResultStore::in_memory().await.unwrap();
        let mut ids = Vec::new();
        for label in ["one", "two", "three"] {
            ids.push(store.start_run(&RunInfo::new(label)).await.unwrap());
        }

        let latest = store.runs(2).await.unwrap();

        assert_eq!(
            latest.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(),
            ["three", "two"]
        );
        assert_eq!(latest[0].id, ids[2]);
        assert_eq!(store.runs(0).await.unwrap().len(), 0);
        assert_eq!(store.runs(usize::MAX).await.unwrap().len(), 3);
    }

    #[tokio::test]
    async fn results_stay_with_their_own_run() {
        let store = ResultStore::in_memory().await.unwrap();
        let a = store.start_run(&RunInfo::new("a")).await.unwrap();
        let b = store.start_run(&RunInfo::new("b")).await.unwrap();
        store.record(a, &result("only_in_a", true)).await.unwrap();
        store.record(b, &result("only_in_b", false)).await.unwrap();

        assert_eq!(store.results(a).await.unwrap()[0].name, "only_in_a");
        assert_eq!(store.results(b).await.unwrap()[0].name, "only_in_b");
        assert_eq!(store.run(a).await.unwrap().unwrap().failed, 0);
        assert_eq!(store.run(b).await.unwrap().unwrap().passed, 0);
    }

    #[tokio::test]
    async fn recording_into_a_run_that_does_not_exist_is_refused() {
        let store = ResultStore::in_memory().await.unwrap();

        let error = store
            .record("99".parse().unwrap(), &result("orphan", true))
            .await
            .unwrap_err();

        assert!(matches!(error, SeleniumBaseError::Database(_)), "{error}");
        assert!(error.to_string().contains("no run 99"), "{error}");
    }

    #[tokio::test]
    async fn only_tests_that_both_pass_and_fail_are_flaky() {
        let store = ResultStore::in_memory().await.unwrap();
        // `flaky` fails once in three, `broken` always fails, `steady` always
        // passes, `flaky_more` fails twice in three.
        let plan = [
            [
                ("flaky", true),
                ("broken", false),
                ("steady", true),
                ("flaky_more", true),
            ],
            [
                ("flaky", false),
                ("broken", false),
                ("steady", true),
                ("flaky_more", false),
            ],
            [
                ("flaky", true),
                ("broken", false),
                ("steady", true),
                ("flaky_more", false),
            ],
        ];
        for results in plan {
            let run = store.start_run(&RunInfo::new("r")).await.unwrap();
            for (name, passed) in results {
                store.record(run, &result(name, passed)).await.unwrap();
            }
        }

        let flaky = store.flaky_tests(10).await.unwrap();

        assert_eq!(
            flaky
                .iter()
                .map(|t| (t.name.as_str(), t.attempts, t.failures))
                .collect::<Vec<_>>(),
            [("flaky_more", 3, 2), ("flaky", 3, 1)],
            "most failures first; the always-failing and always-passing tests are not flaky"
        );
    }

    #[tokio::test]
    async fn flakiness_only_looks_at_the_latest_runs() {
        let store = ResultStore::in_memory().await.unwrap();
        // The failure is in the oldest run; the two newest runs both pass.
        for passed in [false, true, true] {
            let run = store.start_run(&RunInfo::new("r")).await.unwrap();
            store.record(run, &result("t", passed)).await.unwrap();
        }

        assert_eq!(store.flaky_tests(3).await.unwrap().len(), 1);
        assert!(store.flaky_tests(2).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_message_with_quotes_and_unicode_survives() {
        let store = ResultStore::in_memory().await.unwrap();
        let run = store
            .start_run(&RunInfo::new("it's \"quoted\" ✓"))
            .await
            .unwrap();
        let tricky = TestResult {
            name: "naïve; DROP TABLE results;--".to_owned(),
            passed: false,
            duration_secs: 0.0,
            message: "expected 'a' but got \"b\"\nsecond line ✗".to_owned(),
        };
        store.record(run, &tricky).await.unwrap();

        let back = store.results(run).await.unwrap();

        assert_eq!(back[0].name, tricky.name);
        assert_eq!(back[0].message, tricky.message);
        assert_eq!(
            store.run(run).await.unwrap().unwrap().label,
            "it's \"quoted\" ✓"
        );
    }

    #[tokio::test]
    async fn a_duration_that_is_not_a_number_is_stored_as_zero() {
        let store = ResultStore::in_memory().await.unwrap();
        let run = store.start_run(&RunInfo::new("r")).await.unwrap();
        for duration_secs in [f64::NAN, f64::INFINITY] {
            let mut bad = result("t", true);
            bad.duration_secs = duration_secs;
            store.record(run, &bad).await.unwrap();
        }

        let back = store.results(run).await.unwrap();

        assert!(back.iter().all(|r| r.duration_secs == 0.0));
    }

    #[tokio::test]
    async fn a_database_on_disk_keeps_its_runs_across_reopening() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("results.db");
        let run = {
            let store = ResultStore::open(&path).await.unwrap();
            let run = store.start_run(&RunInfo::new("kept")).await.unwrap();
            store.record(run, &result("t", true)).await.unwrap();
            run
        };

        let reopened = ResultStore::open(&path).await.unwrap();

        assert_eq!(reopened.run(run).await.unwrap().unwrap().label, "kept");
        assert_eq!(reopened.results(run).await.unwrap().len(), 1);
        let next = reopened.start_run(&RunInfo::new("later")).await.unwrap();
        assert!(next > run, "run numbers keep counting up");
    }

    #[tokio::test]
    async fn clones_share_one_database() {
        let store = ResultStore::in_memory().await.unwrap();
        let clone = store.clone();
        let run = store.start_run(&RunInfo::new("shared")).await.unwrap();

        clone.record(run, &result("t", true)).await.unwrap();

        assert_eq!(store.results(run).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn a_file_that_is_not_a_results_database_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("other.db");
        {
            let db = db::open_file(&path).await.unwrap();
            let conn = db.connect().unwrap();
            db::prepare(&conn, "something-else", 1, "CREATE TABLE t (x INTEGER)")
                .await
                .unwrap();
        }

        let error = ResultStore::open(&path).await.unwrap_err();

        assert!(
            error
                .to_string()
                .contains("something-else database, not a results"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn a_database_from_a_newer_release_is_refused_not_misread() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("future.db");
        {
            let db = db::open_file(&path).await.unwrap();
            let conn = db.connect().unwrap();
            db::prepare(&conn, KIND, SCHEMA_VERSION + 1, SCHEMA)
                .await
                .unwrap();
        }

        let error = ResultStore::open(&path).await.unwrap_err();

        assert!(error.to_string().contains("upgrade"), "{error}");
    }
}
