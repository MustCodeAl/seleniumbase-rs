//! The arguments of `mkdir` and `mkfile`, and the generator each one makes.
//!
//! (Plain comments on the structs, not docs: a doc comment on a struct that derives `Args`
//! would be help text.)

use seleniumbase_rs::cli::scaffold::ScaffoldError;
use seleniumbase_rs::cli::scripts::sb_mkdir::Suite;
use seleniumbase_rs::cli::scripts::sb_mkfile::{Syntax, TestFile};

use super::language::LanguageFlag;
use crate::failure::ArgumentError;

/// The shape of the generated test, as Python's syntax formats.
#[derive(Clone, Copy, Debug, Eq, PartialEq, usage::ArgGroup)]
#[usage(name = "syntax")]
pub(crate) enum SyntaxFlag {
    /// The rstest `sb` fixture (the default)
    #[usage(long = "pf")]
    Pf,
    #[usage(long = "pytest-fixture", hide)]
    PytestFixture,
    /// The BaseCase made and quit by hand
    #[usage(long = "bc")]
    Bc,
    #[usage(long = "basecase", hide)]
    Basecase,
    /// A closure run by run_browser_test
    #[usage(long = "cm")]
    Cm,
    #[usage(long = "context-manager", hide)]
    ContextManager,
    /// The pure CDP `sb_cdp` fixture
    #[usage(long = "dc")]
    Dc,
    #[usage(long = "driver-context", hide)]
    DriverContext,
    /// A pure CDP browser launched and closed by hand
    #[usage(long = "dm")]
    Dm,
    #[usage(long = "driver-manager", hide)]
    DriverManager,
    /// Not ported: class fixtures have no Rust form (use --pf)
    #[usage(long = "cf")]
    Cf,
    #[usage(long = "class-fixture", hide)]
    ClassFixture,
}

impl SyntaxFlag {
    /// The syntax the flag asks for, or `None` for the class fixtures, which Rust has no form of.
    pub(crate) const fn syntax(self) -> Option<Syntax> {
        match self {
            Self::Pf | Self::PytestFixture => Some(Syntax::Fixture),
            Self::Bc | Self::Basecase => Some(Syntax::BaseCase),
            Self::Cm | Self::ContextManager => Some(Syntax::ContextManager),
            Self::Dc | Self::DriverContext => Some(Syntax::CdpFixture),
            Self::Dm | Self::DriverManager => Some(Syntax::CdpManager),
            Self::Cf | Self::ClassFixture => None,
        }
    }
}

// The arguments of `sbrs mkdir`.
#[derive(Debug, usage::Args)]
#[usage(args_override_self = false)]
pub(crate) struct MkdirArgs {
    /// Folder to create, for example `tests/ui`.
    #[usage(value_hint = usage::ValueHint::DirPath)]
    pub(crate) dir: String,
    /// Create only the scaffolding, with no example tests.
    #[usage(short, long, bool_value)]
    pub(crate) basic: bool,
    /// Also write a GitHub Actions workflow, .github/workflows/sbrs-tests.yml, here.
    #[usage(long, bool_value)]
    pub(crate) gha: bool,
    /// Also write a Gherkin scenario that runs on the built-in steps (needs the bdd feature).
    #[usage(long, bool_value)]
    pub(crate) bdd: bool,
    /// Replace generated files that already exist.
    #[usage(long, bool_value)]
    pub(crate) force: bool,
}

impl MkdirArgs {
    /// The suite the arguments describe.
    pub(crate) fn suite(&self) -> Result<Suite, ScaffoldError> {
        Ok(Suite::new(&self.dir)?
            .basic(self.basic)
            .gha(self.gha)
            .bdd(self.bdd))
    }
}

// The arguments of `sbrs mkfile`.
#[derive(Debug, usage::Args)]
#[usage(args_override_self = false)]
pub(crate) struct MkfileArgs {
    /// File to create, for example `tests/login.rs`.
    #[usage(value_hint = usage::ValueHint::FilePath, extensions("rs"))]
    pub(crate) file: String,
    /// Page the test opens (default: https://example.com). A bare host such as
    /// wikipedia.org gets https://; the test is then the basic one.
    #[usage(long, value_hint = usage::ValueHint::Url)]
    pub(crate) url: Option<String>,
    /// Leave the test body empty: it only opens the page.
    #[usage(short, long, bool_value)]
    pub(crate) basic: bool,
    /// Add a recorder breakpoint for Recorder Mode (also --record, --recorder).
    #[usage(short, long = "rec", aliases = ["record", "recorder"], bool_value)]
    pub(crate) rec: bool,
    /// UC Mode: the run_browser_test closure with the UC driver, a basic body and English.
    #[usage(long, bool_value)]
    pub(crate) uc: bool,
    /// Pure CDP: the sb_cdp fixture, a basic body and English.
    #[usage(long, bool_value)]
    pub(crate) cdp: bool,
    /// Replace the file if it already exists.
    #[usage(long, bool_value)]
    pub(crate) force: bool,
    #[usage(arg_group)]
    pub(crate) language: Option<LanguageFlag>,
    #[usage(arg_group)]
    pub(crate) syntax: Option<SyntaxFlag>,
}

impl MkfileArgs {
    /// Refuses the combinations the parser accepts but the generator cannot make.
    ///
    /// # Errors
    ///
    /// An [`ArgumentError`] naming the flags that do not go together; nothing is written.
    pub(crate) fn check(&self) -> Result<(), ArgumentError> {
        let fail = |message: &str| Err(ArgumentError::new("mkfile", message));
        if matches!(self.syntax, Some(SyntaxFlag::Cf | SyntaxFlag::ClassFixture)) {
            return fail("class fixtures are not ported; use --pf (the rstest sb fixture)");
        }
        if self.uc && self.cdp {
            return fail("--uc and --cdp cannot be combined; pick one");
        }
        if (self.uc || self.cdp) && (self.language.is_some() || self.syntax.is_some()) {
            return fail(
                "--uc and --cdp write an English test in a syntax of their own; \
                 leave out the language and syntax flags",
            );
        }
        if self.rec && (self.uc || self.cdp) {
            return fail("--rec needs WebDriver; leave out --uc and --cdp");
        }
        let syntax = self.syntax.and_then(SyntaxFlag::syntax);
        if self.rec && syntax.is_some_and(Syntax::is_cdp) {
            return fail("--rec needs a WebDriver syntax (--pf, --bc or --cm), not --dc or --dm");
        }
        Ok(())
    }

    /// The test file the arguments describe, once [`check`](Self::check) has passed.
    pub(crate) fn test_file(&self) -> Result<TestFile, ScaffoldError> {
        let mut file = TestFile::new(&self.file)?.basic(self.basic);
        if let Some(url) = &self.url {
            file = file.url(url)?;
        }
        if self.uc {
            return file.uc();
        }
        if self.cdp {
            return file.syntax(Syntax::CdpFixture).map(|file| file.basic(true));
        }
        if let Some(syntax) = self.syntax.and_then(SyntaxFlag::syntax) {
            file = file.syntax(syntax)?;
        }
        if let Some(language) = self.language {
            file = file.language(language.language())?;
        }
        file.recorder(self.rec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::language::LanguageFlag;

    fn args(file: &str) -> MkfileArgs {
        MkfileArgs {
            file: file.to_owned(),
            url: None,
            basic: false,
            rec: false,
            uc: false,
            cdp: false,
            force: false,
            language: None,
            syntax: None,
        }
    }

    fn rendered(args: &MkfileArgs) -> String {
        args.check().expect("the flags go together");
        args.test_file().expect("a test file").render()
    }

    #[test]
    fn class_fixtures_are_refused_and_the_message_names_the_way_out() {
        for flag in [SyntaxFlag::Cf, SyntaxFlag::ClassFixture] {
            let error = MkfileArgs {
                syntax: Some(flag),
                ..args("t")
            }
            .check()
            .unwrap_err();
            assert_eq!(
                error.to_string(),
                "class fixtures are not ported; use --pf (the rstest sb fixture)"
            );
        }
    }

    #[test]
    fn flags_that_cannot_go_together_are_refused_before_anything_is_written() {
        let refused = [
            MkfileArgs {
                uc: true,
                cdp: true,
                ..args("t")
            },
            MkfileArgs {
                uc: true,
                language: Some(LanguageFlag::Zh),
                ..args("t")
            },
            MkfileArgs {
                cdp: true,
                syntax: Some(SyntaxFlag::Bc),
                ..args("t")
            },
            MkfileArgs {
                uc: true,
                rec: true,
                ..args("t")
            },
            MkfileArgs {
                rec: true,
                syntax: Some(SyntaxFlag::Dm),
                ..args("t")
            },
            MkfileArgs {
                rec: true,
                syntax: Some(SyntaxFlag::DriverContext),
                ..args("t")
            },
        ];
        for refused in &refused {
            assert!(refused.check().is_err(), "{refused:?}");
        }
        assert!(MkfileArgs {
            rec: true,
            syntax: Some(SyntaxFlag::Bc),
            ..args("t")
        }
        .check()
        .is_ok());
    }

    #[test]
    fn every_name_of_a_syntax_means_the_same_syntax() {
        for (short, long, syntax) in [
            (SyntaxFlag::Pf, SyntaxFlag::PytestFixture, Syntax::Fixture),
            (SyntaxFlag::Bc, SyntaxFlag::Basecase, Syntax::BaseCase),
            (
                SyntaxFlag::Cm,
                SyntaxFlag::ContextManager,
                Syntax::ContextManager,
            ),
            (
                SyntaxFlag::Dc,
                SyntaxFlag::DriverContext,
                Syntax::CdpFixture,
            ),
            (
                SyntaxFlag::Dm,
                SyntaxFlag::DriverManager,
                Syntax::CdpManager,
            ),
        ] {
            assert_eq!(short.syntax(), Some(syntax));
            assert_eq!(long.syntax(), Some(syntax));
            assert_eq!(syntax.flag(), format!("--{}", flag_name(short)));
        }
        assert_eq!(SyntaxFlag::Cf.syntax(), None);
        assert_eq!(SyntaxFlag::ClassFixture.syntax(), None);
    }

    /// The two-letter flag of a syntax's short member.
    fn flag_name(flag: SyntaxFlag) -> &'static str {
        match flag {
            SyntaxFlag::Pf => "pf",
            SyntaxFlag::Bc => "bc",
            SyntaxFlag::Cm => "cm",
            SyntaxFlag::Dc => "dc",
            SyntaxFlag::Dm => "dm",
            other => unreachable!("{other:?} is not a short member"),
        }
    }

    #[test]
    fn the_flags_choose_the_syntax_the_language_and_the_breakpoint() {
        let default = rendered(&args("t"));
        assert!(default.contains("sb: Sb"), "{default}");

        let cdp_zh = rendered(&MkfileArgs {
            syntax: Some(SyntaxFlag::Dc),
            language: Some(LanguageFlag::ChineseTitle),
            ..args("t")
        });
        assert!(cdp_zh.contains("sb_cdp: SbCdp"), "{cdp_zh}");
        assert!(cdp_zh.contains("你好"), "{cdp_zh}");

        let rec = rendered(&MkfileArgs {
            rec: true,
            syntax: Some(SyntaxFlag::Basecase),
            ..args("t")
        });
        assert!(rec.contains("recorder_breakpoint(sb, "), "{rec}");
        assert!(rec.contains("BaseCase::new"), "{rec}");
    }

    #[test]
    fn uc_and_cdp_are_basic_english_tests() {
        let uc = rendered(&MkfileArgs {
            uc: true,
            ..args("t")
        });
        assert!(uc.contains("DriverMode::Uc"), "{uc}");
        assert!(uc.contains("// Add your steps here."), "{uc}");

        let cdp = rendered(&MkfileArgs {
            cdp: true,
            ..args("t")
        });
        assert!(cdp.contains("sb_cdp: SbCdp"), "{cdp}");
        assert!(cdp.contains("// Add your steps here."), "{cdp}");
    }

    #[test]
    fn a_url_and_a_bad_name_reach_the_library_checks() {
        let site = rendered(&MkfileArgs {
            url: Some("wikipedia.org".to_owned()),
            ..args("t")
        });
        assert!(
            site.contains("sb.open(\"https://wikipedia.org/\")"),
            "{site}"
        );
        assert!(args("../t").test_file().is_err());
    }

    #[test]
    fn the_suite_carries_every_option() {
        let args = MkdirArgs {
            dir: "tests/ui".to_owned(),
            basic: true,
            gha: true,
            bdd: true,
            force: false,
        };
        let names: Vec<String> = args
            .suite()
            .unwrap()
            .files()
            .iter()
            .map(|(name, _)| name.to_string())
            .collect();
        assert!(
            names.contains(&"tests/ui/bdd_test.rs".to_owned()),
            "{names:?}"
        );
        assert!(
            names.contains(&".github/workflows/sbrs-tests.yml".to_owned()),
            "{names:?}"
        );
        assert!(
            !names.contains(&"tests/ui/my_first_test.rs".to_owned()),
            "{names:?}"
        );
    }
}
