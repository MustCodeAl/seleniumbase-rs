//! The shape of a call that takes Python's keyword arguments.
//!
//! An async Python method with keyword arguments, such as
//! `highlight(selector, loops=4, scroll=True, timeout=None)`, becomes a call
//! builder here. The required arguments stay positional, in Python's order. Each
//! keyword argument is a builder method with the Python name, and `.await` runs
//! the call, because the builder implements [`std::future::IntoFuture`]:
//!
//! ```text
//! sb.method(required).await?;                      // every keyword at its default
//! sb.method(required).keyword(value).await?;       // a keyword argument
//! ```
//!
//! A call written without keyword arguments therefore reads as it did before the
//! method became a builder, and no `_with` or `_for` twin of the method is needed.
//!
//! Every builder is `#[must_use]`, since it does nothing until it is awaited, and
//! its future is [`CallFuture`], which is `Send`, so a test can hand a call to
//! `tokio::spawn`. A synchronous Python method with optional arguments is not a
//! builder: it takes `Option<_>` parameters in Python's order.

use std::future::Future;
use std::pin::Pin;

use crate::error::SeleniumBaseError;

/// The future of a call builder: boxed, `Send`, and borrowing the case (and the
/// selector) the builder was made from for `'a`.
///
/// A builder names it as its [`std::future::IntoFuture::IntoFuture`] type, with
/// `T` the value the call returns on success.
pub type CallFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, SeleniumBaseError>> + Send + 'a>>;

#[cfg(test)]
mod tests {
    use std::future::IntoFuture;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::CallFuture;
    use crate::api::base_case::BaseCase;
    use crate::browser::config::BrowserConfig;
    use crate::error::SeleniumBaseError;

    /// A builder of the shape every keyword-argument method has: required
    /// arguments in `new`, a method per keyword, `IntoFuture` to run it.
    #[must_use = "nothing happens until the call is awaited"]
    struct Probe<'a> {
        sb: &'a BaseCase,
        ran: &'a AtomicUsize,
        loops: Option<usize>,
    }

    impl<'a> Probe<'a> {
        fn new(sb: &'a BaseCase, ran: &'a AtomicUsize) -> Self {
            Self {
                sb,
                ran,
                loops: None,
            }
        }

        fn loops(mut self, loops: usize) -> Self {
            self.loops = Some(loops);
            self
        }
    }

    impl<'a> IntoFuture for Probe<'a> {
        type Output = Result<usize, SeleniumBaseError>;
        type IntoFuture = CallFuture<'a, usize>;

        fn into_future(self) -> Self::IntoFuture {
            Box::pin(async move {
                let loops = self.loops.unwrap_or(1);
                self.ran.fetch_add(loops, Ordering::SeqCst);
                // The borrowed case is held across this await, which is what
                // makes the future `Send` or not.
                self.sb.sleep(0.0).await;
                Ok(loops)
            })
        }
    }

    fn assert_send<T: Send>(_: &T) {}

    fn case() -> BaseCase {
        BaseCase::without_session(BrowserConfig::default())
    }

    #[test]
    fn the_future_of_a_builder_is_send() {
        let sb = case();
        let ran = AtomicUsize::new(0);
        let future = Probe::new(&sb, &ran).loops(2).into_future();
        assert_send(&future);
    }

    #[test]
    fn a_builder_does_nothing_until_it_is_awaited() {
        let sb = case();
        let ran = AtomicUsize::new(0);
        let _probe = Probe::new(&sb, &ran).loops(3);
        assert_eq!(ran.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn a_builder_runs_with_its_keyword_arguments_when_awaited() {
        let sb = case();
        let ran = AtomicUsize::new(0);

        // Awaiting a builder runs the call, with the defaults of the keywords
        // it was not given.
        let result = Probe::new(&sb, &ran).await;
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(ran.load(Ordering::SeqCst), 1);

        let result = Probe::new(&sb, &ran).loops(3).await;
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(ran.load(Ordering::SeqCst), 4);
    }
}
