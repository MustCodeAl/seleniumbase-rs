//! Allure results for a run (`SB_ALLURE_DIR`, feature `allure`).
//!
//! The sink that writes one `*-result.json` per test is the work of the Allure
//! lane: it implements [`RunSink`](super::test_run::RunSink) and is added to a run
//! with [`TestRun::add_sink`](super::test_run::TestRun::add_sink). The feature has
//! no dependency yet.
