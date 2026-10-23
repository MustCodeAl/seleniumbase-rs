//! Test-run bookkeeping: result records and the reports made from them.

#[cfg(feature = "allure")]
pub mod allure;
pub mod dashboard;
pub mod junit;
pub mod log_helper;

pub mod report_helper;
pub mod test_run;
