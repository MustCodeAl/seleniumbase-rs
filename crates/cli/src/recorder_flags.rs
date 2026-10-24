//! The Recorder Mode options as flags of `sbrs` itself: `sbrs --rec test`,
//! `sbrs --rec-behave mkrec x.rs`.
//!
//! They are Python's `--rec`, `--rec-print`, `--rec-behave`, `--rec-sb-mgr` and
//! `--rec-sleep` (the pytest options of the Recorder). Each is `global`, so it is
//! accepted before and after the command, and becomes the `SB_*` variable of
//! its row in the option table, which the library reads as it reads the
//! environment (`Settings::apply_options`) and which `sbrs test` hands on to
//! the tests it runs. (`--rec-sb-cdp` is declared at the root by hand, with the
//! options it sits beside.)

/// What Python says when Recorder Mode meets `-n`, with the consequence here:
/// the tests run one at a time instead of the recorder being switched off.
const MULTI_PROCESS_WARNING: &str = "\n  Recorder Mode does NOT support multi-process mode (-n)!\
\n  (DO NOT combine \"--recorder\" with \"-n NUM_PROCESSES\"!)\
\n  (The tests run one at a time while the Recorder is on.)\n";

/// The flags of Recorder Mode.
#[derive(Debug, Default, usage::Args)]
#[usage(args_override_self = false)]
pub(crate) struct RecorderFlags {
    /// Record the browser actions as a test (SB_RECORDER)
    ///
    /// Python's `--rec`. Every test that runs writes `recordings/NAME_rec.rs` when its browser
    /// quits; `sb.recorder_breakpoint("...")` pauses a test so the actions can be done by hand.
    /// `sbrs test` runs the tests one at a time while it is on.
    #[usage(
        long = "rec",
        global,
        visible_aliases = ["recorder", "record", "codegen"],
        bool_value
    )]
    pub(crate) rec: bool,
    /// Print the recording when the test ends (SB_REC_PRINT)
    #[usage(long = "rec-print", global, visible_alias = "rec_print", bool_value)]
    pub(crate) rec_print: bool,
    /// Also write the recording as a Gherkin feature (SB_REC_BEHAVE)
    #[usage(
        long = "rec-behave",
        global,
        visible_aliases = ["rec_behave", "rec-gherkin"],
        bool_value
    )]
    pub(crate) rec_behave: bool,
    /// Write the recording as a script, not a test (SB_REC_SB_MGR)
    #[usage(long = "rec-sb-mgr", global, visible_alias = "rec_sb_mgr", bool_value)]
    pub(crate) rec_sb_mgr: bool,
    /// Keep the sleeps of the recording (SB_REC_SLEEP)
    #[usage(
        long = "rec-sleep",
        global,
        visible_aliases = ["record-sleep", "rec_sleep", "record_sleep"],
        bool_value
    )]
    pub(crate) rec_sleep: bool,
}

impl RecorderFlags {
    /// The `SB_*` variables of the flags that were given, in the order of the table.
    pub(crate) fn options(&self) -> Vec<(&'static str, String)> {
        [
            ("SB_RECORDER", self.rec),
            ("SB_REC_BEHAVE", self.rec_behave),
            ("SB_REC_PRINT", self.rec_print),
            ("SB_REC_SB_MGR", self.rec_sb_mgr),
            ("SB_REC_SLEEP", self.rec_sleep),
        ]
        .into_iter()
        .filter(|(_, on)| *on)
        .map(|(name, _)| (name, "true".to_owned()))
        .collect()
    }
}

/// What to say when the Recorder is on and the person asked for `threads` tests at a time:
/// the warning, if it is more than one. The tests run one at a time all the same.
pub(crate) fn multi_process_warning(threads: Option<usize>) -> Option<&'static str> {
    threads
        .filter(|threads| *threads > 1)
        .map(|_| MULTI_PROCESS_WARNING)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flags_that_were_given_are_variables_in_table_order() {
        let flags = RecorderFlags {
            rec: true,
            rec_print: true,
            rec_behave: false,
            rec_sb_mgr: true,
            rec_sleep: true,
        };

        assert_eq!(
            flags.options(),
            [
                ("SB_RECORDER", "true".to_owned()),
                ("SB_REC_PRINT", "true".to_owned()),
                ("SB_REC_SB_MGR", "true".to_owned()),
                ("SB_REC_SLEEP", "true".to_owned()),
            ]
        );
        assert!(RecorderFlags::default().options().is_empty());
    }

    #[test]
    fn only_more_than_one_thread_gets_the_warning() {
        assert!(multi_process_warning(None).is_none());
        assert!(multi_process_warning(Some(1)).is_none());
        let warning = multi_process_warning(Some(4)).expect("a warning");
        assert!(warning.contains("does NOT support multi-process mode (-n)"));
    }
}
