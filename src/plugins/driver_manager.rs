//! Helper types for managing multiple WebDriver sessions and window handles.

use std::collections::VecDeque;

/// Stack of WebDriver window handles for pushing/popping contexts.
pub struct DriverStack {
    stack: VecDeque<String>,
}

impl DriverStack {
    /// Creates an empty stack.
    pub fn new() -> Self {
        Self {
            stack: VecDeque::new(),
        }
    }

    /// Pushes a window handle onto the stack.
    pub fn push(&mut self, handle: String) {
        self.stack.push_back(handle);
    }

    /// Removes and returns the most recently pushed window handle.
    pub fn pop(&mut self) -> Option<String> {
        self.stack.pop_back()
    }

    /// Returns the current top window handle without removing it.
    pub fn current(&self) -> Option<&str> {
        self.stack.back().map(String::as_str)
    }

    /// Returns the number of handles on the stack.
    pub fn len(&self) -> usize {
        self.stack.len()
    }

    /// Returns true when the stack contains no handles.
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }
}

impl Default for DriverStack {
    fn default() -> Self {
        Self::new()
    }
}
