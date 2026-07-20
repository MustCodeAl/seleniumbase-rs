use std::collections::VecDeque;

/// Stack of WebDriver window handles for pushing/popping contexts.
pub struct DriverStack {
    stack: VecDeque<String>,
}

impl DriverStack {
    pub fn new() -> Self {
        Self {
            stack: VecDeque::new(),
        }
    }

    pub fn push(&mut self, handle: String) {
        self.stack.push_back(handle);
    }

    pub fn pop(&mut self) -> Option<String> {
        self.stack.pop_back()
    }

    pub fn current(&self) -> Option<&String> {
        self.stack.back()
    }

    pub fn len(&self) -> usize {
        self.stack.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }
}

impl Default for DriverStack {
    fn default() -> Self {
        Self::new()
    }
}
