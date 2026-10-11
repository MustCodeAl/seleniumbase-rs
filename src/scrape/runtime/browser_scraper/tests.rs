//! Tests of the browser scraper against scripted browsers, with no Chrome.
//!
//! The scripted world is in [`rig`]: a launcher that hands out mocked browsers answering like one
//! site, a clock the tests move, and sinks that record what the scraper did.

mod build;
mod flows;
mod rig;
mod rotation;
