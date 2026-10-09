//! A small lock helper shared by the Pure CDP modules.

use std::sync::{Mutex, MutexGuard, PoisonError};

/// Locks `mutex`, carrying on if another thread panicked while holding it.
///
/// Every critical section guarded this way is a single read, insert or remove,
/// so a panic cannot leave the data half-updated. Continuing is safe, and it
/// keeps one failed task from taking down a long-running server.
pub(crate) fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
