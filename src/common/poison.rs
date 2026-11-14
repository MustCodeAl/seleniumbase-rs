//! The one place a poisoned mutex is recovered.
//!
//! A panic while a test holds a process-wide registry (the cleanup list, a
//! reused session, the test logs) must not stop the next test from using it.
//! A poisoned lock otherwise stays poisoned. This module is that one break.

use std::sync::{Mutex, MutexGuard, PoisonError};

/// Locks `mutex` and returns the guard, including when the lock is poisoned.
pub(crate) fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // The one place a poisoned lock is recovered, so the next test can use the registry.
    #[expect(
        clippy::disallowed_methods,
        reason = "the one place a poisoned lock is recovered so the next test can use the registry"
    )]
    {
        mutex.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::sync::Mutex;

    use super::locked;

    #[test]
    fn a_poisoned_mutex_still_yields_its_value() {
        let mutex = Mutex::new(7);
        // The test poisons the mutex on purpose, then checks that `locked` still returns it.
        #[expect(
            clippy::disallowed_methods,
            reason = "the test poisons the mutex on purpose"
        )]
        let _ = catch_unwind(AssertUnwindSafe(|| {
            let _guard = mutex.lock().expect("the test lock");
            panic!("poison the mutex");
        }));
        assert!(mutex.is_poisoned());
        assert_eq!(*locked(&mutex), 7);
    }
}
