//! A stack of window handles, for code that moves into another window or tab
//! and has to find its way back.

/// A last-in, first-out stack of browser window handles.
///
/// Push the handle of the window you are leaving before switching to another
/// one, and pop it to go back. The top of the stack is the most recent entry.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::plugins::driver_manager::DriverStack;
///
/// let mut stack = DriverStack::new();
/// stack.push("main-window");
/// stack.push("popup");
/// assert_eq!(stack.current(), Some("popup"));
///
/// assert_eq!(stack.pop().as_deref(), Some("popup"));
/// assert_eq!(stack.current(), Some("main-window"));
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DriverStack {
    stack: Vec<String>,
}

impl DriverStack {
    /// Creates an empty stack.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Pushes a window handle onto the stack.
    pub fn push(&mut self, handle: impl Into<String>) {
        self.stack.push(handle.into());
    }

    /// Removes and returns the most recently pushed window handle, or `None`
    /// if the stack is empty.
    pub fn pop(&mut self) -> Option<String> {
        self.stack.pop()
    }

    /// Returns the most recently pushed window handle without removing it.
    #[must_use]
    pub fn current(&self) -> Option<&str> {
        self.stack.last().map(String::as_str)
    }

    /// Returns the number of handles on the stack.
    #[must_use]
    pub fn len(&self) -> usize {
        self.stack.len()
    }

    /// Returns true when the stack contains no handles.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    /// Removes every handle.
    pub fn clear(&mut self) {
        self.stack.clear();
    }

    /// Iterates over the handles from the oldest to the most recent.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &str> + ExactSizeIterator {
        self.stack.iter().map(String::as_str)
    }
}

impl<S: Into<String>> FromIterator<S> for DriverStack {
    /// Collects handles in order; the last one becomes the top of the stack.
    fn from_iter<I: IntoIterator<Item = S>>(handles: I) -> Self {
        Self {
            stack: handles.into_iter().map(Into::into).collect(),
        }
    }
}

impl<S: Into<String>> Extend<S> for DriverStack {
    fn extend<I: IntoIterator<Item = S>>(&mut self, handles: I) {
        self.stack.extend(handles.into_iter().map(Into::into));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_come_back_most_recent_first() {
        let mut stack = DriverStack::new();
        stack.push("a");
        stack.push(String::from("b"));
        stack.push("c");
        assert_eq!(stack.len(), 3);
        assert_eq!(stack.pop().as_deref(), Some("c"));
        assert_eq!(stack.pop().as_deref(), Some("b"));
        assert_eq!(stack.pop().as_deref(), Some("a"));
        assert_eq!(stack.pop(), None);
        assert!(stack.is_empty());
    }

    #[test]
    fn current_looks_without_removing() {
        let mut stack = DriverStack::new();
        assert_eq!(stack.current(), None);
        stack.push("a");
        stack.push("b");
        assert_eq!(stack.current(), Some("b"));
        assert_eq!(stack.current(), Some("b"));
        assert_eq!(stack.len(), 2);
    }

    #[test]
    fn popping_an_empty_stack_is_not_an_error() {
        let mut stack = DriverStack::default();
        assert_eq!(stack.pop(), None);
        assert_eq!(stack.len(), 0);
    }

    #[test]
    fn a_stack_collects_iterates_and_clears_in_push_order() {
        let mut stack: DriverStack = ["a", "b"].into_iter().collect();
        stack.extend(["c"]);
        assert_eq!(stack.iter().collect::<Vec<_>>(), ["a", "b", "c"]);
        assert_eq!(stack.iter().next_back(), Some("c"));
        assert_eq!(stack.current(), Some("c"));
        let copy = stack.clone();
        stack.clear();
        assert!(stack.is_empty());
        assert_eq!(copy.len(), 3);
        assert_ne!(copy, stack);
    }
}
